use alloc::vec::Vec;

pub struct RecycleAllocator{
    current: usize,
    recycled: Vec<usize>
}

impl RecycleAllocator {
    pub fn new() -> Self {
        RecycleAllocator { current: 0, recycled: Vec::new() }
    }

    pub fn tid_alloc(&mut self) -> usize {
        if let Some(pid) = self.recycled.pop() {
            pid
        }
        else {
            self.current += 1;
            self.current - 1
        }
    }
    pub fn tid_dealloc(&mut self, tid: usize) {
        debug_assert!(tid < self.current);
        debug_assert!(
            !self.recycled.iter().any(|i| *i ==  tid),
            "pid {} has been deallocated!",
            tid
        );
        self.recycled.push(tid);
    }
}