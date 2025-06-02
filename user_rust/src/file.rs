use syscall_riscv::{sys_dup,sys_pipe};

pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}
pub fn pipe(pipe_fd: &mut [u32]) -> isize {
    sys_pipe(pipe_fd.as_mut_ptr() as *mut u32)
}