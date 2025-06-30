#![no_std]
#![no_main]

use user_rust_lib::{task::{fork, sleep, wait, waitpid}, time::get_mtime};

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
fn main(argc:usize, argv:&[&str]) -> i32 {
    let  s= 12;
    let mut exit_code :i32 = 0;
    let mut child_pid :isize;
    child_pid = fork();
    if child_pid != 0 {
        
        waitpid(child_pid,&mut exit_code);
        //wait(&mut exit_code);
        println!("hello world by rust from parent");
    } else{
        println!("hello world by rust from child");
        for i in 0..4{
            sleep(10);
            println!("sleep {}s",i);
            println!("{}",get_mtime());
        }
    }
    
    0
}