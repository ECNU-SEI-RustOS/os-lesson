//! 文件系统模块

use core::ops::DerefMut;

mod file;
mod inode;
mod log;
mod bio;
mod block;
mod superblock;

// TODO - Buf also could?
pub use bio::Buf;
// TODO - could be reduced to use xxx after removing usage from rmain.rs
pub use bio::BCACHE;
pub use inode::{ICACHE, Inode, InodeData, InodeType, FileStat};
pub use log::LOG;
pub use file::{File, Pipe};

use superblock::SUPER_BLOCK;
use log::Log;
use bio::BufData;
use inode::icheck;

/// 初始化文件系统，读取磁盘超级块信息，并根据需要进行日志恢复
/// 安全性：必须在系统启动时被调用一次
pub unsafe fn init(dev: u32) {
    SUPER_BLOCK.init(dev);
    let log_ptr = LOG.lock().deref_mut() as *mut Log;
    log_ptr.as_mut().unwrap().init(dev);
    icheck();
    println!("file system: setup done");

    #[cfg(feature = "verbose_init_info")]
    println!("file system: {} inode per block with size {}", inode::IPB, crate::consts::fs::BSIZE);
}
