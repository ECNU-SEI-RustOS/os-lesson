# 用户程序-rust
使用rust语言编写用户程序。
已经部分实现基础库函数。

## 编译方法
```
    make build
```

## 模块
- [ ] 基础系统调用
- - [x] SYSCALL_FORK  

- - [x] SYSCALL_EXIT
- - [x] SYSCALL_WAIT
- - [x] SYSCALL_PIPE  
    功能：为当前进程打开一个管道。  
    参数：pipe 表示应用地址空间中的一个长度为 2 的 usize 数组的起始地址，内核需要按顺序将管道读端和写端的文件描述符写入到数组中。  
    返回值：如果出现了错误则返回 -1，否则返回 0 。可能的错误原因是：传入的地址不合法。  

- - [x] SYSCALL_READ
- - [x] SYSCALL_KILL  
    功能：当前进程向另一个进程（可以是自身）发送一个信号。  
    参数：pid 表示接受信号的进程的进程 ID, signum 表示要发送的信号的编号。  
    返回值：如果传入参数不正确（比如指定进程或信号类型不存在）则返回 -1 ,否则返回 0 。  

- - [x] SYSCALL_EXEC  
    功能：将当前进程的地址空间清空并加载一个特定的可执行文件，返回用户态后开始它的执行。  
    参数：path 给出了要加载的可执行文件的名字；  
    返回值：如果出错的话（如找不到名字相符的可执行文件）则返回 -1，否则不应该返回。

- [ ] 用户空间线程库  
    用户空间线程库（User-Level Thread Library）是一种在用户态实现线程管理的机制，与操作系统内核管理的线程（内核线程）不同。用户线程的创建、调度、同步等操作完全由库在用户空间处理，无需频繁陷入内核，从而减少上下文切换的开销
- - [x] 切换asm
    ```
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
- - [ ]

## syscall(riscv处理器)
```
fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }
    ret
}
```



