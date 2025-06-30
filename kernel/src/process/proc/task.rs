//! thread in the os kernel 

use alloc::{sync::Weak, vec::Vec};

pub enum TaskStatus {
    Read,
    Running,
    Blocked,
}
pub struct Task{
    pub is_zombie: bool,
    pub status:TaskStatus,
    pub exit_code:i32,
}
pub struct TaskData {
    
}