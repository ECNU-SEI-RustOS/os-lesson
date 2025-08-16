use alloc::collections::VecDeque;

use super::Process;

use lazy_static::lazy_static;

pub struct ProcessFIFO {
    current: Option<*const Process>,
    ready_queue: VecDeque<*const Process>,
}

unsafe impl Sync for ProcessFIFO {}
unsafe impl Send for ProcessFIFO {}
/// A simple FIFO scheduler.
impl ProcessFIFO {
    pub const fn new() -> Self {
        Self {
            current: None,
            ready_queue: VecDeque::new(),
        }
    }
    pub fn add(&mut self, process: *const Process) {
        self.ready_queue.push_back(process);
    }
    pub fn fetch(&mut self) -> Option<*const Process> {
        self.ready_queue.pop_front()
    }
}
