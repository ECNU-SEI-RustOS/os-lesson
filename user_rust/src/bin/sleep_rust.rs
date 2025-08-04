#![no_std]
#![no_main]

use user_rust_lib::task::sleep;

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
fn main(argc:usize, argv:&[&str]) -> i32 {
    sleep(10);
    0
}