use syscall_riscv::{sys_uptime,sys_getmtime};

pub fn get_uptime() -> usize {
    sys_uptime() as usize
}

pub fn get_mtime() -> usize {
    sys_getmtime() as usize
}