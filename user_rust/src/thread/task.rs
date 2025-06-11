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
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)] // not strictly needed but Rust ABI is not guaranteed to be stable
pub struct TaskContext {
    // 15 u64
    pub x1: u64,  //ra: return address
    pub x2: u64,  //sp
    pub x8: u64,  //s0,fp
    pub x9: u64,  //s1
    pub x18: u64, //x18-27: s2-11
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub nx1: u64, //new return address
    pub r_ptr: u64 // self ptr
}

pub struct Task {
    pub id: usize,
    pub stack: Vec<u8>,
    pub ctx: TaskContext,
    pub state: TaskState,
    pub r_ptr: u64
}



impl Task {
    pub fn new(id: usize) -> Self {
        Task {
            id: id,
            stack: vec![0u8; DEFAULT_STACK_SIZE],
            ctx: TaskContext::default(),
            state: TaskState::Available,
            r_ptr: 0
        }
    }
}

