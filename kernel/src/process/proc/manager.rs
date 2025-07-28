use alloc::collections::VecDeque;
use super::Process;

pub struct ProcessFIFO {
    current: Option<*mut Process>,
    ready_queue: VecDeque<*mut Process>
}

unsafe impl Sync for ProcessFIFO {
}
/// A simple FIFO scheduler.
impl ProcessFIFO {
    pub const fn new() -> Self {
        Self {
            current: None,
            ready_queue: VecDeque::new(),
        }
    }
    pub fn add(&mut self, process: *mut Process) {
        self.ready_queue.push_back(process);
    }
    pub fn fetch(&mut self) -> Option<*mut Process> {
        let proc = self.ready_queue.pop_front();
        self.current = proc.clone();
        proc
    }
    pub fn remove(&mut self, process: *mut Process) {
        if let Some((id, _)) = self
            .ready_queue
            .iter()
            .enumerate()
            .find(|(_, t)| **t as usize == process as usize)
        {
            self.ready_queue.remove(id);
        }
    }
}