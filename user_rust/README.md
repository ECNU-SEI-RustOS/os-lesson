### rust用户程序

1.  用户程序入口  
    通过link.ld进行用户代码链接。  
    通过`ENTRY(_start)`来指定用户程序入口函数 `extern "C" fn _start(argc: usize, argv: usize) -> !`。
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
    1. `OUTPUT_ARCH(riscv)`指定目标架构为RISC-V。
    2. `ENTRY(_start)`指定程序的入口点为_start符号（用户程序启动代码）。
    3. `BASE_ADDRESS = 0x10000`表示程序加载的基地址为0x10000。
    4.  `.text` 段包含所有函数代码。  
        `.rodata` 段存储字符串常量（如日志信息）。  
        `.data` 和 `.bss` 分别存储初始化和未初始化的全局变量。  
2.  用户栈中的堆内存分配  
    基于`buddy_system_allocator`库实现的用户栈中内存分配器初始化配置
    ```rust
    use buddy_system_allocator::LockedHeap;

    const USER_HEAP_SIZE: usize = 0x10000;

    static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

    #[global_allocator]
    static HEAP: LockedHeap = LockedHeap::empty();
    ```
    ​​`#[global_allocator]`​​
    将此分配器注册为Rust的全局分配器，所有标准库类型（如Vec、Box）的内存分配会通过它进行。

    ***后续可以结合`sbrk`系统调用，升级为内核中的堆内存分配***
 

3.  `_start`  
    1.初始化用户栈中的堆内存  
    2.从内核初始化的进程（主线程）用户栈中获取参数`argc: usize, argv: usize`（内核参数设置符合函数调用规范）
    *seios操作系统内核已经将参数压入用户栈中，以下`start_`中将符合C语言标准的参数转换为符合Rust语言标准的参数（处理字符串）*
    ```rust {numberLines}
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
    ```

    **使用`#[linkage = "weak"]`进行默认`main`函数的设置，确保用户编写main函数**
    ```rust

    #[linkage = "weak"]
    #[no_mangle]
    fn main(_argc:usize, _argv:&[&str]) -> i32 {
        panic!("Cannot find main!");
    }

    ```
    3.执行用户编写的主函数`fn main(argc:usize, argv:&[&str]) -> i32`
    
    4.退出，执行`exit`系统调用，保存返回值并退出进程（主线程）。


### 基础系统调用
1. syscall(riscv处理器)
    在RISCV处理器上的系统调用，此操作系统最多支持**6个**参数。  
    通过rust语言内嵌汇编实现，使用`ecall`来触发内核陷入。

    ```rust

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
    系统调用的接口进行封装。
- [x] **SYSCALL_FORK(syscall_id = 1)**   
    功能：创建当前进程（主线程）的一个子进程（主线程），复制父进程（主线程）的内存、TrapFrame、打开文件、当前工作目录等信息，并将子进程（主线程）状态设置为可运行。  
    参数：无。    
    返回值：​   
        *child_pid(>0)​*：​​父进程（主线程）中返回子进程（主线程）的 PID​​（Process ID），用于后续管理子进程（主线程）  
        *0*​：子进程（主线程）中返回 0​​，用于确认当前是子进程（主线程）    
        *-1*：错误则返回 -1。可能的错误原因是：复制过程中出现错误  

- [x] **SYSCALL_EXIT(syscall_id = 2)**  
    ​功能​​：终止当前进程（主线程），释放其资源，并向父进程（主线程）传递退出状态。  
    ​​参数​​：  
        *status*：退出状态码（通常 0 表示成功，非 0 表示错误）。  
    ​​返回值​​：无（进程（主线程）终止，不会返回）。  

- [x] **SYSCALL_WAIT(syscall_id = 3)**  
    ​​功能​​：等待任意子进程（主线程）终止，并获取其退出状态。  
    ​​参数​​：  
        *addr*：指向存储退出状态的指针（可为 NULL）。  
    ​​返回值​​：  
        *child_pid(>0)*：成功返回终止的子进程（主线程） PID。  
        *-1*：错误（如无子进程（主线程）或信号中断）。  
- [x] **SYSCALL_PIPE(syscall_id = 4)**  
    ​​功能​​：创建一个管道，用于进程（主线程）间通信（IPC）。  
    ​​参数​​：pipefd[2]：数组，用于返回管道的读写端文件描述符（pipefd[0] 读，pipefd[1] 写）。   
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] **SYSCALL_READ(syscall_id = 5)** 
    ​​功能​​：从文件描述符读取数据。  
    ​​参数​​：  
        *fd*：文件描述符。  
        *buf*：存储数据的缓冲区指针。  
        *count*：要读取的字节数。  
    ​​返回值​​：  
        *count(>=0)*：实际读取的字节数。    
        *-1*：错误。  

- [x] **SYSCALL_KILL(syscall_id = 6)** (未实现sig：信号编号)  
    功能​​：向指定进程（主线程）发送信号。  
    ​​参数​​：  
        *pid*：目标进程（主线程）的 PID。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] **SYSCALL_EXEC(syscall_id = 7)** 
    功能​​：加载并执行新程序，替换当前进程（主线程）的地址空间。  
    ​​参数​​：
        *path*：可执行文件路径。  
        *argv*：命令行参数数组（以 NULL 结尾）。  
    ​​返回值​​：  
        *N/A*：成功不返回（进程（主线程）被替换）  
        *-1*：失败无法启动进程（主线程）。  


- [x] **SYSCALL_FSTAT(syscall_id = 8)**   
    功能​​：获取文件描述符对应的文件状态信息。  
    ​​参数​​：  
        *fd*：文件描述符。  
        *stat*：指向 `struct stat` 的指针。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] **SYSCALL_CHDIR(syscall_id = 9)**
    ​​功能​​：更改当前进程（主线程）的工作目录。  
    ​​参数​​：  
        *path*：目标目录路径。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。 
- [x] **SYSCALL_DUP(syscall_id = 10)**  
    ​​功能​​：复制文件描述符。  
    ​​参数​​：  
        *oldfd*：原文件描述符。  
    ​​返回值​​：  
        *fd(≥0)*：新的文件描述符。    
        *-1*：错误。    

- [x] **SYSCALL_GETPID(syscall_id = 11)**      
    ​​功能​​：获取当前进程（主线程）的 PID。   
    ​​参数​​：无。    
    ​​返回值​​：  
        *pid(≥0)*：当前进程（主线程）的 PID。  
 
- [x] **SYSCALL_SBRK(syscall_id = 12)**  
    ​​功能​​：调整进程（主线程）的堆内存大小。  
    ​​参数​​：  
        *increment*：内存增量（字节）。  
    ​​返回值​​：  
        *heap_addr*：成功返回新的堆顶地址  
        *-1*：错误（如内存不足）。  

- [x] **SYSCALL_SLEEP(syscall_id = 13)**  
    ​​​功能​​：使进程（主线程）休眠指定时间。  
    ​​参数​​：  
        *ticks*：休眠的时钟周期数。  
    ​​返回值​​：  
        无  
- [x] **SYSCALL_UPTIME(usize = 14)**  
    功能​​：获取系统启动后的时钟周期数。  
    ​​参数​​：无。  
    ​​返回值​​：  
        *ticks*：系统启动后运行时间时钟周期数。  
- [x] **SYSCALL_OPEN(usize = 15)**  
    功能​​：打开或创建文件。  
    ​​参数​​：  
        *path*：文件路径。  
        *flags*：打开模式（如 O_RDONLY）。    
    ​​返回值​​：  
        *fd(≥0)*：文件描述符。  
        *-1*：错误。   
- [x] **SYSCALL_WRITE(usize = 16)**  
    ​​功能​​：向文件描述符写入数据。   
    ​​参数​​：  
        *fd*：文件描述符。  
        *buf*：数据缓冲区指针。    
        *count*：要写入的字节数。    
    ​​返回值​​：  
        *count(≥0)*：实际写入的字节数。  
        *-1*：错误。  

- [x] **SYSCALL_MKNOD(usize = 17)**  
    ​​功能​​：创建设备文件或特殊文件。
    ​​参数​​：
        *path*：设备文件的路径。   
        *major*：主设备号（标识设备类型）。    
        *minor*：次设备号（标识具体设备实例）。     
    ​​返回值​​：
        *0*：成功。  
        *-1*：错误。  

- [x] **SYSCALL_UNLINK(usize = 18)**  
    功能​​：删除文件链接。  
    ​​参数​​：  
        *path*：文件路径。  
    ​​返回值​​：  
        *0*：成功。    
        *-1*：错误。  
- [x] **SYSCALL_LINK:(usize = 19)**    
    ​​功能​​：创建硬链接。  
    ​​参数​​：  
        *oldpath*：原文件路径。  
        *newpath*：新链接路径。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] **SYSCALL_MKDIR(usize = 20)**    
    功能​​：创建目录。  
    ​​参数​​：  
        *path*：目录路径。
    ​​返回值​​：
        *0*：成功。  
        *-1*：错误。  
- [x] **SYSCALL_CLOSE:(usize = 21)**    
    功能​​：关闭文件描述符。  
    ​​参数​​：  
        *fd*：文件描述符。  
    ​​返回值​​：  
        *0*：成功。  
        *-1*：错误。  

- [x] **SYSCALL_GETMTIME(usize = 22)**
    ​​功能​​：获取riscv处理器的计时器寄存器。
    ​​参数​​：  
        无。 
    ​​返回值​​：
        *(mtime)≥0*：时间戳。
        *-1*：错误。
### 错误处理
为用户程序实现的 `panic` 处理函数，使用rust提供的属性注释`#[panic_handler]` 来定义当程序发生不可恢复错误（panic）时的处理行为。该标记该函数为全局panic处理器，替代标准库的默认实现。当程序触发`panic!`时，此函数会被调用。  

该错误处理函数先输出错误发生的信息,再调用`kill(getpid())`终止当前进程（主线程）（`getpid()`获取当前进程（主线程）ID）。`unreachable!()`宏表示代码不会执行到这里，用于告诉编译器该分支不可达，帮助编译器进行优化
`PanicInfo`：包含panic的位置（文件、行号、列号）和错误消息。  
``  
**对于用户程序的错误，简单输出**

```rust
    #[panic_handler]
    fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
        // 打印 panic 位置（文件 + 行号）
        if let Some(location) = panic_info.location() {
            println!(
                "Panicked at {}:{}:{}", 
                location.file(), 
                location.line(), 
                location.column()
            );
        }
        // 打印 panic 消息（如果有）
        println!("Error: {}", panic_info.message());
        kill(getpid());
        unreachable!()
    }
```
### 用户线程库(在用户空间实现)
用户空间线程库（User-Level Thread Library）是一种在用户态实现线程管理的机制，与操作系统内核管理的线程（内核线程）不同。用户线程的创建、调度、同步等操作完全由库在用户空间处理，无需频繁陷入内核，从而减少上下文切换的开销。  
***并且这可以算是一个简易的内核，可用于教学，便于学生理解内核切换任务的过程***。

这里在用户创建的线程中的使用`yield_task`主动让出处理器资源和使用`guard`退出任务。  
**以下用户线程示例**
```rust
let r_ptr = runtime.init();

    let args1 = MyType::new(12, "ych");
    let args2 = MyType::new(17, "kss");
    runtime.spawn(|r_ptr, args | {
        println!("TASK 1 STARTING");
        let id = 1;
        let arg =  args as *const MyType;
        
        let para = unsafe {*arg};
        wait_task(r_ptr);
        for i in 0..4 {
            println!("task: {} counter: {} arg:{}", id, i, para.str);
            yield_task(r_ptr);
        }
        println!("TASK 1 FINISHED");
        guard(r_ptr);
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
        for i in 0..8 {
            println!("task: {} counter: {} arg:{}", id, i, para.str);
        }
        println!("TASK 2 FINISHED");
        signal_task(r_ptr, 1);
        guard(r_ptr);
    },&args2 as *const MyType as u64);
    runtime.run();
```
**运行结果**
```
TASK 0 (Runtime) STARTING
TASK 1 STARTING
TASK 2 STARTING
task: 2 counter: 0 arg:kss
task: 2 counter: 1 arg:kss
task: 2 counter: 2 arg:kss
task: 2 counter: 3 arg:kss
task: 2 counter: 4 arg:kss
task: 2 counter: 5 arg:kss
task: 2 counter: 6 arg:kss
task: 2 counter: 7 arg:kss
task: 2 counter: 0 arg:kss
task: 2 counter: 1 arg:kss
task: 2 counter: 2 arg:kss
task: 2 counter: 3 arg:kss
task: 2 counter: 4 arg:kss
task: 2 counter: 5 arg:kss
task: 2 counter: 6 arg:kss
task: 2 counter: 7 arg:kss
TASK 2 FINISHED
task: 1 counter: 0 arg:ych
task: 1 counter: 1 arg:ych
task: 1 counter: 2 arg:ych
task: 1 counter: 3 arg:ych
TASK 1 FINISHED
stackful_coroutine PASSED
```
1. 用户空间线程上下文结构
    ```rust
    pub struct TaskContext {
        pub x1: u64,  //ra: return address
        pub x2: u64,  //sp(s0)
        pub x8: u64,  //fp
        pub x9: u64,  //s1
         //x18-27  通用寄存器
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
    用户空间线程目前已实现`Available`该任务槽位未被占用，空闲状态； 
        `Sleep`任务处于挂起状态，等待信号；
        `Ready`任务处于可运行状态，等待调度器调度执行；
        `Running`当前任务正在 CPU 上运行。
    ```rust
    pub enum TaskState {
        Available,
        Sleep,
        Ready,
        Running,
    }
    ```

3. 用户空间线程结构
    用户态线程库的线程控制块 (Task Control Block) 结构体设计
    ```rust
    pub struct Task {
        pub id: usize,          // 线程id
        pub stack: Vec<u8>,     // 线程栈
        pub ctx: TaskContext,   // 线程上下文
        pub state: TaskState,   // 线程状态
        pub r_ptr: u64          // 线程管理器指针
    }
    ```
    `id`: 线程唯一标识符
    `stack`: 动态分配的线程栈
    `ctx`: 保存寄存器上下文的结构体
    `state`: 线程状态
    `r_ptr`: 线程管理器指针
4. 用户空间线程管理者
    用于用户线程的管理，切换线程
    ```rust
    pub struct Runtime {
        tasks: Vec<Task>,
        current: usize,
        waits: Vec<usize>
    }
    ```
    `​​tasks​`​：管理所有任务，Task 包含栈、上下文和状态。  
​    `​current`​​：标记当前运行的任务（索引）。
    `waits`： 各任务需要等待其他任务的id（`wait[i]`的值为`0`表示没有需要等待的任务；`wait[i]`的值为`x`{x = 1,2,...,MAX_TASKS},表示需要等待任务号为x发出signal信号）；`wait[i]`的值为`usize::MAX`表示需要等待任意任务发出signal信号）。
5. 用户空间线程切换核心
    用户态线程切换汇编，符合riscv架构指令二进制代码遵循严格的函数调用规范（Application Binary Interface, ABI），核心包括​​寄存器使用约定、参数传递规则、栈帧管理​​三部分。  
    通过switch的汇编保存需要被调用者保存(x8-x9, x18-x27)，x1(ra)存储​​返回地址​寄存器和x2(sp)​栈指针寄存器。
    ```S
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
    了加载Runtime的地址，确保后续`guard`和`yield_task`进行任务管理。
    ```S
        ld x10, 0x78(a1) #set r_ptr for the `guard` and the `yield_task`
    ```
    加载任务的参数给自定义函数使用。
    ```S
        ld x11, 0x80(a1)  #set args parameter
    ```

    用户线程切换核心，使用`t_yield`进行任务调度
    ```rust
    #[inline(never)]
    fn t_yield(&mut self) -> bool {
        let mut pos = self.current;
        let mut temp = 0usize;
        while self.tasks[pos].state != TaskState::Ready {
            pos += 1;
            if pos == self.tasks.len() {
                pos = 1;
                if temp == 1 {
                    pos = 0;
                }
                temp = 1;
            }
            if pos == 0 && pos == self.current {
                return false;
            }

        }

        if self.tasks[self.current].state != TaskState::Available {
            self.tasks[self.current].state = TaskState::Ready;
        }

        self.tasks[pos].state = TaskState::Running;
        let old_pos = self.current;
        self.current = pos;
        if old_pos == pos {
            return  self.tasks.len() > 0
        }

        unsafe {
            switch(&mut self.tasks[old_pos].ctx, &self.tasks[pos].ctx);
        }

        self.tasks.len() > 0
    }
    ```
    以上是任务切换过程
    ```rust
        let mut pos = self.current;
        let mut temp = 0usize;
        while self.tasks[pos].state != TaskState::Ready {
            pos += 1;
            if pos == self.tasks.len() {
                pos = 1;
                if temp == 1 {
                    pos = 0;
                }
                temp = 1;
            }
            if pos == 0 && pos == self.current {
                return false;
            }
        }
    ```
    依次在任务列表中寻找状态为`Ready`的任务。在对于Runtime的初始任务(id=0)的IDLE任务，切换时跳过，避免不必要的上下文切换开销。
    ```rust
        if old_pos == pos {
            return  self.tasks.len() > 0
        }
    ```
    在发现新任务和旧任务的相同时，不切换，避免不必要的上下文切换开销。
6. 用户线程主动yield切换  
    用于​给​主动让出当前任务的执行权​​，切换到其他就绪任务。
    ```rust
    // src/thread/mod.rs
    pub fn yield_task(r_ptr: u64) {
        unsafe {
            let rt_ptr = r_ptr as *mut Runtime;
            (*rt_ptr).t_yield();
        };
    }
    ```


7. 线程结束
    用于线程退出处理​，用户自定义函数运行完毕后，返回任务管理器，并且将任务状态设置为`Available`。
    ```rust
    // src/thread/mod.rs
    pub fn guard(r_ptr: *const Runtime) {
        unsafe {
            let rt_ptr = r_ptr as *mut Runtime;
            (*rt_ptr).t_return();
        };
    }
    ```
        ```rust
    impl Runtime{
        ...
        fn t_return(&mut self) {
            if self.current != 0 {
                self.tasks[self.current].state = TaskState::Available;
                self.t_yield();
            }
        }
        ...
    }
    ```

8. 线程创建
    找到空闲任务，设置其栈和上下文。  
    设置任务入口函数f和栈指针（需对齐）和任务参数。  
    `fn spawn(&mut self, f: fn(*const Runtime, u64), params: u64) -> usize`需要传入一个自定义的`fn(*const Runtime, u64)`的函数指针和一个自定义参数的地址`u64`，返回值是分配的任务的`tid`。
    ```rust
    // src/thread/mod.rs
    pub fn spawn(&mut self, f: fn(*const Runtime, u64), params: u64) -> usize {
        let available = self
            .tasks
            .iter_mut()
            .find(|t| t.state == TaskState::Available)
            .expect("no available task.");

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

        available.id
    }
    ```
9. 等待其他任务
由于任务有同步性的需求，实现等待其他任务完成的功能。  
`waittid_task`：等待指定的任务完成，需要指定任务号（id）;  
`wait_task`：等待任务其他任务完成。
```rust
pub fn waittid_task(r_ptr: *const Runtime, id: usize) {
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_wait(id);
    };
}
pub fn wait_task(r_ptr: *const Runtime){
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_wait(0);
    };
}
```
`t_wait`是具体实现，将任务的状态设置为`Sleep`挂起状态，将，并让出处理器资源，切换到其他任务。根据`id`参数确实当前任务是等待指定任务还是任意任务。
```rust
    fn t_wait(&mut self, id: usize) {
        if self.current != 0 {
            self.tasks[self.current].state = TaskState::Sleep;
            if id == 0 {
                self.waits[self.current] = usize::MAX;
            } else {
                self.waits[self.current] = id;
            }
            self.t_yield();
        }
    }
```
10. 唤醒指定任务
通过`signal_task`来唤醒指定任务。
```rust
pub fn signal_task(r_ptr: *const Runtime, id: usize) {
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_signal(id);
    };
}
```
```rust
    fn t_signal(&mut self, id: usize){
        if self.waits[id] == usize::MAX {
            self.tasks[id].state = TaskState::Ready;
            self.waits[id] = 0;
        }else if self.waits[id] == self.current {
            self.tasks[id].state = TaskState::Ready;
            self.waits[id] = 0;
        }
        self.t_yield();
    }
```

11. 获取当前任务id
```rust
pub fn gettid_task(r_ptr: *const Runtime)-> usize{
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_gettid()
    }
}
```