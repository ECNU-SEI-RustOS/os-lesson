use syscall_riscv::{sys_close, sys_dup, sys_open, sys_pipe, sys_read, sys_write};
use bitflags::*;

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}

pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}
pub fn pipe(pipe_fd: &mut [u32]) -> isize {
    sys_pipe(pipe_fd.as_mut_ptr() as *mut u32)
}


pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}
pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}
pub fn open(path: &str, flag: OpenFlags) -> isize {
    sys_open(path, flag.bits())
}
pub fn close(fd: usize) -> isize {
    sys_close(fd)
}