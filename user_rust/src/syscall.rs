use core::{arch::asm};
const SYSCALL_FORK:usize = 1;
const SYSCALL_EXIT:usize = 2;
const SYSCALL_WAIT:usize = 3;
const SYSCALL_PIPE:usize = 4;
const SYSCALL_READ:usize = 5;
const SYSCALL_KILL:usize = 6;
const SYSCALL_EXEC:usize = 7;
const SYSCALL_GETPID:usize = 11;
const SYSCALL_WRITE:usize = 16;

const SYSCALL_TEST: usize = 99;
fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }
    ret
}

pub fn sys_exit(no: i32) -> ! {
    syscall(SYSCALL_EXIT, [no as usize,0,0]);
    panic!("sys_exit never return");
}

pub fn sys_read(fd:usize, buffer: &mut [u8])-> isize{
    syscall(SYSCALL_READ, [fd, buffer.as_ptr() as usize, buffer.len()])
}
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}


pub fn sys_kill(pid: isize) -> isize {
    syscall(SYSCALL_KILL, [pid as usize, 0, 0])
}

pub fn sys_getpid() -> isize {
    syscall(SYSCALL_GETPID, [0, 0, 0])
}