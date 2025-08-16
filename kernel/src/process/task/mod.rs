//! kernel thread
mod context;
pub mod tid;

use crate::consts::TRAPFRAME;
use crate::process::PAGE_SIZE;
use crate::consts::ConstAddr;
use crate::process::NPROC;
/// get the trapframe ptr in user space by tid
#[inline]
pub fn trapframe_from_tid(tid: usize) -> ConstAddr {
    TRAPFRAME.const_sub((NPROC + 1) * (PAGE_SIZE + PAGE_SIZE) + tid * (PAGE_SIZE + PAGE_SIZE))
}