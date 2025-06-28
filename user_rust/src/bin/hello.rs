#![no_std]
#![no_main]

use user_rust_lib::{file::{close, open, read, write, OpenFlags, Stat}, ralloc::sbrk, task::sleep};
use user_rust_lib::file::{fstat};
use user_rust_lib::thread::*;
#[macro_use]
extern crate user_rust_lib;
#[derive(Clone, Copy)]
pub struct MyType{
    id: u32,
    str: &'static str
}
impl MyType {
    fn new(id:u32,str:&'static str)->Self{
        Self { 
            id:id,
            str:str
        }
    }
}
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

    let file = open("ft\0", OpenFlags::CREATE | OpenFlags::WRONLY);
    let string = "ych";
    write(file, string.as_bytes());
    close(file);

    let fd = open("ft\0", OpenFlags::RDONLY);
    let mut buffer = [0u8; 100];
    let read_len =read(file, &mut buffer);
    let res = core::str::from_utf8(&buffer[..read_len as usize]).unwrap();
    println!("{}",res); 
    
    
    let mut a = Stat::default();
    fstat(fd,&mut a);
    println!("{:?}",a);close(file);

    println!("stackful_coroutine begin...");
    println!("TASK 0 (Runtime) STARTING");
    let mut runtime = Runtime::new();

    let r_ptr = runtime.init();
    println!("r_ptr:{:x}",r_ptr);
    let args1 = MyType::new(12, "ych");
    let args2 = MyType::new(17, "kss");
    runtime.spawn(|r_ptr, args | {
        println!("TASK  1 STARTING");
        let id = 1;
        let arg =  args as *const MyType;
        
        let para = unsafe {*arg};
        for i in 0..4 {
            println!("task: {} counter: {} arg:{}", id, i, para.str);
            yield_task(r_ptr);
        }
        println!("TASK 1 FINISHED");
    },&args1 as *const MyType as u64);
    runtime.spawn(|r_ptr, args| {
        println!("TASK 2 STARTING");
        let id = 2;
        let arg =  args as *const MyType;
        
        let para = unsafe {*arg};
        for i in 0..8 {
            println!("task: {} counter: {} arg:{}", id, i, para.str);
            yield_task(r_ptr);
        }
        println!("TASK 2 FINISHED");
    },&args2 as *const MyType as u64);
    // runtime.spawn(|r_ptr| {
    //     println!("TASK 3 STARTING");
    //     let id = 3;
    //     for i in 0..12 {
    //         println!("task: {} counter: {}", id, i);
    //         yield_task(r_ptr);
    //     }
    //     println!("TASK 3 FINISHED");
    // });
    // runtime.spawn(|r_ptr| {
    //     println!("TASK 4 STARTING");
    //     let id = 4;
    //     for i in 0..16 {
    //         println!("task: {} counter: {}", id, i);
    //         yield_task(r_ptr);
    //     }
    //     println!("TASK 4 FINISHED");
    // });`
    runtime.run();
    println!("stackful_coroutine PASSED");

    0
}   