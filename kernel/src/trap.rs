//! Trap handler between user/kernel space and kernel space

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

pub unsafe fn trap_init_hart() {
    extern "C" {
        fn kernelvec();
    }

    stvec::write(kernelvec as usize);
}

/// uservec in trampoline.S jumps here 
#[no_mangle]
pub unsafe extern fn user_trap() {
    if !sstatus::is_from_user() {
        panic!("not from user mode, sstatus={:#x}", sstatus::read());
    }

    // switch the trap handler to kerneltrap()
    extern "C" {fn kernelvec();}
    stvec::write(kernelvec as usize);

    let process = CPU_MANAGER.my_proc();

    let scause = Scause::read();

    match scause.cause() {
        Trap::Interrupt(scause::Interrupt::SupervisorExternal) => {
            // this is a supervisor external interrupt, via PLIC.

            let irq = plic::claim();
            if irq as usize == UART0_IRQ {
                UART.intr();
            } else if irq as usize == VIRTIO0_IRQ {
                DISK.lock().intr();
            } else {
                //panic!("unexpected interrupt, irq={}", irq);
            }
            if irq > 0 {
                plic::complete(irq);
            }

            process.check_abondon(-1);
        }
        Trap::Interrupt(scause::Interrupt::SupervisorSoft) => {
            // software interrupt from a machine-mode timer interrupt,
            // forwarded by timervec in kernelvec.S.
            // only cpu 0 inc ticks
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
            // acknowledge the software interrupt
            sip::clear_ssip();

            // give up the cpu
            process.check_abondon(-1);
            process.yielding();
        }
        Trap::Exception(scause::Exception::UserEnvCall)=> {
            p.check_abondon(-1);
            p.syscall();
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
            println!("scause {:?}", scause.cause());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());
            process.abondon(-1);
        }
    }

    user_trap_ret();
}
use crate::process::task::task::trapframe_from_tid;
/// Return to user space
pub unsafe fn user_trap_ret() -> ! {
    // disable interrupts and prepare sret to user mode
    sstatus::intr_off();
    sstatus::user_ret_prepare();

    // send interrupts and exceptions to uservec/trampoline in trampoline.S
    stvec::write(TRAMPOLINE.into());

    // let pf;
    // let the current process prepare for the sret
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
    //call userret with virtual address
    extern "C" {
        fn trampoline();
        fn userret();
    }
    let distance = userret as usize - trampoline as usize;
    let userret_virt: extern "C" fn(usize, usize) -> ! =
        core::mem::transmute(Into::<usize>::into(TRAMPOLINE) + distance);
    userret_virt(trapframe_from_tid(tid).into(), satp);
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

    let scause = Scause::read();

    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorExternal)=> {
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
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
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
        Trap::Exception(Exception::SupervisorEnvCall) => {

            unimplemented!("supervisor call");
        }
        Trap::Exception(Exception::UserEnvCall)=> {
            panic!("ecall from supervisor mode");
        }
        ScauseType::PageFault => {
            
        }
        _ => {
            println!("scause {:?}", scause.cause());
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
pub fn clock_sleep(process: &Process, count: usize) -> Result<(), ()> {
    let mut guard = TICKS.lock();
    let old_ticks = *guard;
    while (*guard - old_ticks) < Wrapping(count) {
        if process.killed.load(Ordering::Relaxed) {
            return Err(())
        }
        process.sleep(&TICKS as *const _ as usize, guard);
        guard = TICKS.lock();
    }
    Ok(())
}

/// Read the current ticks.
pub fn clock_read() -> usize {
    TICKS.lock().0
}

