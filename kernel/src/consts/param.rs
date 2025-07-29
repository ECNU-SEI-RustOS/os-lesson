/// This is just a maximum used to allocate memory space.
pub const NCPU: usize = 8;

/// Maximum number of processes
pub const NPROC: usize = 512;

/// Maximum number of threads
pub const NTHREAD: usize = 2048;

pub const MAX_TASKS_PER_PROC: usize = 512;

/// This is actual number of harts.
/// Same value is passed to qemu with -smp option
pub const NSMP: usize = 3;

/// memory design
pub const PAGE_SIZE: usize = 0x1000;
pub const PGSHIFT: usize = 12;
pub const PGMASK: usize = 0x1FF;
pub const PGMASKLEN: usize = 9;
pub const PAGE_SIZE_BITS: usize = 0xc;

// 内核栈大小
pub const KERNEL_STACK_SIZE: usize = PAGE_SIZE * 4;
pub const USER_STACK_SIZE: usize = PAGE_SIZE * 4;
/// for syscall
/// maximum length of a file system path
pub const MAXPATH: usize = 128;
/// maximum number of command line arguments
pub const MAXARG: usize = 16;
/// maximum length of a single command line argument
pub const MAXARGLEN: usize = 64;

/// The smallest block size of the buddy system
pub const LEAF_SIZE: usize = 16;