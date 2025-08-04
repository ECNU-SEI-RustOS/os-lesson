use syscall_riscv::{sys_sbrk};

pub unsafe fn sbrk(increment: i32) -> isize {
    sys_sbrk(increment)
}