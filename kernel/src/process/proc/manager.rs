use alloc::collections::VecDeque;

use super::Task;

use lazy_static::lazy_static;

pub struct ProcessFIFO {
    current: Option<*const Task>,
    ready_queue: VecDeque<*const Task>,
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
    pub fn add(&mut self, process: *const Task) {
        self.ready_queue.push_back(process);
    }
    pub fn fetch(&mut self) -> Option<*const Task> {
        self.ready_queue.pop_front()
    }
}
