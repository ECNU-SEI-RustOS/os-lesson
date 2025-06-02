#![no_std]
#![no_main]

use user_rust_lib::{ralloc::sbrk, task::sleep};

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
fn main() -> i32 {
    println!("hello world by rust{}",1);
    unsafe {
    let addr = sbrk(0) as usize;
    println!("{:08x}",addr);
    let addr = sbrk(100) as usize;
    println!("{:08x}",addr);
    let addr = sbrk(-100) as usize;
    println!("{:08x}",addr);
    }
    unsafe {
        core::arch::asm!("csrrw x0, mstatus, x0"); // 读写 `mstatus` CSR（特权指令）
    }
    0
}   