use crate::task::{kill, getpid};
use crate::println;

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
