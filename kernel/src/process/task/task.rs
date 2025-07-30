use core::cell::{Cell, UnsafeCell};
use crate::consts::{KERNEL_STACK_SIZE, PAGE_SIZE, USER_STACK_SIZE, TRAMPOLINE, ConstAddr, TRAPFRAME, NPROC};
use crate::mm::{PhysAddr, RawPage, RawQuadPage, RawSinglePage, VirtAddr};
use crate::process::{fork_ret, kvm_map};
use crate::process::proc::Process;
use crate::process::trapframe::TrapFrame;
use crate::process::Context;
use super::tid::TID_ALLOCATOR;
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TaskStatus {
    /// 线程处于可运行状态，等待调度器调度执行。
    Ready,
    /// 线程当前正在 CPU 上运行。
    Running,
    /// 线程处于阻塞状态，等待某事件或资源唤醒。
    Blocked,
    /// 线程已退出，处于僵尸状态，等待父进程回收。
    Zombie,
}

/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position_by_tid(tid: usize) -> (usize, usize) {
    let kstack_top: usize = Into::<usize>::into(TRAMPOLINE) - (tid + 1024 + 1) * KERNEL_STACK_SIZE;
    (kstack_top - KERNEL_STACK_SIZE, kstack_top)
}

pub struct KernelStack {
    kstack_base: usize,
}

impl KernelStack {

}

pub unsafe fn kstack_alloc(tid: usize) -> KernelStack {
    let (kstack_bottom, kstack_top) = kernel_stack_position_by_tid(tid);
    // KERNEL_SPACE.exclusive_access().insert_framed_area(
    //     kstack_bottom.into(),
    //     kstack_top.into(),
    //     MapPermission::R | MapPermission::W,
    // );
    let pa = RawQuadPage::new_zeroed() as usize;
    // kvm_map(
    //     VirtAddr::try_from(kstack_bottom).unwrap(),
    //     PhysAddr::try_from(pa).unwrap(),
    //     KERNEL_STACK_SIZE,
    //     PteFlag::R | PteFlag::W,
    // );
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
        alloc_user_res: bool,
    ) -> Self {
        let task_user_res = Self {
            tid,
            ustack_base,
            process,
        };
        if alloc_user_res {
            task_user_res.alloc_user_res();
        }
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
    pub tid: usize,
    pub pos: usize,
    // mutable
    inner: UnsafeCell<TaskControlInner>,
}
pub struct TaskControlInner {
    pub res: Option<TaskUserRes>,
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
}
impl Task {
    pub fn new(process: Option<*mut Process>, pos:usize, ustack_base: usize, alloc_user_res: bool) -> Self {
        let tid = TID_ALLOCATOR.lock().tid_alloc();
        kinfo!("alloc tid:{}", tid);
        let res = TaskUserRes::new(process, tid, ustack_base, alloc_user_res);
        let kstack = unsafe { kstack_alloc(tid) };
        let proc = unsafe { &mut *process.unwrap() };
        let procdata = proc.data.get_mut();
        if alloc_user_res {
            // let pa = unsafe { RawQuadPage::new_zeroed() as usize };
            // let va_bottom = ustack_bottom_from_tid(ustack_base, tid);
            // let procdata = proc.data.get_mut();
            // procdata.pagetable.as_mut().unwrap().uvm_alloc(PGSIZE*5, PGSIZE*10);
            // procdata.pagetable.as_mut().unwrap().uvm_clear(PGSIZE*10 - PGSIZE *5);
            // let res = procdata.pagetable.as_mut().unwrap().map_pages(
            //     VirtAddr::try_from(va_bottom).unwrap(), 
            //     USER_STACK_SIZE, 
            //     PhysAddr::try_from(pa).unwrap(), 
            //     PteFlag::R | PteFlag::W | PteFlag::X | PteFlag::U);
            // match res {
            //     Ok(_) => {},
            //     Err(err) => {panic!("{}",err)},
            // }
            kinfo!("ustack map");
        }

        let trap_frame = unsafe { RawSinglePage::try_new_zeroed().unwrap() as *mut TrapFrame };
        // procdata.pagetable.as_mut().unwrap()
        //     .map_pages(
        //         VirtAddr::from(TRAPFRAME),
        //         PGSIZE,
        //         PhysAddr::try_from(trap_frame as usize).unwrap(),
        //         PteFlag::R | PteFlag::W,
        //     );

        // // 分配进程页表
        //             match PageTable::alloc_proc_pagetable(procdata.trapframe as usize) {
        //                 Some(pagetable) => procdata.pagetable = Some(pagetable),
        //                 None => {
        //                     unsafe {
        //                         RawSinglePage::from_raw_and_drop(procdata.trapframe as *mut u8);
        //                     }
        //                     return None;
        //                 }
        //             }
        let mut context = Context::new();
        context.set_ra(fork_ret as usize);
        context.set_sp(kstack.kstack_base + KERNEL_STACK_SIZE);

        Self {
            process,
            kstack,
            tid,
            pos,
            inner: UnsafeCell::new(TaskControlInner {
                res: Some(res),
                trapframe: trap_frame,
                task_context: context,
                task_status: TaskStatus::Ready,
                exit_code: None,
            }),
        }
    }
    pub fn get_context(&mut self) -> *mut Context {
        &mut self.inner.get_mut().task_context as *mut Context
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        TID_ALLOCATOR.lock().tid_dealloc(self.tid);
        kinfo!("dealloc tid:{}", self.tid);
    }
}

#[inline]
fn kstack_by_tid(tid: usize) -> (usize,usize) {
    let kstack_bottom = Into::<usize>::into(TRAMPOLINE) - (NPROC + 1) * (KERNEL_STACK_SIZE + PAGE_SIZE) - (tid + 1) * KERNEL_STACK_SIZE;
    let kstack_top = kstack_bottom + KERNEL_STACK_SIZE;
    (kstack_bottom,kstack_top)
}

#[inline]
fn ustack_bottom_from_tid(ustack_base: usize, pos: usize) -> usize {
    ustack_base + pos * (PAGE_SIZE + USER_STACK_SIZE)
}
/// get the trapframe ptr in user space by tid
fn trapframe_from_tid(tid: usize) -> ConstAddr {
    TRAPFRAME.const_sub(tid * PAGE_SIZE)
}