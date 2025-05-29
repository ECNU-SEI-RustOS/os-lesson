#![no_std]
#![no_main]

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
fn main(argc:usize, argv:&[&str]) -> i32 {
    println!("hello world by rust, {}",argv[1]);
   0
}