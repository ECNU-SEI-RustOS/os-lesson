#![no_std]
/// the syscall on RISCV chips which support 6 parameters
pub fn syscall(id: usize, args: [usize; 6]) -> isize {
    let mut ret: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x13") args[3],
            in("x14") args[4],
            in("x15") args[5],
            in("x17") id
        );
    }
    ret
}

const SYSCALL_FORK: usize = 1;
const SYSCALL_EXIT: usize = 2;
const SYSCALL_WAIT: usize = 3;
const SYSCALL_PIPE: usize = 4;
const SYSCALL_READ: usize = 5;
const SYSCALL_KILL: usize = 6;
const SYSCALL_EXEC: usize = 7;
const SYSCALL_DUP: usize = 10;
const SYSCALL_GETPID: usize = 11;
const SYSCALL_SBRK: usize = 12;
const SYSCALL_SLEEP: usize = 13;
const SYSCALL_WRITE: usize = 16;

const SYSCALL_TEST: usize = 99;

///进程 A 调用 fork 系统调用之后，内核会创建一个新进程 B，这个进程 B 和调用 fork 的进程A在它们分别返回用户态那一瞬间几乎处于相同的状态：这意味着它们包含的用户态的代码段、堆栈段及其他数据段的内容完全相同，但是它们是被放在两个独立的地址空间中的。因此新进程的地址空间需要从原有进程的地址空间完整拷贝一份。两个进程通用寄存器也几乎完全相同。
pub fn sys_fork() -> isize {
    syscall(SYSCALL_FORK, [0, 0, 0, 0, 0, 0])
}

pub fn sys_exit(no: i32) -> ! {
    syscall(SYSCALL_EXIT, [no as usize, 0, 0, 0, 0, 0]);
    panic!("sys_exit never return");
}

pub fn sys_wait(addr: *mut usize) -> isize {
    syscall(SYSCALL_WAIT, [addr as usize, 0, 0, 0, 0, 0])
}
pub fn sys_pipe(addr: *mut u32) -> isize {
    syscall(SYSCALL_PIPE, [addr as usize, 0, 0, 0, 0, 0])
}
pub fn sys_read(fd: usize, buffer: &mut [u8]) -> isize {
    syscall(
        SYSCALL_READ,
        [fd, buffer.as_ptr() as usize, buffer.len(), 0, 0, 0],
    )
}
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall(
        SYSCALL_WRITE,
        [fd, buffer.as_ptr() as usize, buffer.len(), 0, 0, 0],
    )
}

pub fn sys_kill(pid: isize) -> isize {
    syscall(SYSCALL_KILL, [pid as usize, 0, 0, 0, 0, 0])
}

/// 功能：将当前进程的地址空间清空并加载一个特定的可执行文件，返回用户态后开始它的执行。
/// 参数：path 给出了要加载的可执行文件的名字；
/// 返回值：如果出错的话（如找不到名字相符的可执行文件）则返回 -1，否则不应该返回。
pub fn sys_exec(path: &str, args: &[*const u8]) -> isize {
    syscall(
        SYSCALL_EXEC,
        [path.as_ptr() as usize, args.as_ptr() as usize, 0, 0, 0, 0],
    )
}
pub fn sys_dup(fd: usize) -> isize {
    syscall(SYSCALL_DUP, [fd, 0, 0, 0, 0, 0])
}
pub fn sys_getpid() -> isize {
    syscall(SYSCALL_GETPID, [0, 0, 0, 0, 0, 0])
}
pub fn sys_sbrk(increment: i32) -> isize {
    syscall(SYSCALL_SBRK, [increment as usize, 0, 0, 0, 0, 0])
}

pub fn sys_sleep(sleep_ms: usize) -> isize {
    syscall(SYSCALL_SLEEP, [sleep_ms as usize, 0, 0, 0, 0, 0])
}