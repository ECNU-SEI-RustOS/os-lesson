# 用户程序-rust
使用rust语言编写用户程序。
已经部分实现基础库函数。

## 用户程序编译方法

```
make build
```


## rust用户程序

1.  用户程序入口  
    通过link.ld进行用户代码链接。  
    用户程序入口函数fn `_start(argc: usize, argv: usize) -> !`
    ```
    // src/link.ld
    
    OUTPUT_ARCH(riscv)
    ENTRY(_start)

    BASE_ADDRESS = 0x10000;

    SECTIONS
    {
        . = BASE_ADDRESS;
        .text : {
            *(.text.entry)
            *(.text .text.*)
        }
        . = ALIGN(4K);
        .rodata : {
            *(.rodata .rodata.*)
            *(.srodata .srodata.*)
        }
        . = ALIGN(4K);
        .data : {
            *(.data .data.*)
            *(.sdata .sdata.*)
        }
        .bss : {
            *(.bss .bss.*)
            *(.sbss .sbss.*)
        }
        /DISCARD/ : {
            *(.eh_frame)
            *(.debug*)
        }
    }
    ```

2.  用户栈中的堆内存分配  
    基于buddy_system_allocator库实现的用户栈中内存分配器初始化配置
    ```
    use buddy_system_allocator::LockedHeap;

    const USER_HEAP_SIZE: usize = 0x10000;

    static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

    #[global_allocator]
    static HEAP: LockedHeap = LockedHeap::empty();
    ```
    ​​`#[global_allocator]`​​
    将此分配器注册为Rust的全局分配器，所有标准库类型（如Vec、Box）的内存分配会通过它进行。

    ***后续可升级为内核中的堆内存分配***
 

3.  `_start`  
    1.初始化用户栈中的堆内存  
    2.从内核初始化的进程中用户栈中获取参数`argc: usize, argv: usize`  
    *操作系统内核已经将参数压入用户栈中，此将符合C语言标准的参数转换为符合Rust语言标准的参数（处理字符串）*
    ```
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
    ```
    3.执行用户编写的主函数`fn main(argc:usize, argv:&[&str]) -> i32`
    4.退出，调用`exit`系统调用，保存返回值。

    ```
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

    ```

# 模块

## 基础系统调用
1. syscall(riscv处理器)
    在RISCV处理器上的系统调用，此操作系统最多支持**6个**参数。  
    通过rust语言内嵌汇编实现。

    ```
    // src/syscall_riscv/src/lib.rs

    /// the syscall on RISCV chips which supports 6 parameters
    pub fn syscall(id: usize, args: [usize; 6]) -> isize {
        let mut ret: isize;
        unsafe {
            core::arch::asm!(
                "ecall",
                inlateout("x10") args[0] => ret,
                in("x11") args[1],
                in("x12") args[2],
                in("x13") args[3],
                in("x14") args[4],
                in("x15") args[5],
                in("x17") id
            );
        }
        ret
    }
    ```
2. 系统调用详细信息
- [x] SYSCALL_FORK  
    功能：创建当前进程的一个子进程，复制父进程的内存、TrapFrame、打开文件、当前工作目录等信息，并将子进程状态设置为可运行。
    参数：无。  
    返回值：​  
        *child_pid(>0)​*：​​父进程中返回子进程的 PID​​（Process ID），用于后续管理子进程  
        *0*​：子进程中返回 0​​，用于确认当前是子进程    
        *-1*：错误则返回 -1。可能的错误原因是：复制过程中出现错误  

- [x] SYSCALL_FORK(syscall_id = 1)
- [x] SYSCALL_EXIT(syscall_id = 2)
- [x] SYSCALL_WAIT(syscall_id = 3)
- [x] SYSCALL_PIPE(syscall_id = 4)
- [x] SYSCALL_READ(syscall_id = 5)
- [x] SYSCALL_KILL(syscall_id = 6)
- [x] SYSCALL_EXEC(syscall_id = 7)
- [ ] SYSCALL_FSTAT(syscall_id = 8)
- [ ] SYSCALL_CHDIR(syscall_id = 9)
- [x] SYSCALL_DUP(syscall_id = 10)
- [x] SYSCALL_GETPID(syscall_id = 11)
- [x] SYSCALL_SBRK(syscall_id = 12)
- [x] SYSCALL_SLEEP(syscall_id = 13)
- [x] SYSCALL_UPTIME:usize = 14;
- [x] SYSCALL_OPEN: usize = 15;
- [x] SYSCALL_WRITE: usize = 16;
- [x] SYSCALL_MKNOD: usize = 17;
- [x] SYSCALL_UNLINK: usize = 18;
- [x] SYSCALL_LINK: usize = 19;
- [x] SYSCALL_MKDIR: usize = 20;
- [x] SYSCALL_CLOSE: usize = 21;
- [x] SYSCALL_GETMTIME: usize = 22;
- [x] SYSCALL_TEST: usize = 99;

- [x] SYSCALL_EXIT
- [x] SYSCALL_WAIT
- [x] SYSCALL_PIPE  
    功能：为当前进程打开一个管道。  
    参数：pipe 表示应用地址空间中的一个长度为 2 的 usize 数组的起始地址，内核需要按顺序将管道读端和写端的文件描述符写入到数组中。  
    返回值：出现了错误则返回 -1。可能的错误原因是：传入的地址不合法。  

- [x] SYSCALL_READ
- [x] SYSCALL_KILL  
    功能：当前进程向另一个进程（可以是自身）发送一个信号。  
    参数：pid 表示接受信号的进程的进程 ID, signum 表示要发送的信号的编号。  
    返回值：如果传入参数不正确（比如指定进程或信号类型不存在）则返回 -1 ,否则返回 0 。  

- [x] SYSCALL_EXEC  
    功能：将当前进程的地址空间清空并加载一个特定的可执行文件，返回用户态后开始它的执行。  
    参数：path 给出了要加载的可执行文件的名字；  
    返回值：如果出错的话（如找不到名字相符的可执行文件）则返回 -1，否则不应该返回。 
## 用户线程库(全部在用户空间实现)
用户空间线程库（User-Level Thread Library）是一种在用户态实现线程管理的机制，与操作系统内核管理的线程（内核线程）不同。用户线程的创建、调度、同步等操作完全由库在用户空间处理，无需频繁陷入内核，从而减少上下文切换的开销

1. 用户空间线程线程切换核心

    ```
    // 
    switch: 
        sd x1, 0x00(a0)
        sd x2, 0x08(a0)
        sd x8, 0x10(a0)
        sd x9, 0x18(a0)
        sd x18, 0x20(a0)
        sd x19, 0x28(a0)
        sd x20, 0x30(a0)
        sd x21, 0x38(a0)
        sd x22, 0x40(a0)
        sd x23, 0x48(a0)
        sd x24, 0x50(a0)
        sd x25, 0x58(a0)
        sd x26, 0x60(a0)
        sd x27, 0x68(a0)
        sd x1, 0x70(a0)

        ld x1, 0x00(a1)
        ld x2, 0x08(a1)
        ld x8, 0x10(a1)
        ld x9, 0x18(a1)
        ld x18, 0x20(a1)
        ld x19, 0x28(a1)
        ld x20, 0x30(a1)
        ld x21, 0x38(a1)
        ld x22, 0x40(a1)
        ld x23, 0x48(a1)
        ld x24, 0x50(a1)
        ld x25, 0x58(a1)
        ld x26, 0x60(a1)
        ld x27, 0x68(a1)
        ld t0, 0x70(a1)

        jr t0
    ```

## 用户线程库(结合内核线程实现)[ ]