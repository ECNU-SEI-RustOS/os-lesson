//! 中断处理模块，用户或内核模式下发生中断或异常时进行处理

use core::alloc::GlobalAlloc;
use core::{alloc::Layout, num::Wrapping};
use core::sync::atomic::Ordering;

use crate::mm::{pg_round_down, PhysAddr, PteFlag, VirtAddr};
use crate::mm::{trapframe_from_pid, VirtAddr};
use crate::{consts::{ConstAddr, PAGE_SIZE, TRAMPOLINE, TRAPFRAME, UART0_IRQ, USER_STACK_SIZE, VIRTIO0_IRQ}, mm::KERNEL_HEAP, process::{Process, PROC_MANAGER}};
use crate::register::{stvec, sstatus, sepc, stval, sip,
    scause::{self}};
use crate::process::{CPU_MANAGER, CpuManager};
use crate::spinlock::SpinLock;
use crate::plic;
use crate::driver::virtio_disk::DISK;
use crate::driver::uart::UART;

/// 初始化当前CPU核心的中断处理
///
/// # 功能说明
/// 设置监督者模式陷阱向量基址寄存器(stvec)，
/// 指向内核中断处理程序(kernelvec)。
///
/// # 安全性
/// - 必须在核心启动时调用
/// - 需确保kernelvec符号在链接脚本中正确定义
pub unsafe fn trap_init_hart() {
    extern "C" {
        fn kernelvec();
    }

    stvec::write(kernelvec as usize);
}

/// 用户模式陷阱入口（由trampoline.S调用）
///
/// # 功能说明
/// 处理从用户模式触发的所有中断和异常事件，
/// 包括系统调用、设备中断、时钟中断等。
///
/// # 流程解释
/// 1. 验证中断来源确为用户模式
/// 2. 设置陷阱处理程序为内核模式处理入口
/// 3. 根据中断原因(scause)分发处理：
///   - 外部中断：处理UART/磁盘中断
///   - 软件中断：处理时钟中断
///   - 系统调用：执行系统调用处理
///   - 其他异常：终止进程
/// 4. 处理完成后返回用户空间
///
/// # 安全性
/// - 必须由trampoline.S在正确上下文中调用
/// - 直接访问进程管理器和硬件寄存器
#[no_mangle]
pub unsafe extern fn user_trap() {
    // 验证中断来源：必须来自用户模式
    if !sstatus::is_from_user() {
        panic!("not from user mode, sstatus={:#x}", sstatus::read());
    }

    // 设置陷阱处理程序为内核模式入口
    extern "C" {fn kernelvec();}
    stvec::write(kernelvec as usize);

    // 获取当前进程
    let process = CPU_MANAGER.my_proc();

    let scause = Scause::read();

    // 根据中断原因分发处理
    match scause.cause() {
        Trap::Interrupt(scause::Interrupt::SupervisorExternal) => {
            // 监督者模式外部中断

            // 从PLIC中断控制器获取中断号
            let irq = plic::claim();

            // 处理UART串口中断
            if irq as usize == UART0_IRQ {
                UART.intr();

            // 处理虚拟磁盘中断
            } else if irq as usize == VIRTIO0_IRQ {
                DISK.lock().intr();
            } else {
                //panic!("unexpected interrupt, irq={}", irq);
            }
            // 其他中断暂不处理

            // 完成中断处理
            if irq > 0 {
                plic::complete(irq);
            }

            // 检查进程终止标志
            process.check_abondon(-1);
        }
        Trap::Interrupt(scause::Interrupt::SupervisorSoft) => {
            // 监督者模式软件中断
            // 仅在CPU 0上更新时钟计数
            if CpuManager::cpu_id() == 0 {
                clock_intr();
            }

            if STARTED.load(Ordering::SeqCst) {
                let pa = unsafe { CPU_MANAGER.my_proc().alarm.get_mut() };
                let pd = unsafe { CPU_MANAGER.my_proc().data.get_mut() };
                if pa.interval > 0 {
                    pa.past_tick += 1;
                }
                if (pa.past_tick == pa.interval) && (!pa.handler_called) && (pa.interval > 0) {
                    pa.past_tick = 0;
                    unsafe { core::ptr::copy_nonoverlapping(pd.tf, pa.alarm_frame, 1) };
                    (unsafe {&mut *pd.tf }).epc = pa.handler_addr as usize;
                    pa.handler_called = true;
                }
            }
            // 清除软件中断标志
            sip::clear_ssip();

            // 检查进程终止标志
            process.check_abondon(-1);
            // 主动让出CPU
            process.yielding();
        }
        Trap::Exception(scause::Exception::UserEnvCall)=> {
            // 用户模式系统调用

            // 检查进程终止标志
            p.check_abondon(-1);
            // 处理系统调用
            p.syscall();
            // 再次检查终止标志（系统调用可能设置）
            p.check_abondon(-1);
        }
        ScauseType::PageFault => {
            //println!("!!");
            let layout = Layout::from_size_align(4096, 4096).expect("Invalid layout");
            // 分配 4KB 的内存空间
            let ptr = GlobalAlloc::alloc(&KERNEL_HEAP, layout) as *mut u8;
            if ptr.is_null() {
                p.killed = true.into();
            }
            p.data.get_mut().pagetable.as_mut().unwrap().map_pages(
                VirtAddr::from_raw(pg_round_down(stval::read())),
                4096,
                PhysAddr::from_raw(ptr as usize),
                PteFlag::R | PteFlag::W | PteFlag::U);
        }
        _ => {
            // 未知异常

            println!("scause {:?}", scause.cause());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());

            // 终止当前进程
            process.abondon(-1);
        }
    }

    // 返回用户空间
    user_trap_ret();
}
use crate::process::task::task::trapframe_from_tid;
/// 返回用户空间
///
/// # 功能说明
/// 准备并执行从内核模式返回到用户空间的所有操作，
/// 包括设置状态寄存器、陷阱向量和页表切换。
///
/// # 流程解释
/// 1. 禁用中断（sstatus::intr_off）
/// 2. 设置返回用户模式所需状态（sstatus::user_ret_prepare）
/// 3. 设置陷阱向量为用户空间处理程序（trampoline.S）
/// 4. 准备用户空间页表
/// 5. 通过trampoline跳转回用户空间
///
/// # 返回值
/// 永不返回（-> !）
///
/// # 安全性
/// - 必须在用户陷阱处理完成后调用
/// - 涉及页表和地址空间切换
pub unsafe fn user_trap_ret() -> ! {
    // 禁用中断并设置返回用户模式状态
    sstatus::intr_off();
    sstatus::user_ret_prepare();

    // 设置陷阱向量为用户空间处理程序（trampoline.S）
    stvec::write(TRAMPOLINE.into());

    // let pf;
    // 获取当前进程的用户页表
    // let (satp,pid) = {
    //     let pdata = CPU_MANAGER.my_proc().data.get_mut();
    //     let pid = CPU_MANAGER.my_proc().excl.lock().pid;
    //     let a =pdata.pagetable.as_ref().unwrap();
    //     pf = a.find_pa_by_kernel(trapframe_from_pid(pid).into()).unwrap().into_raw();
    //     (pdata.user_ret_prepare(), pid)
    // };
    let tf;
    let (satp,tid) = {
        let pdata = CPU_MANAGER.my_proc().data.get_mut();
        let res = pdata.user_ret_prepare_task();
        let a = pdata.pagetable.as_mut().unwrap();
        
        tf = a.find_pa_by_kernel(trapframe_from_tid(res.1).into()).unwrap().into_raw();
        res
    };
    // debug_assert_eq!((tf as *const TrapFrame).as_ref().unwrap(),(pf as *const TrapFrame).as_ref().unwrap());
    // kinfo!("tf {:?}\npf {:?}", (tf as *const TrapFrame).as_ref().unwrap(),(pf as *const TrapFrame).as_ref().unwrap());
    //计算userret在跳板页中的虚拟地址
    extern "C" {
        fn trampoline();    // 跳板页起始地址
        fn userret();       // 用户返回函数
    }
    let distance = userret as usize - trampoline as usize;
    let userret_virt: extern "C" fn(usize, usize) -> ! =
        core::mem::transmute(Into::<usize>::into(TRAMPOLINE) + distance);

    // 调用userret(TRAPFRAME, satp)返回用户空间
    userret_virt(trapframe_from_tid(tid).into(), satp);
}

/// 内核模式陷阱处理（由kernelvec调用）
///
/// # 功能说明
/// 处理内核执行过程中发生的中断和异常，
/// 包括设备中断、时钟中断等。
///
/// # 流程解释
/// 1. 保存关键寄存器状态（sepc, sstatus）
/// 2. 验证中断来源为内核模式
/// 3. 根据中断原因分发处理：
///   - 外部中断：处理UART/磁盘中断
///   - 软件中断：处理时钟中断并尝试调度
///   - 系统调用：内核模式不应触发（panic）
///   - 其他异常：panic
/// 4. 恢复保存的寄存器状态
///
/// # 安全性
/// - 必须由kernelvec在正确上下文中调用
/// - 直接访问硬件和全局状态
#[no_mangle]
pub unsafe fn kerneltrap() {
    // 保存关键寄存器状态
    let local_sepc = sepc::read();
    let local_sstatus = sstatus::read();

    // 验证中断来源：必须来自内核模式
    if !sstatus::is_from_supervisor() {
        panic!("not from supervisor mode");
    }

    // 验证中断状态：内核陷阱处理期间应禁用中断
    if sstatus::intr_get() {
        panic!("interrupts enabled");
    }

    let scause = Scause::read();

    // 根据中断原因分发处理
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorExternal)=> {
            // 监督者模式外部中断

            // 处理PLIC中断（同用户模式）
            let irq = plic::claim();
            if irq as usize == UART0_IRQ {
                UART.intr();
            } else if irq as usize == VIRTIO0_IRQ {
                DISK.lock().intr();
            } else {
                // panic!("unexpected interrupt, irq={}", irq);
            }
            if irq > 0 {
                plic::complete(irq);
            }
        }
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
            // 监督者模式软件中断

            // 仅在CPU 0上更新时钟计数
            if CpuManager::cpu_id() == 0 {
                clock_intr();
            }

            // 清除软件中断标志
            sip::clear_ssip();

            // 尝试让出CPU（调度其他进程）
            CPU_MANAGER.my_cpu_mut().try_yield_proc();
        }
        Trap::Exception(Exception::SupervisorEnvCall) => {  // 用户模式系统调用（内核不应触发）

            unimplemented!("supervisor call");
        }
        Trap::Exception(Exception::UserEnvCall)=> {
            panic!("ecall from supervisor mode");
        }
        ScauseType::PageFault => {
            
        }
        _ => {    // 未知异常
            println!("scause {:?}", scause.cause());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());
            panic!("unknown trap type");
        }
    }

    // 恢复保存的寄存器状态
    sepc::write(local_sepc);
    sstatus::write(local_sstatus);
}

/// 全局时钟计数器（自旋锁保护）
static TICKS: SpinLock<Wrapping<usize>> = SpinLock::new(Wrapping(0), "time");

/// 处理时钟中断（更新全局计数器）
///
/// # 功能说明
/// 增加全局时钟计数并唤醒等待时钟的进程。
/// 由时钟中断处理程序调用。
fn clock_intr() {
    let mut guard = TICKS.lock();
    *guard += Wrapping(1);
    unsafe { PROC_MANAGER.wakeup(&TICKS as *const _ as usize); }
    drop(guard);
}

/// 使进程休眠指定时钟周期
///
/// # 功能说明
/// 将当前进程置于休眠状态，直到经过指定数量的时钟周期。
///
/// # 参数
/// - `p`: 当前进程引用
/// - `count`: 要休眠的时钟周期数
///
/// # 返回值
/// - `Ok(())`: 成功休眠指定周期
/// - `Err(())`: 休眠期间进程被终止
pub fn clock_sleep(process: &Process, count: usize) -> Result<(), ()> {
    let mut guard = TICKS.lock();
    let old_ticks = *guard; // 记录起始时钟

    // 等待指定周期
    while (*guard - old_ticks) < Wrapping(count) {
        // 检查进程终止标志
        if process.killed.load(Ordering::Relaxed) {
            return Err(())
        }

        // 在TICKS地址上休眠
        process.sleep(&TICKS as *const _ as usize, guard);
        // 被唤醒后重新获取锁
        guard = TICKS.lock();
    }
    Ok(())
}

/// 读取当前时钟计数值
///
/// # 返回值
/// 系统启动以来的时钟周期数
pub fn clock_read() -> usize {
    TICKS.lock().0
}

