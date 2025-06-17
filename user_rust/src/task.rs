use syscall_riscv::{sys_chdir, sys_exec, sys_fork, sys_getpid, sys_kill, sys_sleep, sys_wait, sys_waitpid};

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

pub fn sleep(ticks: usize) {
    sys_sleep(ticks);
}

pub fn chdir(working_dir:&str) -> isize {
    sys_chdir(working_dir)
}

pub fn waitpid(pid: isize, exit_code: &mut i32) -> isize {
    sys_waitpid(pid, exit_code as *mut i32)
}