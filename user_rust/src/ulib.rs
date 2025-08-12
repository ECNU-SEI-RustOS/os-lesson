use crate::file::Stat;


pub unsafe fn strcpy(dst: *mut u8, src: *const u8) -> *mut u8 {
    let mut d = dst;
    let mut s = src;
    while *s != 0 {
        *d = *s;
        d = d.add(1);
        s = s.add(1);
    }
    *d = 0;
    dst
}

pub unsafe fn strcmp(p: *const u8, q: *const u8) -> i32 {
    let mut p1 = p;
    let mut q1 = q;
    while *p1 != 0 && *p1 == *q1 {
        p1 = p1.add(1);
        q1 = q1.add(1);
    }
    (*p1 as u8 as i32) - (*q1 as u8 as i32)
}

pub unsafe fn strlen(s: *const u8) -> usize {
    let mut len = 0;
    let mut p = s;
    while *p != 0 {
        len += 1;
        p = p.add(1);
    }
    len
}

pub unsafe fn memset(dst: *mut u8, c: u8, n: usize) -> *mut u8 {
    for i in 0..n {
        *dst.add(i) = c;
    }
    dst
}

pub unsafe fn strchr(s: *const u8, c: u8) -> *const u8 {
    let mut p = s;
    while *p != 0 {
        if *p == c {
            return p;
        }
        p = p.add(1);
    }
    core::ptr::null()
}

pub unsafe fn gets(buf: *mut u8, max: usize) -> *mut u8 {
    let mut i = 0;
    let mut c = 0u8;
    while i + 1 < max {
        let n = read(0, &mut c as *mut u8, 1);
        if n < 1 {
            break;
        }
        *buf.add(i) = c;
        i += 1;
        if c == b'\n' || c == b'\r' {
            break;
        }
    }
    *buf.add(i) = 0;
    buf
}

extern "C" {
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn open(path: *const u8, flags: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn fstat(fd: i32, st: *mut Stat) -> i32;
}

pub const O_RDONLY: i32 = 0;


pub unsafe fn stat(n: *const u8, st: *mut Stat) -> i32 {
    let fd = open(n, O_RDONLY);
    if fd < 0 {
        return -1;
    }
    let r = fstat(fd, st);
    close(fd);
    r
}

pub unsafe fn atoi(s: *const u8) -> i32 {
    let mut n = 0;
    let mut p = s;
    while *p >= b'0' && *p <= b'9' {
        n = n * 10 + (*p - b'0') as i32;
        p = p.add(1);
    }
    n
}

pub unsafe fn memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if src > dst as *const u8 {
        for i in 0..n {
            *dst.add(i) = *src.add(i);
        }
    } else {
        for i in (0..n).rev() {
            *dst.add(i) = *src.add(i);
        }
    }
    dst
}

pub unsafe fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return (a as i32) - (b as i32);
        }
    }
    0
}

pub unsafe fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    memmove(dst, src, n)
}
