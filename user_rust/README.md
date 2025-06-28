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
- [x] SYSCALL_FORK(syscall_id = 1)  
    功能：创建当前进程的一个子进程，复制父进程的内存、TrapFrame、打开文件、当前工作目录等信息，并将子进程状态设置为可运行。  
    参数：无。    
    返回值：​   
        *child_pid(>0)​*：​​父进程中返回子进程的 PID​​（Process ID），用于后续管理子进程  
        *0*​：子进程中返回 0​​，用于确认当前是子进程    
        *-1*：错误则返回 -1。可能的错误原因是：复制过程中出现错误  

- [x] SYSCALL_EXIT(syscall_id = 2)  
    ​功能​​：终止当前进程，释放其资源，并向父进程传递退出状态。  
    ​​参数​​：  
        *status*：退出状态码（通常 0 表示成功，非 0 表示错误）。  
    ​​返回值​​：无（进程终止，不会返回）。  

- [x] SYSCALL_WAIT(syscall_id = 3)  
    ​​功能​​：等待任意子进程终止，并获取其退出状态。  
    ​​参数​​：  
        *addr*：指向存储退出状态的指针（可为 NULL）。  
    ​​返回值​​：  
        *child_pid(>0)*：成功返回终止的子进程 PID。  
        *-1*：错误（如无子进程或信号中断）。  
- [x] SYSCALL_PIPE(syscall_id = 4)  
    ​​功能​​：创建一个管道，用于进程间通信（IPC）。  
    ​​参数​​：pipefd[2]：数组，用于返回管道的读写端文件描述符（pipefd[0] 读，pipefd[1] 写）。   
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] SYSCALL_READ(syscall_id = 5) 
    ​​功能​​：从文件描述符读取数据。  
    ​​参数​​：  
        *fd*：文件描述符。  
        *buf*：存储数据的缓冲区指针。  
        *count*：要读取的字节数。  
    ​​返回值​​：  
        *count(>=0)*：实际读取的字节数。    
        *-1*：错误。  

- [x] SYSCALL_KILL(syscall_id = 6) (未实现sig：信号编号)  
    功能​​：向指定进程发送信号。  
    ​​参数​​：  
        *pid*：目标进程的 PID。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] SYSCALL_EXEC(syscall_id = 7)  
    功能​​：加载并执行新程序，替换当前进程的地址空间。  
    ​​参数​​：
        *path*：可执行文件路径。  
        *argv*：命令行参数数组（以 NULL 结尾）。  
    ​​返回值​​：  
        *N/A*：成功不返回（进程被替换）  
        *-1*：失败无法启动进程。  


- [x] SYSCALL_FSTAT(syscall_id = 8)   
    功能​​：获取文件描述符对应的文件状态信息。  
    ​​参数​​：  
        *fd*：文件描述符。  
        *stat*：指向 `struct stat` 的指针。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] SYSCALL_CHDIR(syscall_id = 9)
    ​​功能​​：更改当前进程的工作目录。  
    ​​参数​​：  
        *path*：目标目录路径。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。 
- [x] SYSCALL_DUP(syscall_id = 10)  
    ​​功能​​：复制文件描述符。  
    ​​参数​​：  
        *oldfd*：原文件描述符。  
    ​​返回值​​：  
        *fd(≥0)*：新的文件描述符。    
        *-1*：错误。    

- [x] SYSCALL_GETPID(syscall_id = 11)      
    ​​功能​​：获取当前进程的 PID。   
    ​​参数​​：无。    
    ​​返回值​​：  
        *pid(≥0)*：当前进程的 PID。  
 
- [x] SYSCALL_SBRK(syscall_id = 12)  
    ​​功能​​：调整进程的堆内存大小。  
    ​​参数​​：  
        *increment*：内存增量（字节）。  
    ​​返回值​​：  
        *heap_addr*：成功返回新的堆顶地址  
        *-1*：错误（如内存不足）。  

- [x] SYSCALL_SLEEP(syscall_id = 13)  
    ​​​功能​​：使进程休眠指定时间。  
    ​​参数​​：  
        *ticks*：休眠的时钟周期数。  
    ​​返回值​​：  
        无  
- [x] SYSCALL_UPTIME(usize = 14)  
    功能​​：获取系统启动后的时钟周期数。  
    ​​参数​​：无。  
    ​​返回值​​：  
        *ticks*：系统启动后运行时间时钟周期数。  
- [x] SYSCALL_OPEN(usize = 15)  
    功能​​：打开或创建文件。  
    ​​参数​​：  
        *path*：文件路径。  
        *flags*：打开模式（如 O_RDONLY）。    
    ​​返回值​​：  
        *fd(≥0)*：文件描述符。  
        *-1*：错误。   
- [x] SYSCALL_WRITE(usize = 16)  
    ​​功能​​：向文件描述符写入数据。   
    ​​参数​​：  
        *fd*：文件描述符。  
        *buf*：数据缓冲区指针。    
        *count*：要写入的字节数。    
    ​​返回值​​：  
        *count(≥0)*：实际写入的字节数。  
        *-1*：错误。  

- [x] SYSCALL_MKNOD(usize = 17)  
    ​​功能​​：创建设备文件或特殊文件。
    ​​参数​​：
        *path*：设备文件的路径。   
        *major*：主设备号（标识设备类型）。    
        *minor*：次设备号（标识具体设备实例）。     
    ​​返回值​​：
        *0*：成功。  
        *-1*：错误。  

- [x] SYSCALL_UNLINK(usize = 18)  
    功能​​：删除文件链接。  
    ​​参数​​：  
        *path*：文件路径。  
    ​​返回值​​：  
        *0*：成功。    
        *-1*：错误。  
- [x] SYSCALL_LINK:(usize = 19)    
    ​​功能​​：创建硬链接。  
    ​​参数​​：  
        *oldpath*：原文件路径。  
        *newpath*：新链接路径。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] SYSCALL_MKDIR(usize = 20)    
    功能​​：创建目录。  
    ​​参数​​：  
        *path*：目录路径。
    ​​返回值​​：
        *0*：成功。  
        *-1*：错误。  
- [x] SYSCALL_CLOSE:(usize = 21)    
    功能​​：关闭文件描述符。  
    ​​参数​​：  
        *fd*：文件描述符。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] SYSCALL_GETMTIME(usize = 22)
    ​​功能​​：获取riscv处理器的计时器寄存器。
    ​​参数​​：  
        无。 
    ​​返回值​​：
        *(mtime)≥0*：时间戳。
        *-1*：错误。

## 用户线程库(全部在用户空间实现)
用户空间线程库（User-Level Thread Library）是一种在用户态实现线程管理的机制，与操作系统内核管理的线程（内核线程）不同。用户线程的创建、调度、同步等操作完全由库在用户空间处理，无需频繁陷入内核，从而减少上下文切换的开销

1. 用户空间线程上下文结构
    ```
    pub struct TaskContext {
        pub x1: u64,  //ra: return address
        pub x2: u64,  //sp(s0)
        pub x8: u64,  //fp
        pub x9: u64,  //s1
         //x18-27: s2-11  通用寄存器
        pub x18: u64,
        pub x19: u64,
        pub x20: u64,
        pub x21: u64,
        pub x22: u64,
        pub x23: u64,
        pub x24: u64,
        pub x25: u64,
        pub x26: u64,
        pub x27: u64,
        pub nx1: u64,   // new return address
        pub r_ptr: u64  // self ptr 指向RUNTIME的指针
        pub params: u64 // 传入参数的 args 的地址
    }
    ```
2. 用户空间线程状态
    ```
    pub enum TaskState {
        Available,
        Ready,
        Running,
    }
    ```

3. 用户空间线程结构
    ```
    pub struct Task {
        pub id: usize,          // 线程id
        pub stack: Vec<u8>,     // 线程栈
        pub ctx: TaskContext,   // 线程上下文
        pub state: TaskState,   // 线程状态
        pub r_ptr: u64          // 线程管理器指针
    }
    ```
4. 用户空间线程管理者
    用于用户线程的管理，切换线程
    ```
    pub struct Runtime {
        tasks: Vec<Task>,
        current: usize,
    }
    ```
    `​​tasks​`​：管理所有任务，Task 包含栈、上下文和状态。  
​    `​current`​​：标记当前运行的任务（索引）。  
5. 用户空间线程切换核心
    用户态线程切换汇编，符合riscv架构指令二进制代码遵循严格的函数调用规范（Application Binary Interface, ABI），核心包括​​寄存器使用约定、参数传递规则、栈帧管理​​三部分。  
    通过switch的汇编保存需要被调用者保存(x8-x9, x18-x27)，x1(ra)存储​​返回地址​寄存器和x2(sp)​栈指针寄存器。
    ```
    // src/thread/switch.S
    .global switch
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
        sd x11, 0x80(a0)

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
        ld x10, 0x78(a1) #set r_ptr
        ld x11, 0x80(a1) #set param
        jr t0
    ```
    
    ```
        ld x10, 0x78(a1) #set r_ptr for the `guard` and the `yield_task`
    ```
    以上是为了加载Runtime的地址，确保后续`guard`和`yield_task`进行任务管理。
    ```
        ld x11, 0x80(a1)  #set args parameter
    ```
    以上是为了加载任务的参数给自定义函数使用。

6. 用户线程主动yield切换  
    用于​​主动让出当前任务的执行权​​，切换到其他就绪任务。
    ```
    // src/thread/mod.rs
    pub fn yield_task(r_ptr: u64) {
        unsafe {
            let rt_ptr = r_ptr as *mut Runtime;
            (*rt_ptr).t_yield();
        };
    }
    ```
    这里在用户创建的线程中的使用`yield_task`主动让出处理器资源和使用`guard`退出任务。
    例如以下创建的用户线程
    ```
    runtime.spawn(|r_ptr, args| {
        println!("TASK 2 STARTING");
        let id = 2;
        let arg =  args as *const MyType;
        
        let para = unsafe {*arg};
        for i in 0..8 {
            println!("task: {} counter: {} arg:{}", id, i, para.str);
            yield_task(r_ptr);
        }
        for i in 0..8 {
            println!("task: {} counter: {} arg:{}", id, i, para.str);
        }
        println!("TASK 2 FINISHED");
        guard(r_ptr);
    },&args2 as *const MyType as u64);
    ```
7. 线程结束
    用于线程退出处理​，用户自定义函数运行完毕后，返回线程管理器
    ```
    // src/thread/mod.rs
    fn guard() {
        let value: u64;
        unsafe {
            asm!(
                "mv {}, t1",
                out(reg) value, 
            );                  //获取t1寄存器的RunTime地址
            
            let rt_ptr = value as *mut Runtime;
            (*rt_ptr).t_return();
        };
    }
    ```

8. 线程创建
    找到空闲任务，设置其栈和上下文。  
    设置任务入口函数f和栈指针（需对齐）和任务参数。  
    `fn spawn(&mut self, f: fn(*const Runtime, u64), params: u64) `需要传入一个自定义的`fn(*const Runtime, u64)`的函数指针和一个自定义参数的地址`u64`。
    ```
    // src/thread/mod.rs
    pub fn spawn(&mut self, f: fn(*const Runtime, u64), params: u64) {
        let available = self
            .tasks
            .iter_mut()
            .find(|t| t.state == TaskState::Available)
            .expect("no available task.");

        //println!("RUNTIME: spawning task {} and r_ptr {:x}", available.id, available.r_ptr);
        let size = available.stack.len();
        unsafe {
            let s_ptr = available.stack.as_mut_ptr().offset(size as isize);

            let s_ptr = (s_ptr as usize & !7) as *mut u8;

            available.ctx.x1 = guard as u64; //ctx.x1  is old return address
            available.ctx.nx1 = f as u64; //ctx.nx1 is new return address
            available.ctx.x2 = s_ptr.offset(-32) as u64; //cxt.x2 is sp
            available.ctx.r_ptr = available.r_ptr;
            available.ctx.params = params;        // pointer to user's custom parameter
        }
        available.state = TaskState::Ready;
    }
    ```
## 用户线程库(结合内核线程实现)[ ]