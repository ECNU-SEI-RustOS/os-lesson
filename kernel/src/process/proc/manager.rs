use alloc::collections::VecDeque;

use super::Task;

use lazy_static::lazy_static;

pub struct TaskFIFO {
    current: Option<*const Task>,
    ready_queue: VecDeque<*const Task>,
}

unsafe impl Sync for TaskFIFO {}
unsafe impl Send for TaskFIFO {}
/// A simple FIFO scheduler.
impl TaskFIFO {
    pub const fn new() -> Self {
        Self {
            current: None,
            ready_queue: VecDeque::new(),
        }
    }
    pub fn add(&mut self, task: *const Task) {
        self.ready_queue.push_back(task);
    }
    pub fn fetch(&mut self) -> Option<*const Task> {
        self.ready_queue.pop_front()
    }
}
