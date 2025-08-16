use core::ptr;

use crate::consts::{KERNEL_STACK_SIZE, PAGE_SIZE, USER_STACK_SIZE, TRAMPOLINE, ConstAddr, TRAPFRAME, NPROC};
use crate::mm::{PhysAddr, RawPage, RawQuadPage, RawSinglePage, VirtAddr};
use crate::process::{fork_ret};
use crate::mm::{kvm_task_kstack_map};
use crate::process::proc::Process;
use crate::process::trapframe::TrapFrame;
use crate::process::Context;
use crate::process::PteFlag;
use crate::spinlock::SpinLock;
use super::tid::TID_ALLOCATOR;
use crate::process::CpuManager;
use crate::register::satp;
use crate::trap::user_trap;
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TaskStatus {
    /// 线程处于可运行状态，等待调度器调度执行。
    Ready,
    /// 线程当前正在 CPU 上运行。
    Running,
    /// 线程处于阻塞状态，等待某事件或资源唤醒。
    Blocked,
    /// 线程已退出，处于僵尸状态，等待父进程（主线程）回收。
    Zombie,
}


pub struct KernelStack {
    kstack_base: usize,
}

impl KernelStack {

}

pub unsafe fn kstack_alloc(tid: usize) -> KernelStack {
    let (kstack_bottom, kstack_top) = kernel_stack_position_by_tid(tid);
    let pa = RawQuadPage::new_zeroed() as usize;
    
    kvm_task_kstack_map(
        VirtAddr::try_from(kstack_bottom).unwrap(),
        PhysAddr::try_from(pa).unwrap(),
        tid,
        KERNEL_STACK_SIZE,
        PteFlag::R | PteFlag::W,
    );
    KernelStack {
        kstack_base: kstack_bottom
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {

    }
}
pub struct TaskUserRes {
    pub tid: usize,
    ///线程独占的用户态资源
    pub ustack_base: usize,
    pub process: Option<*mut Process>,
}

impl TaskUserRes {
    pub fn new(
        process: Option<*mut Process>,
        tid: usize,
        ustack_base: usize,
    ) -> Self {
        let task_user_res = Self {
            tid,
            ustack_base,
            process,
        };
        task_user_res
    }

    pub fn ustack_base(&self) -> usize {
        self.ustack_base
    }
    pub fn ustack_top(&self) -> usize {
        ustack_bottom_from_tid(self.ustack_base, self.tid) + USER_STACK_SIZE
    }

    pub fn alloc_user_res(&self) {}

    fn dealloc_user_res(&self) {}
}

pub struct Task {
    // immutable
    pub process: Option<*mut Process>,
    pub kstack: KernelStack,
    pub ustack_base: usize,
    pub tid: usize,
    pub pos: usize,
    // mutable
    pub inner: SpinLock<TaskControlInner>,
}

pub struct TaskControlInner {
    pub trapframe: *mut TrapFrame,
    pub task_context: Context,
    pub task_status: TaskStatus,
    pub exit_code: Option<i32>,
}

impl TaskControlInner {
    pub fn get_trap_frame(&self) -> &'static mut TrapFrame {
        unsafe { self.trapframe.as_mut().unwrap() }
    }
    pub fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn set_task_context(&mut self, task_context: Context) {
        self.task_context = task_context
    }
    pub fn get_context(&mut self) -> *mut Context {
        &mut self.task_context as *mut Context
    }
    pub fn set_status(&mut self, status: TaskStatus){
        self.task_status = status;
    }
    pub fn get_trapframe(&mut self) -> *mut TrapFrame{
        self.trapframe
    }
}
impl Task {
    pub fn new(process: Option<*mut Process>, pos:usize, ustack_base: usize, entry: usize) -> Self {
        let tid = TID_ALLOCATOR.lock().tid_alloc();
        kinfo!("new alloc tid:{}", tid);
        let kstack = unsafe { kstack_alloc(tid) };
        let proc = unsafe { process.unwrap().as_mut().unwrap() };
        let procdata = proc.data.get_mut();
        let trapframe_pa = unsafe { RawSinglePage::try_new_zeroed().unwrap() as usize};
        kinfo!("set trapframe map {:?} {:x}",VirtAddr::from(trapframe_from_tid(tid)),unsafe { procdata.pagetable.unwrap().as_mut().unwrap().as_satp() });
        match unsafe { procdata.pagetable.unwrap().as_mut().unwrap() }
                    .map_pages(
                        VirtAddr::from(trapframe_from_tid(tid)),
                        PAGE_SIZE,
                        PhysAddr::try_from(trapframe_pa).unwrap(),
                        PteFlag::R | PteFlag::W,
                    ) {
            Ok(_) => {},
            Err(_) => {panic!("task trapframe error")},
        };
        let ustack_bottom =ustack_bottom_from_tid(ustack_base, pos);
        let trapframe =unsafe {&mut *(trapframe_pa as *mut TrapFrame)};
        trapframe.epc = entry;
        trapframe.sp = ustack_bottom + USER_STACK_SIZE;
        trapframe.kernel_satp = satp::read();
        // current kernel stack's content is cleaned
        // after returning to the kernel space
        trapframe.kernel_sp = kstack.kstack_base + KERNEL_STACK_SIZE;
        trapframe.kernel_trap = user_trap as usize;
        trapframe.kernel_hartid = unsafe { CpuManager::cpu_id() };
        kinfo!("new {:?} \n {:?}",trapframe, unsafe { proc.data.get_mut().trapframe.as_ref().unwrap() });
        let mut context = Context::new();
        context.set_ra(fork_ret as usize);
        context.set_sp(kstack.kstack_base + KERNEL_STACK_SIZE);

        Self {
            process,
            kstack,
            ustack_base,
            tid,
            pos,
            inner: SpinLock::new(TaskControlInner {
                trapframe: trapframe_pa as _,
                task_context: context,
                task_status: TaskStatus::Ready,
                exit_code: None,
            },""),
        }
    }
    pub fn from(process: Option<*mut Process>, pos:usize, ustack_base: usize, ptrapframe: *mut TrapFrame) -> Self {
        let tid = TID_ALLOCATOR.lock().tid_alloc();
        kinfo!("new alloc tid:{}", tid);
        let kstack = unsafe { kstack_alloc(tid) };
        let proc = unsafe { &mut *process.unwrap() };
        let procdata = proc.data.get_mut();
        kinfo!("set trapframe");
        let ctrapframe = unsafe { RawSinglePage::try_new_zeroed().unwrap() as usize};
        kinfo!("map {:?} {:x}",VirtAddr::from(trapframe_from_tid(tid)),unsafe { procdata.pagetable.unwrap().as_mut().unwrap().as_satp() });
        match unsafe { procdata.pagetable.unwrap().as_mut().unwrap() }
                    .map_pages(
                        VirtAddr::from(trapframe_from_tid(tid)),
                        PAGE_SIZE,
                        PhysAddr::try_from(ctrapframe).unwrap(),
                        PteFlag::R | PteFlag::W,
                    ) {
            Ok(_) => {},
            Err(_) => {panic!("task trapframe error")},
        };
        let trapframe =unsafe {(ctrapframe as *mut TrapFrame).as_mut().unwrap()};
        unsafe {
            ptr::copy_nonoverlapping(ptrapframe, ctrapframe as *mut TrapFrame, 1);
        }
        trapframe.a0 = 0;
        trapframe.kernel_sp = kstack.kstack_base + KERNEL_STACK_SIZE;
        kinfo!("fork {:?} \n {:?}",trapframe,unsafe {& *proc.data.get_mut().trapframe });
        let mut context = Context::new();
        context.set_ra(fork_ret as usize);
        context.set_sp(kstack.kstack_base + KERNEL_STACK_SIZE);

        Self {
            process,
            kstack,
            ustack_base,
            tid,
            pos,
            inner: SpinLock::new(TaskControlInner {
                trapframe: ctrapframe as _,
                task_context: context,
                task_status: TaskStatus::Ready,
                exit_code: None,
            },""),
        }
    }
    pub fn get_context(&mut self) -> *mut Context {
        &mut self.inner.lock().task_context as *mut Context
    }
    pub fn set_status(&self, status: TaskStatus){
        self.inner.lock().task_status = status;
    }
    pub fn get_kstack_bottom(&self)-> usize {
        self.kstack.kstack_base
    }
    pub fn get_trap_frame(&self) -> &'static mut TrapFrame {
        self.inner.lock().get_trap_frame()
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        let tid = self.tid;
        let proc = unsafe { &mut *self.process.unwrap() };
        let procdata = proc.data.get_mut();
        kinfo!("free trapframe unmap {:?} {:x}",VirtAddr::from(trapframe_from_tid(tid)),unsafe { procdata.pagetable.unwrap().as_mut().unwrap().as_satp() });
        
        unsafe { procdata.pagetable.unwrap().as_mut().unwrap().uvm_unmap(trapframe_from_tid(tid).into(), 1, false) };
        TID_ALLOCATOR.lock().tid_dealloc(tid);
        kinfo!("dealloc tid:{}", tid);
    }
}

/// Return (bottom, top) of a kernel stack in kernel space.
#[inline]
fn kernel_stack_position_by_tid(tid: usize) -> (usize,usize) {
    let kstack_bottom = Into::<usize>::into(TRAMPOLINE) - (tid + 4 + NPROC) * (KERNEL_STACK_SIZE + PAGE_SIZE) ;
    let kstack_top = kstack_bottom + KERNEL_STACK_SIZE;
    (kstack_bottom,kstack_top)
}

#[inline]
fn ustack_bottom_from_tid(ustack_base: usize, pos: usize) -> usize {
    ustack_base + pos * (PAGE_SIZE + USER_STACK_SIZE)
}
/// get the trapframe ptr in user space by tid
#[inline]
pub fn trapframe_from_tid(tid: usize) -> ConstAddr {
    TRAPFRAME.const_sub((NPROC + 1) * (PAGE_SIZE + PAGE_SIZE) + tid * (PAGE_SIZE + PAGE_SIZE))
}