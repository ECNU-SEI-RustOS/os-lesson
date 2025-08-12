use alloc::collections::VecDeque;
use alloc::sync::Arc;

use crate::process::task::task::{Task, TaskStatus};
use crate::spinlock::SpinLock;
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

pub struct TaskManager {
    ready_queue: VecDeque<Arc<Task>>,
}

unsafe impl Sync for TaskManager {}
unsafe impl Send for TaskManager {}

lazy_static!{
    pub static ref TASK_MANAGER:SpinLock<TaskManager> =  SpinLock::new(TaskManager::new(),"taskmanager");
}
impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    pub fn add(&mut self, task: Arc<Task>) {
        self.ready_queue.push_back(task);
    }
    pub fn fetch(&mut self) -> Option<Arc<Task>> {
        self.ready_queue.pop_front()
    }
}

pub fn add_task(task: Arc<Task>) {
    TASK_MANAGER.lock().add(task);
}

pub fn wakeup_task(task: Arc<Task>) {
    task.set_status(TaskStatus::Ready);
    add_task(task);
}

pub fn fetch_task() -> Option<Arc<Task>> {
    TASK_MANAGER.lock().fetch()
}
