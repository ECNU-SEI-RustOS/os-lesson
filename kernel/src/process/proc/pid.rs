use alloc::vec::Vec;

use lazy_static::*;
use crate::spinlock::SpinLock;
lazy_static!{
    pub static ref PID_ALLOCATOR: SpinLock<RecycleAllocator> = SpinLock::new(RecycleAllocator::new(),"pid_allocator");
}
pub struct RecycleAllocator{
    current: usize,
    recycled: Vec<usize>
}

impl RecycleAllocator {
    pub fn new() -> Self {
        RecycleAllocator { current: 0, recycled: Vec::new() }
    }

    pub fn pid_alloc(&mut self) -> usize {
        let mut res = 0;
        if let Some(pid) = self.recycled.pop() {
            res = pid;
        }
        else {
            self.current += 1;
            res = self.current - 1;
        }
        res
    }
    pub fn pid_dealloc(&mut self, pid: usize) {
        debug_assert!(pid < self.current);
        debug_assert!(
            !self.recycled.iter().any(|i| *i ==  pid),
            "pid {} has been deallocated!",
            pid
        );
        self.recycled.push(pid);
    }
}