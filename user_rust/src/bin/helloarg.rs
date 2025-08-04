#![no_std]
#![no_main]

use user_rust_lib::{io::getchar, task::sleep, time::get_mtime};

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
fn main(argc:usize, argv:&[&str]) -> i32 {
    println!("hello world by rust, {}",argv[1]);
    for i in 0..4{
        sleep(10);
        println!("sleep {}s",i);
        println!("{}",get_mtime());
    }
    
   0
}