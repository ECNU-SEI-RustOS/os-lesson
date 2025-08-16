#![no_std]
#![no_main]

use core::ops::Add;

use syscall_riscv::{sys_gettid, sys_semaphore_create, sys_semaphore_down, sys_semaphore_up, sys_sleep, sys_thread_count, sys_thread_waittid};
use user_rust_lib::{exit, kernel_thread::thread_create};
use user_rust_lib::task::sleep;
use core::ptr::addr_of_mut;
#[macro_use]
extern crate user_rust_lib;
static mut COUNTER: u32 = 0;
const SEM_SYNC: usize = 0;
fn first(arg:usize){
    let a = addr_of_mut!(COUNTER);
    for i in 0..1000{
        sys_semaphore_down(SEM_SYNC);
        unsafe { a.write_volatile( *a +1 ) };
        sys_semaphore_up(SEM_SYNC);
    }

    exit(0)
}

fn second(arg:usize){
    let a = addr_of_mut!(COUNTER);
    for i in 0..1000{
        sys_semaphore_down(SEM_SYNC);
        unsafe { a.write_volatile( *a +1 ) };
        sys_semaphore_up(SEM_SYNC);
    }
    exit(0)
}

#[no_mangle]
fn main(argc:usize, argv:&[&str]) -> i32 {
    assert_eq!(sys_semaphore_create(1) as usize, SEM_SYNC);
    // create threads

    let tid1 = thread_create(first, 17);
    let tid2 = thread_create(second, 17);

    let code = sys_thread_waittid(tid1 as usize);
    let code = sys_thread_waittid(tid2 as usize);
    println!("count:{}",unsafe { COUNTER });
    0
}