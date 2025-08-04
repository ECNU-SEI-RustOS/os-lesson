use core::fmt::{self, Write};

const STDIN: usize = 0;
const STDOUT: usize = 1;

use super::file::{read, write};

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write(STDOUT as isize, s.as_bytes());
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

#[macro_export]
#[allow_internal_unstable(print_internals)]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::macros::print($crate::format_args!($($arg)*));
    }};
}

#[macro_export]
#[allow_internal_unstable(print_internals, format_args_nl)]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::macros::print(format_args_nl!($($arg)*));
    }};
}

