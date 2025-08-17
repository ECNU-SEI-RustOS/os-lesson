use syscall_riscv::{sys_get_task_exitstatus, sys_gettid, sys_thread_count, sys_thread_create, sys_thread_waittid};

/// 在当前进程（主线程）中创建新线程
pub fn thread_create(f: fn(usize), arg: usize) -> isize {
    sys_thread_create(f as usize, arg)
}

/// 获取当前进程（主线程）中的线程数
pub fn thread_count() -> isize {
    sys_thread_count()
}
/// 获取当前线程的线程号
pub fn gettid() -> isize {
    sys_gettid()
}
/// 等待指定线程结束，获取线程的返回值
pub fn waittid(tid: isize) -> isize {
    sys_thread_waittid(tid as usize)
}

/// 获取线程的退出值
pub fn get_task_exitstatus(tid: isize) -> isize {
    sys_get_task_exitstatus(tid as usize)
}