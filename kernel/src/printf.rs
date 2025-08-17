//! 定义系统内核的输出方法

use core::fmt;
use core::panic;
use core::sync::atomic::Ordering;

use crate::driver::{console, PANICKED};
use crate::mm::kvm_pa;
use crate::mm::VirtAddr;
use crate::process::CPU_MANAGER;
use crate::spinlock::SpinLock;

/// 零大小类型（ZST）的打印结构体，用于在多个 CPU 之间对打印操作进行排序。
struct Print;

impl Print {
    /// 向控制台输出单个字符
    ///
    /// # 参数
    /// - `c`: 要输出的ASCII字节
    fn print(&self, c: u8) {
        console::putc(c);
    }
}

impl fmt::Write for Print {
    /// 将字符串写入控制台
    ///
    /// # 参数
    /// - `s`: 要输出的字符串
    ///
    /// # 返回值
    /// fmt::Result 表示操作结果
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.print(byte);
        }
        Ok(())
    }
}

/// 核心打印函数（被宏调用）
///
/// # 功能说明
/// 根据系统状态决定是否加锁输出：
/// - 当系统处于panic状态时，直接输出（不加锁）
/// - 正常状态下使用自旋锁保证多核输出同步
///
/// # 参数
/// - `args`: 格式化参数
///
/// # 注意
/// 此函数被声明为pub，因为需要在宏中调用
pub fn _print(args: fmt::Arguments<'_>) {
    use fmt::Write;
    static PRINT: SpinLock<()> = SpinLock::new((), "print");

    if PANICKED.load(Ordering::Relaxed) {
        // no need to lock
        Print.write_fmt(args).expect("_print: error");
    } else {
        let guard = PRINT.lock();
        Print.write_fmt(args).expect("_print: error");
        drop(guard);
    }
}

/// 在终端输出一串字符
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::printf::_print(format_args!($($arg)*));
    };
}

/// 在终端输出一行字符
#[macro_export]
macro_rules! println {
    () => {$crate::print!("\n")};
    ($fmt:expr) => {$crate::print!(concat!($fmt, "\n"))};
    ($fmt:expr, $($arg:tt)*) => {
        $crate::print!(concat!($fmt, "\n"), $($arg)*)
    };
}

#[macro_export]
macro_rules! kinfo {
    () => {$crate::print!("\n")};
    ($fmt:expr) => {$crate::print!(concat!("\x1b[34m" ,concat!( $fmt, "\x1b[0m\n")))};
    ($fmt:expr, $($arg:tt)*) => {
        $crate::print!(concat!("\x1b[34m",concat!( $fmt , "\x1b[0m\n")), $($arg)*)
    };
}
#[macro_export]
macro_rules! kerror {
    () => {$crate::print!("\n")};
    ($fmt:expr) => {$crate::print!(concat!("\x1b[31m" ,concat!( $fmt, "\x1b[0m\n")))};
    ($fmt:expr, $($arg:tt)*) => {
        $crate::print!(concat!("\x1b[31m",concat!( $fmt , "\x1b[0m\n")), $($arg)*)
    };
}


#[panic_handler]
fn panic(info: &panic::PanicInfo<'_>) -> ! {
    crate::kerror!("{}\n", info);
    PANICKED.store(true, Ordering::Relaxed);
    loop {}
}

/// 内核中止函数
///
/// # 功能说明
/// 触发panic并输出"abort"信息
#[no_mangle]
fn abort() -> ! {
    panic!("abort");
}

/// 单元测试模块
#[cfg(feature = "unit_test")]
pub mod tests {
    use crate::consts::NSMP;
    use crate::proc::cpu_id;
    use core::sync::atomic::{AtomicU8, Ordering};

    /// 多核同步打印测试
    ///
    /// # 测试点
    /// 验证多核环境下println!的同步输出能力：
    /// 1. 使用原子计数器确保所有核心同时开始测试
    /// 2. 每个核心连续输出10行带核心ID的信息
    /// 3. 使用原子计数器确保所有核心完成测试
    pub fn println_simo() {
        let cpu_id = unsafe { cpu_id() };

        // 使用 NSMP 来同步测试 pr 的自旋锁
        static NSMP: AtomicU8 = AtomicU8::new(0);
        NSMP.fetch_add(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != NSMP as u8 {}

        for i in 0..10 {
            println!("println_mul_hart{}: hart {}", i, cpu_id);
        }

        NSMP.fetch_sub(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != 0 {}
    }
}

pub fn backtrace() {
    //let pgt = unsafe { CPU_MANAGER.my_proc().data.get_mut().pagetable.as_mut().unwrap() };
    let mut fp: usize;
    unsafe{
        core::arch::asm!("mv {}, fp", out(reg) fp);
    }
    //println!("fp:0x{:x}", fp);
    let mut ra: usize;
    let barrier: usize = (fp + crate::consts::PGSIZE - 1) & !(crate::consts::PGSIZE - 1);
    println!("backtrace:");
    while fp < barrier {
        ra = unsafe{ kvm_pa(VirtAddr::from_raw(*((fp - 8) as *const usize))) } as usize;
        println!("0x{:x}", ra);
        fp = unsafe { *((fp - 16) as *const usize) };
        //println!("0x{:x}", fp);
    }
}