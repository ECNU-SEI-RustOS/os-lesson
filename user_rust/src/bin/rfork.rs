#![no_std]
#![no_main]

use user_rust_lib::{task::{fork, sleep, wait}, time::get_mtime};

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
fn main(argc:usize, argv:&[&str]) -> i32 {
    let  s= 12;
    let mut exit_code :i32 = 0;
    if fork() != 0 {
        println!("hello world by rust from parent");
        for i in 0..4{
            sleep(10);
            println!("sleep {}s",i);
            println!("{}",get_mtime());
        }
        wait(&mut exit_code);
    } else{
        println!("hello world by rust from child");
    }
    
    0
}