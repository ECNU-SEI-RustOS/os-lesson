use syscall_riscv::{sys_close, sys_dup, sys_fstat, sys_link, sys_mkdir, sys_mknod, sys_open, sys_pipe, sys_read, sys_unlink, sys_write };
use bitflags::*;

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}
// #define T_DIR     1   // Directory
// #define T_FILE    2   // File
// #define T_DEVICE  3   // Device

// struct stat {
//   int dev;     // File system's disk device
//   uint ino;    // Inode number
//   short type;  // Type of file
//   short nlink; // Number of links to file
//   uint64 size; // Size of file in bytes
// };
#[derive(Debug)]
pub enum FileT {
    TDIR,
    TFILE,
    TDEVICE,
    TNONE,
}
impl Default for FileT{
    fn default() -> Self {
        Self::TNONE
    }
}

pub const DIRSIZ: usize = 14;
pub const T_DIR: u16 = 1;
pub const T_FILE: u16 = 2;

#[derive(Default,Debug)]
pub struct Stat{
    pub dev: i32,
    pub ino: u32,
    pub ftype: FileT,
    pub nlink: u16,
    pub size: u64
}

#[repr(C)]
pub struct Dirent {
    pub inum: u16,
    pub name: [u8; DIRSIZ],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StatC{
    dev: i32,
    ino: u32,
    ftype: u16,
    nlink: u16,
    size: u64
}

pub fn dup(fd: isize) -> isize {
    sys_dup(fd)
}
pub fn pipe(pipe_fd: &mut [u32]) -> isize {
    sys_pipe(pipe_fd.as_mut_ptr() as *mut u32)
}

pub fn read(fd: isize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}
pub fn write(fd: isize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}
pub fn open(path: &str, flag: OpenFlags) -> isize {
    sys_open(path, flag.bits())
}
pub fn close(fd: isize) -> isize {
    sys_close(fd)
}
pub fn fstat(fd: isize, fstat :&mut Stat)->isize{
    let mut fstat_c = StatC{dev:0,ino:0,ftype:0,nlink:0,size:0};
    let res = sys_fstat(fd, &mut fstat_c as * mut StatC as usize);
    if res == -1 {
        return res;
    }
    fstat.dev = fstat_c.dev;
    fstat.ino = fstat_c.ino;
    fstat.ftype = match fstat_c.ftype {
        1 => FileT::TDIR,
        2 => FileT::TFILE,
        3 => FileT::TDEVICE,
        _ => FileT::TNONE
    };
    fstat.nlink = fstat_c.nlink;
    fstat.size = fstat_c.size;
    res
}

pub fn link(old_path: &str, new_path: &str) -> isize {
    sys_link(old_path, new_path)
}

pub fn unlink(path: &str) -> isize {
    sys_unlink(path)
}

pub fn mkdir(dir_name: &str) -> isize {
    sys_mkdir(dir_name)
}

pub fn mknod(path: &str, major: u16, minor: u16) -> isize {
    sys_mknod(path, major, minor)
}
use syscall_riscv::{sys_close, sys_dup, sys_fstat, sys_link, sys_mkdir, sys_mknod, sys_open, sys_pipe, sys_read, sys_unlink, sys_write };
use bitflags::*;

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}
// #define T_DIR     1   // Directory
// #define T_FILE    2   // File
// #define T_DEVICE  3   // Device

// struct stat {
//   int dev;     // File system's disk device
//   uint ino;    // Inode number
//   short type;  // Type of file
//   short nlink; // Number of links to file
//   uint64 size; // Size of file in bytes
// };
#[derive(Debug)]
pub enum FileT {
    TDIR,
    TFILE,
    TDEVICE,
    TNONE,
}
impl Default for FileT{
    fn default() -> Self {
        Self::TNONE
    }
}

#[derive(Default,Debug)]
pub struct Stat{
    pub dev: i32,
    pub ino: u32,
    pub ftype: FileT,
    pub nlink: u16,
    pub size: u64
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StatC{
    dev: i32,
    ino: u32,
    ftype: u16,
    nlink: u16,
    size: u64
}

pub fn dup(fd: isize) -> isize {
    sys_dup(fd)
}
pub fn pipe(pipe_fd: &mut [u32]) -> isize {
    sys_pipe(pipe_fd.as_mut_ptr() as *mut u32)
}

pub fn read(fd: isize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}
pub fn write(fd: isize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}
pub fn open(path: &str, flag: OpenFlags) -> isize {
    sys_open(path, flag.bits())
}
pub fn close(fd: isize) -> isize {
    sys_close(fd)
}
pub fn fstat(fd: isize, fstat :&mut Stat)->isize{
    let mut fstat_c = StatC{dev:0,ino:0,ftype:0,nlink:0,size:0};
    let res = sys_fstat(fd, &mut fstat_c as * mut StatC as usize);
    if res == -1 {
        return res;
    }
    fstat.dev = fstat_c.dev;
    fstat.ino = fstat_c.ino;
    fstat.ftype = match fstat_c.ftype {
        1 => FileT::TDIR,
        2 => FileT::TFILE,
        3 => FileT::TDEVICE,
        _ => FileT::TNONE
    };
    fstat.nlink = fstat_c.nlink;
    fstat.size = fstat_c.size;
    res
}

pub fn link(old_path: &str, new_path: &str) -> isize {
    sys_link(old_path, new_path)
}

pub fn unlink(path: &str) -> isize {
    sys_unlink(path)
}

pub fn mkdir(dir_name: &str) -> isize {
    sys_mkdir(dir_name)
}

pub fn mknod(path: &str, major: u16, minor: u16) -> isize {
    sys_mknod(path, major, minor)
}
