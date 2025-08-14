//! 中断处理模块，用户或内核模式下发生中断或异常时进行处理

use core::num::Wrapping;
use core::sync::atomic::Ordering;

use crate::{consts::{TRAMPOLINE, TRAPFRAME, UART0_IRQ, VIRTIO0_IRQ}, process::{PROC_MANAGER, Proc}};
use crate::register::{stvec, sstatus, sepc, stval, sip,
    scause::{self, ScauseType}};
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
    let p = CPU_MANAGER.my_proc();

    // 根据中断原因分发处理
    match scause::get_scause() {
        ScauseType::IntSExt => {
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
                // panic!("unexpected interrupt, irq={}", irq);
            }
            // 其他中断暂不处理

            // 完成中断处理
            if irq > 0 {
                plic::complete(irq);
            }

            // 检查进程终止标志
            p.check_abondon(-1);
        }
        ScauseType::IntSSoft => {
            // 监督者模式软件中断

            // 仅在CPU 0上更新时钟计数
            if CpuManager::cpu_id() == 0 {
                clock_intr();
            }

            // 清除软件中断标志
            sip::clear_ssip();

            // 检查进程终止标志
            p.check_abondon(-1);
            // 主动让出CPU
            p.yielding();
        }
        ScauseType::ExcUEcall => {
            // 用户模式系统调用

            // 检查进程终止标志
            p.check_abondon(-1);
            // 处理系统调用
            p.syscall();
            // 再次检查终止标志（系统调用可能设置）
            p.check_abondon(-1);
        }
        ScauseType::Unknown => {
            // 未知异常

            println!("scause {:#x}", scause::read());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());

            // 终止当前进程
            p.abondon(-1);
        }
    }

    // 返回用户空间
    user_trap_ret();
}

/// Return to user space
pub unsafe fn user_trap_ret() -> ! {
    // disable interrupts and prepare sret to user mode
    sstatus::intr_off();
    sstatus::user_ret_prepare();

    // send interrupts and exceptions to uservec/trampoline in trampoline.S
    stvec::write(TRAMPOLINE.into());

    // let the current process prepare for the sret
    let satp = {
        let pd = CPU_MANAGER.my_proc().data.get_mut();
        pd.user_ret_prepare()
    };

    // call userret with virtual address
    extern "C" {
        fn trampoline();
        fn userret();
    }
    let distance = userret as usize - trampoline as usize;
    let userret_virt: extern "C" fn(usize, usize) -> ! =
        core::mem::transmute(Into::<usize>::into(TRAMPOLINE) + distance);
    userret_virt(TRAPFRAME.into(), satp);
}

/// Used to handle kernel space's trap
/// Being called from kernelvec
#[no_mangle]
pub unsafe fn kerneltrap() {
    let local_sepc = sepc::read();
    let local_sstatus = sstatus::read();

    if !sstatus::is_from_supervisor() {
        panic!("not from supervisor mode");
    }
    if sstatus::intr_get() {
        panic!("interrupts enabled");
    }

    match scause::get_scause() {
        ScauseType::IntSExt => {
            // this is a supervisor external interrupt, via PLIC.

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
        ScauseType::IntSSoft => {
            // software interrupt from a machine-mode timer interrupt,
            // forwarded by timervec in kernelvec.S.

            // only cpu 0 inc ticks
            if CpuManager::cpu_id() == 0 {
                clock_intr();
            }

            // acknowledge the software interrupt
            sip::clear_ssip();

            // give up the cpu
            CPU_MANAGER.my_cpu_mut().try_yield_proc();
        }
        ScauseType::ExcUEcall => {
            panic!("ecall from supervisor mode");
        }
        ScauseType::Unknown => {
            println!("scause {:#x}", scause::read());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());
            panic!("unknown trap type");
        }
    }

    // The yielding() may have caused some traps to occur,
    // so restore trap registers for use by kernelvec.S's sepc instruction.
    sepc::write(local_sepc);
    sstatus::write(local_sstatus);
}

static TICKS: SpinLock<Wrapping<usize>> = SpinLock::new(Wrapping(0), "time");

fn clock_intr() {
    let mut guard = TICKS.lock();
    *guard += Wrapping(1);
    unsafe { PROC_MANAGER.wakeup(&TICKS as *const _ as usize); }
    drop(guard);
}

/// Sleep for a specified number of ticks.
pub fn clock_sleep(p: &Proc, count: usize) -> Result<(), ()> {
    let mut guard = TICKS.lock();
    let old_ticks = *guard;
    while (*guard - old_ticks) < Wrapping(count) {
        if p.killed.load(Ordering::Relaxed) {
            return Err(())
        }
        p.sleep(&TICKS as *const _ as usize, guard);
        guard = TICKS.lock();
    }
    Ok(())
}

/// Read the current ticks.
pub fn clock_read() -> usize {
    TICKS.lock().0
}
