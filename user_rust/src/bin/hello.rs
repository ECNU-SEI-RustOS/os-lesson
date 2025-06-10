#![no_std]
#![no_main]

use user_rust_lib::{file::{close, open, read, write, OpenFlags}, ralloc::sbrk, task::sleep};

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
    // unsafe {
    //     core::arch::asm!("csrrw x0, mstatus, x0"); // 读写 `mstatus` CSR（特权指令）
    // }

    let file = open("ft\0", OpenFlags::CREATE | OpenFlags::WRONLY) as usize;
    let string = "ych";
    write(file, string.as_bytes());
    close(file);

    let fd = open("ft\0", OpenFlags::RDONLY);
    let mut buffer = [0u8; 100];
    let read_len =read(file, &mut buffer) as usize;
    let res = core::str::from_utf8(&buffer[..read_len]).unwrap();
    println!("{}",res);
    close(file as usize);
    0
}   