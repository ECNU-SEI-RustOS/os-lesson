use crate::{process::{Task}, spinlock::SpinLock};
use alloc::collections::VecDeque;
use crate::process::PROC_MANAGER;
use crate::process::CPU_MANAGER;
pub struct Semaphore {
    pub inner: SpinLock<SemaphoreInner>
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<*const Task>,
}

impl Semaphore {
    pub fn new(res_count: usize) -> Self {
        Self {
            inner:
                SpinLock::new(
                    SemaphoreInner {
                        count: res_count as isize,
                        wait_queue: VecDeque::new()
                    },
                    "lock"
                )
            ,
        }
    }

    pub fn up(&self) {
        let mut inner = self.inner.lock();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                unsafe { PROC_MANAGER.task_wakeup(task as _) };
            }
        }
    }

    pub fn down(&self) {
        let mut inner = self.inner.lock();
        inner.count -= 1;
        while inner.count < 0 {
            let task =  unsafe { CPU_MANAGER.my_task() as *const Task};
            inner.wait_queue.push_back(task);  
            let channel = task as usize;
            let task = unsafe { task.as_ref().unwrap() };
            task.sleep(channel, inner);
            inner = self.inner.lock();
        }
    }
}