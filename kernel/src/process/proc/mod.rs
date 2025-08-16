use alloc::rc::Rc;
use alloc::vec::{self, Vec};
use array_macro::array;

use crate::consts::{
    fs::{NFILE, ROOTIPATH},
    PAGE_SIZE,
};
use crate::consts::{KERNEL_STACK_SIZE, MAX_TASKS_PER_PROC, USER_STACK_SIZE};
use crate::fs::{File, Inode, ICACHE, LOG};
use crate::mm::pagetable::ustack_bottom_by_pos;
use crate::mm::{PageTable, PhysAddr, PteFlag, RawPage, RawSinglePage, VirtAddr};
use crate::process::sync::sem::Semaphore;
use crate::process::task::trapframe_from_tid;
use crate::process::TaskFifo;
use crate::register::{satp, sepc, sstatus};
use crate::spinlock::{SpinLock, SpinLockGuard};
use crate::trap::user_trap;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::mem;
use core::option::Option;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use super::cpu::CPU_MANAGER;
use super::CpuManager;
use super::PROC_MANAGER;
use super::{fork_ret, Context, TrapFrame};

use self::syscall::Syscall;

mod elf;
pub mod manager;
pub mod pid;
mod syscall;

/// 进程（主线程）状态枚举类型，表示操作系统内核中进程（主线程）的不同生命周期状态。
///
/// 该枚举用于进程（主线程）调度与管理，反映进程（主线程）当前的执行或等待状态，
/// 便于操作系统根据状态做出调度决策与资源回收处理。
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum ProcState {
    /// 该进程（主线程）槽位未被占用，空闲状态。
    UNUSED,
    /// 进程（主线程）处于睡眠状态，等待某事件或资源唤醒。
    SLEEPING,
    /// 进程（主线程）处于可运行状态，等待调度器调度执行。
    RUNNABLE,
    /// 进程（主线程）当前正在 CPU 上运行。
    RUNNING,
    /// 进程（主线程）已被分配但尚未准备好运行。
    ALLOCATED,
    /// 进程（主线程）已退出，处于僵尸状态，等待父进程（主线程）回收。
    ZOMBIE,
}

/// 进程（主线程）的排他信息结构体，包含进程（主线程）的核心状态和控制字段。
///
/// 该结构体保存进程（主线程）的调度状态、退出码、等待通道及进程（主线程）标识符等信息，
/// 通常由进程（主线程）的排它锁保护，确保并发环境下的安全访问与修改。
pub struct TaskExcl {
    /// 进程（主线程）当前的状态，类型为 [`ProcState`]，反映进程（主线程）生命周期阶段。
    pub state: ProcState,
    /// 进程（主线程）退出时的状态码，用于父进程（主线程）获取子进程（主线程）退出信息。
    pub exit_status: i32,
    /// 进程（主线程）等待的通道标识，用于睡眠和唤醒机制的同步。
    pub channel: usize,
    /// 进程（主线程）的唯一标识符（进程（主线程）ID）。
    pub pid: usize,
    /// 线程号
    pub tid: usize,
}

impl TaskExcl {
    const fn new() -> Self {
        Self {
            state: ProcState::UNUSED,
            exit_status: 0,
            channel: 0,
            pid: 0,
            tid: 0,
        }
    }

    /// Clean up the content in [`ProcExcl`],
    pub fn cleanup(&mut self) {
        self.pid = 0;
        self.channel = 0;
        self.exit_status = 0;
        self.state = ProcState::UNUSED;
    }
}

/// 进程（主线程）私有数据结构，保存进程（主线程）运行时的核心信息。
///
/// 该结构体仅在当前进程（主线程）运行时访问，或在持有 [`ProcExcl`] 锁的其他进程（主线程）（例如 fork）
/// 初始化时访问。包含内核栈指针、内存大小、上下文、打开的文件、用户页表等私有资源。
pub struct TaskData {
    /// 进程（主线程）内核栈的起始虚拟地址。
    kstack: usize,
    ///  用户栈起始基地址。
    ustack_base: usize,
    /// 进程（主线程）使用的内存大小（字节数）。
    size: usize,
    /// 进程（主线程）上下文（寄存器状态等），用于上下文切换。
    context: Context,
    /// 进程（主线程）名称，最长16字节，通常用于调试和显示。
    name: [u8; 16],
    /// 进程（主线程）打开的文件数组，元素为可选的引用计数智能指针。
    open_files: [Option<Arc<File>>; NFILE],
    /// 指向 TrapFrame 的裸指针，保存用户态寄存器临时值等信息。
    pub trapframe: *mut TrapFrame,
    /// 进程（主线程）的用户页表，管理用户地址空间映射。
    pub pagetable: Option<*mut PageTable>,
    /// 进程（主线程）当前工作目录的 inode。
    pub cwd: Option<Inode>,
}

impl TaskData {
    const fn new() -> Self {
        Self {
            kstack: 0,
            ustack_base: 0,
            size: 0,
            context: Context::new(),
            name: [0; 16],
            open_files: array![_ => None; NFILE],
            trapframe: ptr::null_mut(),
            pagetable: None,
            cwd: None,
        }
    }
    /// 获取进程（主线程）中的线程数量
    // pub fn thread_count(&self) -> usize {
    //     self.tasks.len()
    // }
    /// Set kstack
    pub fn set_kstack(&mut self, kstack: usize) {
        self.kstack = kstack;
    }
    /// Set ustack_base
    pub fn set_ustack_base(&mut self, ustack_base: usize) {
        self.ustack_base = ustack_base;
    }
    /// Set ustack_base
    pub fn get_ustack_base(&self) -> usize {
        self.ustack_base
    }
    /// # 功能说明
    /// 初始化进程（主线程）的上下文信息。该函数在进程（主线程）创建后调用，
    /// 将进程（主线程）上下文清零，并设置返回地址为 `fork_ret`，
    /// 以便进程（主线程）切换到用户态时从 `fork_ret` 函数开始执行。
    ///
    /// # 流程解释
    /// 1. 调用 `context.clear()` 清空当前上下文寄存器状态。
    /// 2. 设置上下文的返回地址寄存器（ra）为 `fork_ret` 函数的地址，
    ///    确保进程（主线程）切换后执行 fork 返回逻辑。
    /// 3. 设置栈指针（sp）指向内核栈顶（`kstack + PGSIZE*4`），
    ///    以保证内核栈空间正确。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）的可变引用，用于修改其上下文和内核栈指针。
    ///
    /// # 返回值
    /// - 无返回值。
    pub fn init_context(&mut self) {
        self.context.clear();
        self.context.set_ra(fork_ret as *const () as usize);
        self.context.set_sp(self.kstack + PAGE_SIZE * 4);
    }

    /// Return the task's mutable reference of context
    pub fn get_context(&mut self) -> *mut Context {
        &mut self.context as *mut _
    }

    /// # 功能说明
    /// 准备进程（主线程）从内核态返回到用户态所需的 TrapFrame 和寄存器状态，
    /// 并返回用户页表的 satp 寄存器值以切换地址空间。
    ///
    /// # 流程解释
    /// 1. 获取当前进程（主线程）的 TrapFrame，可修改其中的内核态相关字段。
    /// 2. 读取当前内核页表的 satp 寄存器值，保存到 `tf.kernel_satp`，
    ///    用于内核态返回时恢复内核页表映射。
    /// 3. 设置内核栈指针 `tf.kernel_sp` 指向内核栈顶（`kstack + PGSIZE*4`）。
    /// 4. 设置内核陷阱处理入口 `tf.kernel_trap` 为用户态陷阱处理函数地址 `user_trap`。
    /// 5. 设置当前 CPU 核心编号到 `tf.kernel_hartid`。
    /// 6. 将之前保存在 TrapFrame 的用户程序计数器 `epc` 写回 sepc 寄存器，
    ///    用于从陷阱返回后继续执行用户程序。
    /// 7. 返回当前进程（主线程）的用户页表的 satp 寄存器值，供汇编代码切换页表。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）的可变引用，用于修改其 TrapFrame 和页表信息。
    ///
    /// # 返回值
    /// - 返回 `usize` 类型的用户页表的 satp 寄存器值，用于地址空间切换。
    ///
    /// # 可能的错误
    /// - 函数中使用了多处 `unwrap()` 和 `unsafe`，
    ///   若 `tf` 或 `pagetable` 未正确初始化，可能触发 panic 或未定义行为。
    /// - 需保证当前线程确实持有对进程（主线程）数据的独占访问。
    ///
    /// # 安全性
    /// - 本函数包含 `unsafe` 代码块，假设 `tf` 指针有效且进程（主线程）页表已正确初始化。
    /// - 调用者需确保在进程（主线程）调度上下文中调用此函数，避免数据竞争。
    /// - 返回的 satp 值需用于低级上下文切换汇编代码，确保切换正确执行。
    pub fn user_ret_prepare(&mut self) -> usize {
        let trapframe: &mut TrapFrame = unsafe { self.trapframe.as_mut().unwrap() };
        trapframe.kernel_satp = satp::read();
        // current kernel stack's content is cleaned
        // after returning to the kernel spaces
        trapframe.kernel_sp = &self.kstack + KERNEL_STACK_SIZE;
        trapframe.kernel_trap = user_trap as usize;
        trapframe.kernel_hartid = unsafe { CpuManager::cpu_id() };
        // restore the user pc previously stored in sepc
        sepc::write(trapframe.epc);

        unsafe { self.pagetable.unwrap().as_mut().unwrap().as_satp() }
    }
    // pub fn user_ret_prepare_task(&mut self) -> (usize, usize) {
    //     let task = &self.tasks[0].as_ref().unwrap();
    //     let trapframe: &mut TrapFrame = task.get_trap_frame();
    //     trapframe.kernel_satp = satp::read();
    //     // current kernel stack's content is cleaned
    //     // after returning to the kernel space

    //     trapframe.kernel_sp = task.get_kstack_bottom() + KERNEL_STACK_SIZE;
    //     trapframe.kernel_trap = user_trap as usize;
    //     trapframe.kernel_hartid = unsafe { CpuManager::cpu_id() };

    //     // restore the user pc previously stored in sepc
    //     sepc::write(trapframe.epc);

    //     (self.pagetable.as_ref().unwrap().as_satp(), task.tid)
    // }

    /// 简单检查用户传入的虚拟地址是否在合法范围内。
    fn check_user_addr(&self, user_addr: usize) -> Result<(), ()> {
        if user_addr > self.size {
            Err(())
        } else {
            Ok(())
        }
    }

    /// 将内容从 src 复制到用户的目标虚拟地址 dst。
    /// 总共复制 count 字节。
    /// 实际操作会转发调用到页表的对应方法。
    #[inline]
    pub fn copy_out(&mut self, src: *const u8, dst: usize, count: usize) -> Result<(), ()> {
        unsafe {
            self.pagetable
                .unwrap()
                .as_mut()
                .unwrap()
                .copy_out(src, dst, count)
        }
    }

    /// 将内容从用户的源虚拟地址 src 复制到内核空间的目标地址 dst。
    /// 总共复制 count 字节。
    /// 实际操作会转发调用到页表的对应方法。
    #[inline]
    pub fn copy_in(&self, src: usize, dst: *mut u8, count: usize) -> Result<(), ()> {
        unsafe {
            self.pagetable
                .unwrap()
                .as_mut()
                .unwrap()
                .copy_in(src, dst, count)
        }
    }

    /// 分配一个新的文件描述符。
    /// 返回的文件描述符可直接作为索引使用，因为它仅属于当前进程（主线程）私有。
    fn alloc_fd(&mut self) -> Option<usize> {
        self.open_files
            .iter()
            .enumerate()
            .find(|(_, f)| f.is_none())
            .map(|(i, _)| i)
    }

    /// 分配一对文件描述符。
    /// 通常用于管道（pipe）的创建。
    fn alloc_fd2(&mut self) -> Option<(usize, usize)> {
        let mut iter = self
            .open_files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_none())
            .take(2)
            .map(|(i, _)| i);
        let fd1 = iter.next()?;
        let fd2 = iter.next()?;
        Some((fd1, fd2))
    }

    /// # 功能说明
    /// 清理进程（主线程）私有数据中的部分资源状态，主要用于进程（主线程）退出或重用时的复位。
    /// 该函数会清空进程（主线程）名称首字节，释放与进程（主线程）相关的 trapframe 内存，
    /// 并释放进程（主线程）的用户页表对应的内存区域，同时重置进程（主线程）的内存大小。
    ///
    /// # 流程解释
    /// 1. 将进程（主线程）名称数组的第一个字节置为 0，标记名称为空。
    /// 2. 保存当前的 `tf`（TrapFrame）裸指针，并将结构体中的 `tf` 指针置空。
    /// 3. 若原 `tf` 指针非空，调用不安全代码通过 `RawSinglePage::from_raw_and_drop` 释放该内存。
    /// 4. 取出并移除当前进程（主线程）的用户页表（`pagetable`）。
    /// 5. 若页表存在，调用 `dealloc_proc_pagetable` 释放进程（主线程）占用的用户内存页面。
    /// 6. 重置进程（主线程）占用的内存大小 `sz` 为 0。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）私有数据的可变引用，用于修改和释放其资源。
    ///
    /// # 返回值
    /// - 无返回值。
    ///
    /// # 可能的错误
    /// - 如果 `tf` 指针指向的内存已经被其他代码释放，调用 `from_raw_and_drop` 可能导致未定义行为。
    /// - 若 `pagetable` 未正确初始化，调用 `dealloc_proc_pagetable` 可能引发错误或崩溃。
    ///
    /// # 安全性
    /// - 该函数包含不安全代码，依赖于 `tf` 指针的有效性和唯一所有权。
    /// - 调用者必须确保在进程（主线程）数据被其他代码访问之前调用此函数，避免资源竞争。
    /// - 释放页表时必须保证当前进程（主线程）内存映射处于可安全释放状态，避免悬挂指针。
    pub fn cleanup(&mut self, tid: usize, is_child_task: bool) {
        self.name[0] = 0;
        let tf = self.trapframe;
        self.trapframe = ptr::null_mut();
        if !tf.is_null() {
            unsafe {
                RawSinglePage::from_raw_and_drop(tf as *mut u8);
            }
        }
        if !is_child_task {
            let pgt = self.pagetable.take();
            if let Some(mut pgt) = pgt {
                let mut pgt = unsafe { pgt.as_mut().unwrap() };
                pgt.dealloc_proc_pagetable(self.size, tid);
            }
            self.size = 0;
        } else {
            self.pagetable.take();
        }
    }

    /// # 功能说明
    /// 关闭进程（主线程）打开的所有文件，并释放当前工作目录的引用。
    /// 该函数通常在进程（主线程）退出时调用，用于清理进程（主线程）的文件资源和目录引用。
    ///
    /// # 流程解释
    /// 1. 遍历进程（主线程）打开的文件句柄数组 `open_files`，逐个取出并释放文件引用。
    /// 2. 调用日志系统 `LOG` 的 `begin_op()`，开始一次文件系统操作。
    /// 3. 使用断言确保当前工作目录 `cwd` 不为空。
    /// 4. 释放当前工作目录的引用（调用 `take()` 后立即 drop）。
    /// 5. 调用 `LOG.end_op()` 结束日志操作。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）私有数据的可变引用，用于操作其文件和目录成员。
    ///
    /// # 返回值
    /// - 无返回值。
    ///
    /// # 可能的错误
    /// - 若 `cwd` 为 `None`，`debug_assert!` 会在调试模式下触发断言失败。
    /// - 释放文件句柄和目录引用过程中，若底层文件系统操作失败，可能影响资源释放完整性（依赖日志系统机制）。
    ///
    /// # 安全性
    /// - 函数依赖外部日志系统 `LOG` 正确管理文件系统操作的事务一致性。
    /// - 关闭文件和释放目录引用必须确保调用时无其他线程或代码持有相关资源，避免竞态条件。
    /// - 本函数无不安全代码调用，符合 Rust 安全规范。
    pub fn close_files(&mut self) {
        for f in self.open_files.iter_mut() {
            drop(f.take())
        }
        LOG.begin_op();
        debug_assert!(self.cwd.is_some());
        drop(self.cwd.take());
        LOG.end_op();
    }

    /// # 功能说明
    /// 调整进程（主线程）的用户堆大小，实现类似 UNIX 中的 `sbrk` 功能。
    /// 根据参数 `increment` 增加或减少用户地址空间的大小，
    /// 并相应地分配或释放物理内存页面。
    ///
    /// # 流程解释
    /// 1. 记录当前内存大小 `old_size` 以备返回。
    /// 2. 若 `increment` 大于 0，计算新的堆大小 `new_size`，
    ///    调用页表的 `uvm_alloc` 分配对应内存区域，更新进程（主线程）内存大小。
    /// 3. 若 `increment` 小于 0，计算减少后的堆大小 `new_size`，
    ///    调用页表的 `uvm_dealloc` 释放对应内存区域，更新进程（主线程）内存大小。
    /// 4. 返回调整前的内存大小 `old_size`。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）私有数据的可变引用，用于访问和修改内存大小及页表。
    /// - `increment`：调整的字节数，正数表示扩展堆空间，负数表示缩减堆空间。
    ///
    /// # 返回值
    /// - `Ok(usize)`：返回调整前的堆大小（字节数）。
    /// - `Err(())`：当内存分配失败时返回错误。
    ///
    /// # 可能的错误
    /// - 当调用 `uvm_alloc` 分配新内存失败时，返回 `Err(())`。
    /// - 负数缩减堆空间时未显式检查边界，可能出现内存越界或非法释放。
    ///
    /// # 安全性
    /// - 依赖 `pagetable` 正确初始化和有效性，`unwrap()` 可能引发 panic。
    /// - 调用者需保证调整操作在进程（主线程）内存空间允许的范围内，避免非法访问。
    /// - 函数内部无使用不安全代码，符合 Rust 内存安全原则。
    fn sbrk(&mut self, increment: i32) -> Result<usize, ()> {
        let old_size = self.size;
        if increment > 0 {
            let new_size = old_size + (increment as usize);
            unsafe {
                self.pagetable
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .uvm_alloc(old_size, new_size)?
            };
            self.size = new_size;
        } else if increment < 0 {
            let new_size = old_size - ((-increment) as usize);
            unsafe {
                self.pagetable
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .uvm_dealloc(old_size, new_size)
            };
            self.size = new_size;
        }
        Ok(old_size)
    }
}

/// Task结构体，代表操作系统内核中的一个任务实体和一个线程（区分主线程和子线程）。
///
/// 该结构体封装了进程（主线程）在进程（主线程）表中的索引，
/// 进程（主线程）状态的排它锁保护数据（`ProcExcl`），
/// 进程（主线程）私有数据（`ProcData`），
/// 以及进程（主线程）是否被杀死的原子标志。
///
/// 通过该结构体，操作系统能够管理进程（主线程）调度、状态更新和资源访问的并发安全。
pub struct Task {
    /// 在任务表中的索引，唯一标识该进程（主线程）槽位。
    index: usize,
    /// 是否为子线程
    pub is_child_task: bool,
    /// 任务排它锁保护的状态信息，包括状态、pid、等待通道等。
    pub excl: SpinLock<TaskExcl>,
    /// 任务私有数据，包含内存、上下文、文件描述符等，通过 UnsafeCell 实现内部可变性。
    pub data: UnsafeCell<TaskData>,
    /// 子线程
    pub tasks: SpinLock<Vec<Option<*mut Task>>>,
    /// 父主线程
    pub parent: Option<*mut Task>,
    /// 锁
    pub semaphore_list: SpinLock<Vec<Option<Semaphore>>>,
    /// 标识（主线程）是否被杀死的原子布尔变量，用于调度和信号处理。
    pub killed: AtomicBool,
}

impl Task {
    pub const fn new(index: usize, is_child_task: bool) -> Self {
        Self {
            index,
            is_child_task,
            excl: SpinLock::new(TaskExcl::new(), "ProcExcl"),
            data: UnsafeCell::new(TaskData::new()),
            tasks: SpinLock::new(Vec::new(), "tasks"),
            parent: None,
            semaphore_list: SpinLock::new(Vec::new(), "mutex locks"),
            killed: AtomicBool::new(false),
        }
    }

    /// # 功能说明
    /// 初始化第一个用户进程（主线程）的相关数据，包括加载初始化代码到用户页表、
    /// 设置用户程序计数器（PC）和栈指针（SP），
    /// 以及初始化进程（主线程）名称和当前工作目录。
    ///
    /// # 流程解释
    /// 1. 获取当前进程（主线程）的私有数据的可变引用 `pd`。
    /// 2. 使用 `uvm_init` 将内核预定义的初始化代码 `INITCODE` 映射到用户页表。
    /// 3. 设置进程（主线程）内存大小 `sz` 为一页大小（`PGSIZE`）。
    /// 4. 获取进程（主线程）的 TrapFrame 指针 `tf`，设置用户态程序计数器 `epc` 为 0，
    ///    栈指针 `sp` 为一页大小，准备用户态执行环境。
    /// 5. 将进程（主线程）名称设置为 `"initcode"`，通过不安全的内存复制完成。
    /// 6. 断言当前工作目录 `cwd` 为空，确保进程（主线程）尚未设置目录。
    /// 7. 通过根目录路径 `ROOTIPATH` 从 inode 缓存中获取根目录 inode，
    ///    并设置为当前工作目录。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）的可变引用，用于访问和修改其私有数据。
    ///
    /// # 返回值
    /// - 无返回值。
    ///
    /// # 可能的错误
    /// - 如果根目录 inode 无法找到，`expect` 会导致内核 panic，
    ///   表示文件系统初始化异常。
    /// - `unwrap` 调用若指针无效会触发 panic。
    ///
    /// # 安全性
    /// - 使用了不安全代码 `unsafe` 来操作裸指针和内存复制，
    ///   调用者需确保 `tf` 和 `name` 字段有效且可写。
    /// - 假定当前调用环境下独占访问 `ProcData`，避免数据竞争。
    pub fn user_init(&mut self) {
        let tdata = self.data.get_mut();

        // map initcode in user pagetable
        unsafe {
            tdata
                .pagetable
                .unwrap()
                .as_mut()
                .unwrap()
                .uvm_init(&INITCODE);
        }
        tdata.ustack_base = PAGE_SIZE;
        tdata.size = PAGE_SIZE;

        // prepare return pc and stack pointer
        let trapframe = unsafe { tdata.trapframe.as_mut().unwrap() };
        trapframe.epc = 0;
        trapframe.sp = PAGE_SIZE * 10;

        let init_name = b"initcode\0";
        unsafe {
            ptr::copy_nonoverlapping(init_name.as_ptr(), tdata.name.as_mut_ptr(), init_name.len());
        }

        debug_assert!(tdata.cwd.is_none());
        tdata.cwd = Some(
            ICACHE
                .namei(&ROOTIPATH)
                .expect("cannot find root inode by b'/'"),
        );
    }

    /// Abondon current task if
    /// the killed flag is true
    pub fn check_abondon(&mut self, exit_status: i32) {
        if self.killed.load(Ordering::Relaxed) {
            if self.is_child_task {
                unsafe {
                    PROC_MANAGER.child_thread_exiting(self.index, exit_status);
                }
            } else {
                unsafe {
                    loop {
                        PROC_MANAGER.exiting(self.index, exit_status);
                        //self.yielding();

                        let mut parent_map = PROC_MANAGER.parents.lock();

                        let channel = self as *const Task as usize;
                        self.sleep(channel, parent_map);
                        parent_map = PROC_MANAGER.parents.lock();
                    }
                }
            }
        }
    }

    /// Abondon current task by:
    /// 1. setting its killed flag to true
    /// 2. and then exit
    pub fn abondon(&mut self, exit_status: i32) {
        self.killed.store(true, Ordering::Relaxed);
        if self.is_child_task {
            unsafe {
                PROC_MANAGER.child_thread_exiting(self.index, exit_status);
            }
        } else {
            unsafe {
                loop {
                    PROC_MANAGER.exiting(self.index, exit_status);
                    //self.yielding();

                    let mut parent_map = PROC_MANAGER.parents.lock();

                    let channel = self as *const Task as usize;
                    self.sleep(channel, parent_map);
                    parent_map = PROC_MANAGER.parents.lock();
                }
            }
        }
    }

    /// # 功能说明
    /// 处理当前进程（主线程）发起的系统调用请求。根据 TrapFrame 中寄存器 a7 指定的系统调用号，
    /// 调用对应的系统调用处理函数，并将返回结果写回寄存器 a0。
    ///
    /// # 流程解释
    /// 1. 使能中断，允许系统中断处理。
    /// 2. 通过不安全代码获取当前进程（主线程）的 TrapFrame 指针，读取系统调用号 `a7`。
    /// 3. 调用 `tf.admit_ecall()`，完成系统调用的相关状态处理（如跳过指令等）。
    /// 4. 使用 `match` 匹配系统调用号，调用对应的系统调用实现函数。
    /// 5. 若系统调用号非法，调用 `panic!` 抛出异常，终止内核执行。
    /// 6. 将系统调用执行结果写入 TrapFrame 的返回寄存器 `a0`，
    ///    成功返回实际结果，失败返回 -1（以 `usize` 格式存储）。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）的可变引用，用于访问其 TrapFrame 和调用系统调用实现。
    ///
    /// # 返回值
    /// - 无返回值，系统调用结果通过 TrapFrame 的 `a0` 寄存器返回给用户态。
    ///
    /// # 可能的错误
    /// - 系统调用号非法时，会导致内核 panic，内核崩溃或重启。
    /// - 各个系统调用具体实现可能返回错误，统一映射为返回值 -1。
    ///
    /// # 安全性
    /// - 使用了 `unsafe` 获取 TrapFrame 裸指针，假设指针有效且唯一所有权。
    /// - 该函数应在内核上下文且进程（主线程）排他访问时调用，避免数据竞争。
    /// - 系统调用执行过程中可能包含更底层的 `unsafe`，调用此函数时需确保整体安全环境。
    pub fn syscall(&mut self) {
        sstatus::intr_on();
        //let trapframe = self.data.get_mut().tasks[0].as_ref().unwrap().get_trap_frame();
        let trapframe = unsafe { self.data.get_mut().trapframe.as_mut().unwrap() };
        let a7 = trapframe.a7;
        trapframe.admit_ecall();
        let sys_result = match a7 {
            1 => self.sys_fork(),
            2 => self.sys_exit(),
            3 => self.sys_wait(),
            4 => self.sys_pipe(),
            5 => self.sys_read(),
            6 => self.sys_kill(),
            7 => self.sys_exec(),
            8 => self.sys_fstat(),
            9 => self.sys_chdir(),
            10 => self.sys_dup(),
            11 => self.sys_getpid(),
            12 => self.sys_sbrk(),
            13 => self.sys_sleep(),
            14 => self.sys_uptime(),
            15 => self.sys_open(),
            16 => self.sys_write(),
            17 => self.sys_mknod(),
            18 => self.sys_unlink(),
            19 => self.sys_link(),
            20 => self.sys_mkdir(),
            21 => self.sys_close(),
            22 => self.sys_getmtime(),
            23 => self.sys_waitpid(),
            24 => self.sys_thread_create(),
            25 => self.sys_thread_count(),
            26 => self.sys_thread_waittid(),
            27 => self.sys_gittid(),
            28 => self.sys_semaphore_create(),
            29 => self.sys_semaphore_up(),
            30 => self.sys_semaphore_down(),
            _ => {
                panic!("unknown syscall num: {}", a7);
            }
        };
        trapframe.a0 = match sys_result {
            Ok(ret) => ret,
            Err(()) => -1isize as usize,
        };
    }

    /// # 功能说明
    /// 让出当前进程（主线程）的 CPU 使用权，将进程（主线程）状态从运行中（RUNNING）
    /// 改为可运行（RUNNABLE），并调用调度器进行上下文切换，
    /// 以便其他进程（主线程）获得执行机会。
    ///
    /// # 流程解释
    /// 1. 获取进程（主线程）的排它锁 `excl`，保证状态修改的线程安全。
    /// 2. 断言当前进程（主线程）状态为 `RUNNING`，确保进程（主线程）处于运行态。
    /// 3. 将进程（主线程）状态设置为 `RUNNABLE`，表示可被调度。
    /// 4. 调用当前 CPU 的调度函数 `sched`，传入当前进程（主线程）的锁保护和上下文，
    ///    进行上下文切换，切换到其他进程（主线程）执行。
    /// 5. 释放锁保护 `guard`。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）的可变引用，用于访问和修改进程（主线程）状态及上下文。
    ///
    /// # 返回值
    /// - 无返回值，完成调度切换。
    ///
    /// # 可能的错误
    /// - 若进程（主线程）当前状态不是 `RUNNING`，断言失败会导致内核 panic。
    /// - 调用 `sched` 函数过程中可能出现不可预期的调度错误。
    ///
    /// # 安全性
    /// - 使用了 `unsafe` 来获取 CPU 当前核心的可变引用，
    ///   假设调度器和 CPU 管理器状态正确且无竞争。
    /// - 调用此函数时应确保进程（主线程）处于正确调度上下文，避免竞态条件。
    /// - 进程（主线程）状态和上下文的修改均在锁保护下进行，保证线程安全。
    pub fn yielding(&mut self) {
        let mut guard = self.excl.lock();
        assert_eq!(guard.state, ProcState::RUNNING);
        guard.state = ProcState::RUNNABLE;
        TaskFifo.lock().add(self as *const Task);
        guard = unsafe {
            CPU_MANAGER
                .my_cpu_mut()
                .sched(guard, self.data.get_mut().get_context())
        };
        drop(guard);
    }

    /// # 功能说明
    /// 原子地释放传入的自旋锁（非进程（主线程）自身的锁），使当前进程（主线程）进入睡眠状态，
    /// 并挂起在指定的等待通道 `channel` 上，等待被唤醒。
    /// 该函数不会在被唤醒后重新获取传入的锁，
    /// 需要调用者在必要时自行重新获取锁。
    ///
    /// # 流程解释
    /// 1. 获取进程（主线程）自身的排它锁 `excl`，确保状态修改的安全性。
    /// 2. 释放传入的外部锁 `guard`，避免死锁（因为进程（主线程）锁必须先获取）。
    /// 3. 设置进程（主线程）等待通道为 `channel`，并将状态修改为 `SLEEPING`。
    /// 4. 调用当前 CPU 的调度器 `sched`，让出 CPU 并切换上下文，进入睡眠。
    /// 5. 睡眠被唤醒后，清空等待通道，释放进程（主线程）锁。
    ///
    /// # 参数
    /// - `&self`：进程（主线程）的不可变引用，用于访问排它锁和上下文。
    /// - `channel`：进程（主线程）挂起等待的通道标识，用于唤醒匹配。
    /// - `guard`：传入的自旋锁保护的锁 guard，必须不是进程（主线程）的排它锁，
    ///   用于在进入睡眠前释放，避免死锁。
    ///
    /// # 返回值
    /// - 无返回值，完成睡眠操作。
    ///
    /// # 可能的错误
    /// - 传入的 `guard` 若为进程（主线程）自身的排它锁，会导致死锁。
    /// - 调用 `sched` 过程中若发生上下文切换异常可能导致系统调度异常。
    ///
    /// # 安全性
    /// - 使用 `unsafe` 代码获取并操作 CPU 相关资源和进程（主线程）上下文，
    ///   需要保证指针有效且调用环境正确。
    /// - 保证在调用时持有适当的锁，避免竞态条件和死锁。
    /// - 进程（主线程）状态和通道的修改均在锁保护下完成，保证线程安全。
    pub fn sleep<T>(&self, channel: usize, guard: SpinLockGuard<'_, T>) {
        // Must acquire p->lock in order to
        // change p->state and then call sched.
        // Once we hold p->lock, we can be
        // guaranteed that we won't miss any wakeup
        // (wakeup locks p->lock),
        // so it's okay to release lk.
        let mut excl_guard = self.excl.lock();
        drop(guard);
        // go to sleep
        excl_guard.channel = channel;
        excl_guard.state = ProcState::SLEEPING;

        unsafe {
            let c = CPU_MANAGER.my_cpu_mut();
            excl_guard = c.sched(excl_guard, &mut (*self.data.get()).context as *mut _);
        }

        excl_guard.channel = 0;
        drop(excl_guard);
    }

    /// # 功能说明
    /// 创建当前进程（主线程）的一个子进程（主线程）（fork），
    /// 复制父进程（主线程）的内存、TrapFrame、打开文件、当前工作目录等信息，
    /// 并将子进程（主线程）状态设置为可运行。
    ///
    /// # 流程解释
    /// 1. 获取当前进程（主线程）的私有数据引用 `pdata`。
    /// 2. 通过 `PROC_MANAGER.alloc_proc()` 分配一个新的子进程（主线程），
    ///    若失败则返回错误 `Err(())`。
    /// 3. 获取子进程（主线程）的排它锁 `cexcl` 和私有数据 `cdata`。
    /// 4. 复制父进程（主线程）的用户内存到子进程（主线程）页表，调用 `uvm_copy`。
    ///    若复制失败，清理子进程（主线程）相关资源，返回错误。
    /// 5. 设置子进程（主线程）的内存大小 `sz` 与父进程（主线程）一致。
    /// 6. 复制 TrapFrame（用户寄存器状态），并将子进程（主线程）的返回值寄存器 `a0` 设为 0。
    /// 7. 克隆父进程（主线程）的打开文件数组和当前工作目录。
    /// 8. 复制父进程（主线程）名称到子进程（主线程）。
    /// 9. 记录子进程（主线程）的进程（主线程） ID（pid）。
    /// 10. 设置子进程（主线程）的父进程（主线程）为当前进程（主线程）。
    /// 11. 将子进程（主线程）状态置为 `RUNNABLE`，表示可调度。
    /// 12. 返回子进程（主线程）的进程（主线程） ID。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）的可变引用，用于访问自身私有数据和状态。
    ///
    /// # 返回值
    /// - `Ok(usize)`：子进程（主线程）的进程（主线程） ID（pid）。
    /// - `Err(())`：分配子进程（主线程）或复制内存失败时返回错误。
    ///
    /// # 可能的错误
    /// - 子进程（主线程）分配失败（如进程（主线程）表满），返回 `Err(())`。
    /// - 复制父进程（主线程）内存失败时，清理子进程（主线程）并返回错误。
    /// - 若 TrapFrame 指针无效，`unsafe` 操作可能导致未定义行为。
    ///
    /// # 安全性
    /// - 使用了多处 `unsafe` 操作，包括裸指针复制和子进程（主线程）数据访问，
    ///   假设指针有效且内存分配正确。
    /// - 调用者需保证进程（主线程）状态和私有数据在调用时无并发冲突。
    /// - 子进程（主线程）资源清理确保不产生内存泄漏和悬挂指针。
    fn fork(&mut self) -> Result<usize, ()> {
        // 为便于后续代码编写，确保fork市当前进程（主线程）中只有主线程
        assert!(self.tasks.lock().len() == 0);
        let tdata = self.data.get_mut();
        let child = unsafe { PROC_MANAGER.alloc_proc().ok_or(())? };
        let mut cexcl = child.excl.lock();
        let cpid = cexcl.pid;
        let cdata = unsafe { child.data.get().as_mut().unwrap() };
        cdata.ustack_base = tdata.ustack_base;
        // clone memory
        let cpgt = cdata.pagetable.as_mut().unwrap();
        let size = tdata.size;
        if unsafe {
            tdata
                .pagetable
                .unwrap()
                .as_mut()
                .unwrap()
                .uvm_copy(cpgt.as_mut().unwrap(), size)
                .is_err()
        } {
            debug_assert_eq!(child.killed.load(Ordering::Relaxed), false);
            child.killed.store(false, Ordering::Relaxed);
            cdata.cleanup(cpid, false);
            cexcl.cleanup();
            return Err(());
        }
        cdata.size = size;

        // clone trapframe and return 0 on a0
        unsafe {
            ptr::copy_nonoverlapping(tdata.trapframe, cdata.trapframe, 1);
            cdata.trapframe.as_mut().unwrap().a0 = 0;
        }

        // clone opened files and cwd
        cdata.open_files.clone_from(&tdata.open_files);
        cdata.cwd.clone_from(&tdata.cwd);

        // copy main task name
        cdata.name.copy_from_slice(&tdata.name);

        let cpid = cexcl.pid;

        drop(cexcl);

        unsafe {
            PROC_MANAGER.set_parent(child.index, self.index);
        }

        let mut cexcl = child.excl.lock();
        cexcl.state = ProcState::RUNNABLE;
        drop(cexcl);

        TaskFifo.lock().add(child as *const Task);

        Ok(cpid)
    }

    fn thread_create(&mut self, entry: usize, arg: usize) -> Result<usize, ()> {
        let pos = self.tasks.lock().len() + 1;
        assert!(pos < MAX_TASKS_PER_PROC);
        let t_ptr = self as *const Task;
        let tdata = self.data.get_mut();
        let child_task = unsafe { PROC_MANAGER.alloc_task(t_ptr).ok_or(())? };
        child_task.is_child_task = true;

        let child_tid = child_task.excl.lock().tid;
        child_task.parent = Some(t_ptr as *mut Task);

        let cdata = unsafe { child_task.data.get().as_mut().unwrap() };
        // 分配子线程的trapframe
        cdata.pagetable = tdata.pagetable.clone();

        let ctrapframe = unsafe { RawSinglePage::try_new_zeroed().unwrap() as usize };
        match unsafe { cdata.pagetable.unwrap().as_mut().unwrap() }.map_pages(
            VirtAddr::from(trapframe_from_tid(child_tid)),
            PAGE_SIZE,
            PhysAddr::try_from(ctrapframe).unwrap(),
            PteFlag::R | PteFlag::W,
        ) {
            Ok(_) => {}
            Err(_) => {
                panic!("memory not enough")
            }
        };
        cdata.trapframe = ctrapframe as _;

        cdata.size = tdata.size;

        cdata.open_files.clone_from(&tdata.open_files);
        cdata.cwd.clone_from(&tdata.cwd);

        // copy main thread name
        cdata.name.copy_from_slice(&tdata.name);

        debug_assert!(cdata.pagetable == tdata.pagetable);
        let mut cexcl = child_task.excl.lock();
        let child_tid = cexcl.tid;
        cexcl.state = ProcState::RUNNABLE;

        // 准备ustack
        let ustack_base = cdata.ustack_base;
        cdata.ustack_base = ustack_base;
        let ctrapframe = unsafe { cdata.trapframe.as_mut().unwrap() };
        ctrapframe.a0 = arg;
        ctrapframe.epc = entry;
        ctrapframe.sp = ustack_bottom_by_pos(ustack_base, pos) + USER_STACK_SIZE;
        // set context
        cdata.init_context();

        drop(cexcl);
        drop(cdata);

        self.tasks.lock().push(Some(child_task as *mut Task));
        TaskFifo.lock().add(child_task as *const Task);

        Ok(child_tid)
    }
}

impl Task {
    /// # 功能说明
    /// 从当前进程（主线程）的 TrapFrame 中获取第 `n` 个系统调用参数的原始值（usize 类型）。
    /// 系统调用参数通过寄存器 a0~a5 传递，`n` 指定参数索引（0 到 5）。
    ///
    /// # 流程解释
    /// 1. 通过不安全代码获取当前进程（主线程）的 TrapFrame 引用，确保指针有效。
    /// 2. 使用 match 匹配参数索引 `n`，返回对应寄存器 a0~a5 的值。
    /// 3. 若 `n` 大于 5，调用 panic 抛出异常，表明参数索引超出范围。
    ///
    /// # 参数
    /// - `&self`：当前进程（主线程）不可变引用，用于访问 TrapFrame。
    /// - `n`：参数索引，范围为 0 至 5。
    ///
    /// # 返回值
    /// - 返回指定参数的原始寄存器值，类型为 usize。
    fn arg_raw(&mut self, n: usize) -> usize {
        let trapframe = unsafe {
            self.data
                .get()
                .as_ref()
                .unwrap()
                .trapframe
                .as_ref()
                .unwrap()
        };
        //let trapframe = self.data.get_mut().tasks[0].as_ref().unwrap().get_trap_frame();
        match n {
            0 => trapframe.a0,
            1 => trapframe.a1,
            2 => trapframe.a2,
            3 => trapframe.a3,
            4 => trapframe.a4,
            5 => trapframe.a5,
            _ => {
                panic!("n is larger than 5")
            }
        }
    }

    /// Fetch 32-bit register value.
    /// Note: `as` conversion is performed between usize and i32
    #[inline]
    fn arg_i32(&mut self, n: usize) -> i32 {
        self.arg_raw(n) as i32
    }

    /// Fetch a raw user virtual address from register value.
    /// Note: This raw address could be null,
    ///     and it might only be used to access user virtual address.
    #[inline]
    fn arg_addr(&mut self, n: usize) -> usize {
        self.arg_raw(n)
    }

    /// # 功能说明
    /// 从指定的系统调用参数寄存器中获取文件描述符（fd）
    /// 并检查该文件描述符是否合法且已被打开。
    ///
    /// # 流程解释
    /// 1. 调用 `arg_raw` 获取第 `n` 个参数的原始值，视为文件描述符。
    /// 2. 检查文件描述符是否超出最大允许值 `NFILE`。
    /// 3. 检查该文件描述符对应的文件是否存在（是否为 `Some`）。
    /// 4. 若检查通过，返回文件描述符；否则返回错误。
    ///
    /// # 参数
    /// - `&mut self`：当前进程（主线程）可变引用，用于访问打开的文件数组。
    /// - `n`：参数索引，指明从第几个寄存器读取文件描述符。
    ///
    /// # 返回值
    /// - `Ok(usize)`：合法且打开的文件描述符。
    /// - `Err(())`：无效或未打开的文件描述符。
    ///
    /// # 可能的错误
    /// - 文件描述符超过允许的最大值 `NFILE`。
    /// - 文件描述符对应的文件句柄为 `None`，表示文件未打开。
    ///
    /// # 安全性
    /// - 该函数内部调用 `arg_raw` 使用了 `unsafe`，需保证寄存器指针有效。
    /// - 读取和判断文件句柄时，确保没有并发修改导致状态不一致。
    #[inline]
    fn arg_fd(&mut self, n: usize) -> Result<usize, ()> {
        let fd = self.arg_raw(n);
        if fd >= NFILE || self.data.get_mut().open_files[fd].is_none() {
            Err(())
        } else {
            Ok(fd)
        }
    }

    /// # 功能说明
    /// 从系统调用参数寄存器中获取一个指向用户空间的字符串指针，
    /// 将该以 null 结尾的字符串复制到内核缓冲区 `buf` 中。
    ///
    /// # 流程解释
    /// 1. 调用 `arg_raw` 获取第 `n` 个参数的原始值，视为用户虚拟地址字符串指针 `addr`。
    /// 2. 通过 `UnsafeCell` 获取当前进程（主线程）的用户页表引用 `pagetable`。
    /// 3. 调用页表的 `copy_in_str` 方法，从用户虚拟地址空间复制字符串到 `buf`。
    /// 4. 若复制成功，返回 `Ok(())`，否则返回错误。
    ///
    /// # 参数
    /// - `&self`：当前进程（主线程）不可变引用，用于访问页表和寄存器。
    /// - `n`：参数索引，指定从第几个寄存器读取字符串指针。
    /// - `buf`：用于存放复制进来的字符串的内核缓冲区。
    ///
    /// # 返回值
    /// - `Ok(())`：字符串复制成功。
    /// - `Err(&'static str)`：复制失败，可能是地址非法或未映射。
    ///
    /// # 可能的错误
    /// - 用户传入的指针非法，超出进程（主线程）地址空间范围。
    /// - 字符串未正确以 null 结尾导致复制失败。
    /// - 页表查找或映射异常。
    ///
    /// # 安全性
    /// - 使用了 `unsafe` 访问裸指针，假设页表和数据有效。
    /// - 复制操作仅读用户空间，不修改数据，安全性较高。
    /// - 需要保证缓冲区 `buf` 大小足够存放用户字符串。
    fn arg_str(&mut self, n: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let addr: usize = self.arg_raw(n);
        let pagetable = unsafe {
            self.data
                .get()
                .as_ref()
                .unwrap()
                .pagetable
                .as_ref()
                .unwrap()
        };
        unsafe { pagetable.as_mut().unwrap().copy_in_str(addr, buf)? };
        Ok(())
    }

    /// # 功能说明
    /// 从用户虚拟地址 `addr` 处读取一个 `usize` 类型的数据。
    /// 用于获取用户空间中存储的地址或数值。
    ///
    /// # 流程解释
    /// 1. 通过不安全代码获取当前进程（主线程）的私有数据引用 `pd`。
    /// 2. 检查请求读取的地址范围是否超出进程（主线程）当前内存大小 `sz`。
    ///    如果越界，返回错误。
    /// 3. 在缓冲变量 `ret` 中为数据分配空间。
    /// 4. 调用 `copy_in` 将用户虚拟地址 `addr` 处的内容复制到内核缓冲 `ret`。
    /// 5. 根据复制结果返回成功的读取值或错误信息。
    ///
    /// # 参数
    /// - `&self`：当前进程（主线程）不可变引用，用于访问其内存数据和页表。
    /// - `addr`：用户虚拟地址，指向待读取的 `usize` 数据。
    ///
    /// # 返回值
    /// - `Ok(usize)`：成功读取用户地址处的数据。
    /// - `Err(&'static str)`：失败，返回错误字符串描述。
    ///
    /// # 可能的错误
    /// - 读取地址超出进程（主线程）内存大小，返回地址越界错误。
    /// - 用户页表的 `copy_in` 操作失败，返回拷贝错误。
    ///
    /// # 安全性
    /// - 依赖不安全代码访问进程（主线程）私有数据指针，假设指针有效且唯一所有权。
    /// - 通过页表安全复制数据，避免直接裸指针访问用户空间，符合内核安全规范。
    /// - 调用者需保证地址合法且缓冲区足够存储数据。
    fn fetch_addr(&self, addr: usize) -> Result<usize, &'static str> {
        let pd = unsafe { self.data.get().as_ref().unwrap() };
        if addr + mem::size_of::<usize>() > pd.size {
            Err("input addr > proc's mem size")
        } else {
            let mut ret: usize = 0;
            match pd.copy_in(
                addr,
                &mut ret as *mut usize as *mut u8,
                mem::size_of::<usize>(),
            ) {
                Ok(_) => Ok(ret),
                Err(_) => Err("pagetable copy_in eror"),
            }
        }
    }

    /// Fetch a null-nullterminated string from virtual address `addr` into the kernel buffer.
    fn fetch_str(&self, addr: usize, dst: &mut [u8]) -> Result<(), &'static str> {
        let pd = unsafe { self.data.get().as_ref().unwrap() };
        unsafe {
            pd.pagetable
                .unwrap()
                .as_ref()
                .unwrap()
                .copy_in_str(addr, dst)
        }
    }
}

/// first user program that calls exec("/init")
static INITCODE: [u8; 51] = [
    0x17, 0x05, 0x00, 0x00, 0x13, 0x05, 0x05, 0x02, 0x97, 0x05, 0x00, 0x00, 0x93, 0x85, 0x05, 0x02,
    0x9d, 0x48, 0x73, 0x00, 0x00, 0x00, 0x89, 0x48, 0x73, 0x00, 0x00, 0x00, 0xef, 0xf0, 0xbf, 0xff,
    0x2f, 0x69, 0x6e, 0x69, 0x74, 0x00, 0x00, 0x01, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00,
];
