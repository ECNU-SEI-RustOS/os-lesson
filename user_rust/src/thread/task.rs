use alloc::vec;
use alloc::vec::Vec;
use super::config::*;


#[derive(PartialEq, Eq, Debug)]
pub enum TaskState {
    Available,
    Ready,
    Running,
    Blocked,
    Dead,
}
#[derive(Debug, Default)]
#[repr(C)] // not strictly needed but Rust ABI is not guaranteed to be stable
pub struct TaskContext {
    // 15 u64
    x1: u64,  //ra: return address
    x2: u64,  //sp
    x8: u64,  //s0,fp
    x9: u64,  //s1
    x18: u64, //x18-27: s2-11
    x19: u64,
    x20: u64,
    x21: u64,
    x22: u64,
    x23: u64,
    x24: u64,
    x25: u64,
    x26: u64,
    x27: u64,
    nx1: u64, //new return address
}

pub struct Task {
    pub id: usize,
    pub stack: Vec<u8>,
    pub ctx: TaskContext,
    pub state: TaskState,
}



impl Task {
    pub fn new(id: usize) -> Self {
        Task {
            id: id,
            stack: vec![0u8; DEFAULT_STACK_SIZE],
            ctx: TaskContext::default(),
            state: TaskState::Available,
        }
    }
}

