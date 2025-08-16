#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(allow_internal_unstable)]
#[macro_use]
pub mod macros;
mod panic;
pub mod task;
pub mod io;
pub mod ralloc;
pub mod file;
pub mod time;
pub mod thread;
pub mod kernel_thread;

extern crate alloc;
extern crate syscall_riscv;

use syscall_riscv::*;
use alloc::vec::Vec;
use buddy_system_allocator::LockedHeap;


const USER_HEAP_SIZE: usize = 0x10000;

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
    let mut v: Vec<&'static str> = Vec::new();
    for i in 0..argc{
        let str_start = unsafe {
            ((argv + i * core::mem::size_of::<usize>()) as * const usize).read_volatile()
        };
        let len = (0usize..)
            .find(|i| unsafe{((str_start + *i) as *const u8).read_volatile() == 0})
            .unwrap();
        v.push(
            core::str::from_utf8(
                unsafe {
                    core::slice::from_raw_parts(str_start as *const u8, len)
                }
            )
            .unwrap()
        );
    }
    exit(main(argc,v.as_slice()));
}

#[linkage = "weak"]
#[no_mangle]
fn main(_argc:usize, _argv:&[&str]) -> i32 {
    panic!("Cannot find main!");
}
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code)
}

