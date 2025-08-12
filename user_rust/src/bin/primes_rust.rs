#![no_std]
#![no_main]

use user_rust_lib::{exit, file::{close, pipe, read}, task::{fork, wait}};

#[macro_use]
extern crate user_rust_lib;

#[no_mangle]
pub fn main(){
    let mut in_pipe = [0u32; 2];
    let _ = pipe(&mut in_pipe);

    if fork() == 0 {
        let _ = close(in_pipe[1] as isize);
        pipeline(in_pipe);
    } else {
        let _ = close(in_pipe[0] as isize);
        for i in 2..35 {
            let n = i as u32;
            let _ = user_rust_lib::file::write(in_pipe[1] as isize, &n.to_ne_bytes());
        }
        let _ = close(in_pipe[1] as isize);
        let _ = wait(&mut 0);
    }

    exit(0);
}

fn pipeline(in_pipe: [u32; 2]) {
    let mut out_pipe = [0u32; 2];
    let _ = pipe(&mut out_pipe);

    let mut buf = [0u8; 4];
    let mut head = 0u32;

    if read(in_pipe[0] as isize, &mut buf) == 0 {
        return;
    }
    head = u32::from_ne_bytes(buf);
    if head >= 35 {
        return;
    }

    if fork() == 0 {
        let _ = close(out_pipe[1] as isize);
        pipeline(out_pipe);
    } else {
        println!("prime {}", head);
        loop {
            let nread = read(in_pipe[0] as isize, &mut buf);
            if nread == 0 {
                break;
            }
            let val = u32::from_ne_bytes(buf);
            if val % head != 0 {
                let _ = user_rust_lib::file::write(out_pipe[1] as isize, &val.to_ne_bytes());
            }
        }
        let _ = close(in_pipe[0] as isize);
        let _ = close(out_pipe[1] as isize);
        let _ = wait(&mut 0);
    }
}
