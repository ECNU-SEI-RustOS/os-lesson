#![no_std]
#![no_main]

use user_rust_lib::{exit, file::{pipe, read}, task::{fork, getpid, sleep, wait}};

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
pub fn main() -> i32 {
    let mut p1 = [0; 2];
    let mut p2 = [0; 2];
    let mut buf = [0u8; 10];

    pipe(&mut p1);
    pipe(&mut p2);

    if fork() == 0 {
        // child
        if read(p2[0] as isize, &mut buf) != 0 {
            println!("{}: received ping", getpid());
            user_rust_lib::file::write(p1[1] as isize, b"CHILD");
        }
        exit(0)
    } else {
        // parent
        user_rust_lib::file::write(p2[1] as isize, b"PARENT");
        if read(p1[0] as isize, &mut buf) != 0 {
            wait(&mut 0);
            println!("{}: received pong", getpid());
        }
        exit(0);
    }
}