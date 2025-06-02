use syscall_riscv::{sys_exec, sys_fork, sys_getpid, sys_kill, sys_wait, sys_sleep};

pub fn fork() -> isize {
    sys_fork()
}

pub fn kill(pid: isize) -> isize{
    sys_kill(pid)
}
pub fn exec(path: &str, args: &[*const u8]) -> isize {
    sys_exec(path,args)
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn wait(exit_code: &mut i32) -> isize {
    sys_wait(exit_code as *mut i32 as *mut usize)
}

pub fn sleep(sleep_ms: usize) {
    sys_sleep(sleep_ms);
}