#![no_std]
#![no_main]

use syscall_riscv::{sys_gettid, sys_thread_count, sys_thread_waittid};
use user_rust_lib::{exit, kernel_thread::thread_create};

#[macro_use]
extern crate user_rust_lib;
static mut COUNTER: u32 = 0;
static string: &str = "1ych";
pub fn f(arg: usize){
    let mut a = 0;
    for i in 0..10{
        println!("{}","stringych");
        a = i;
       unsafe { COUNTER += i;}
    }
    println!("tid:{}",sys_gettid());
    exit(arg as i32);
}
pub fn f2(arg: usize){
    let mut a = 0;
    for i in 0..100{
        println!("{}","welcome shanghai");
        a = i;
       unsafe { COUNTER += i;}
    }
    println!("tid:{}",sys_gettid());
    exit(17);
}
#[no_mangle]
fn main(argc:usize, argv:&[&str]) -> i32 {
    let tid2 = thread_create(f2, 17);
    println!("hello world by kernel thread {}", thread_create(f, 17));
    println!("hello world by kernel thread {}",tid2);
    let  mut a= 0;
    let code = sys_thread_waittid(tid2 as usize);
    println!("exit code:{}",code);
    for i in 0..10{
        println!("{}",unsafe { COUNTER });
        a += 1;
    }
    println!("thread_count: {}", unsafe {
        sys_thread_count()
    });

    0
}