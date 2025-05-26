use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::cmp::min;
use core::convert::TryInto;

use crate::consts::driver::NDEV;
use crate::consts::fs::{MAXOPBLOCKS, BSIZE};
use crate::consts::fs::{O_RDONLY, O_WRONLY, O_RDWR, O_CREATE, O_TRUNC};
use crate::driver::DEVICES;
use crate::mm::Address;

use super::{ICACHE, LOG, inode::FileStat};
use super::{Inode, InodeType};

mod pipe;

pub use pipe::Pipe;

/// 表示内核中的文件抽象结构，构建在 inode 之上。
///
/// `File` 类型用于统一表示三类文件实体：常规文件（regular file）、设备文件（device）、以及管道（pipe）。
/// 它封装了底层 inode 结构，并通过 `FileInner` 枚举区分实际文件类型。`File` 是用户进程打开文件后在内核态持有的资源，
/// 支持对文件的读写与状态获取等操作，同时在文件关闭时自动释放 inode 或关闭管道端口。
///
/// ### 使用注意：
/// - `File` 使用 `Arc<File>` 管理引用计数，便于在多个线程之间共享；
/// - 文件偏移量通过内部结构中的 `UnsafeCell` 表示，由 inode 锁进行同步；
/// - 打开文件后需调用 `drop` 或将 `Arc` 释放，以触发 inode 或资源的正确回收。
#[derive(Debug)]
pub struct File {
    /// 封装文件内部数据，区分是常规文件、管道还是设备。
    inner: FileInner,

    /// 标志该文件是否支持读取操作。
    readable: bool,

    /// 标志该文件是否支持写入操作。
    writable: bool,
}


unsafe impl Send for File {}
unsafe impl Sync for File {}

impl File {
    /// 打开指定路径的文件，并根据传入的标志位决定是否创建新文件。
    ///
    /// # 功能说明
    /// 该函数提供文件打开功能，支持对常规文件、目录、设备文件进行统一处理。
    /// 若传入 `O_CREATE` 标志，则尝试在路径不存在时创建新文件；否则尝试查找并打开已有文件。
    /// 对于不同类型的 inode，会构造对应的 `FileInner` 实例并初始化可读/可写标志。
    ///
    /// # 流程解释
    /// 1. 启动日志操作（`LOG.begin_op()`），以保障文件系统操作的一致性；
    /// 2. 若指定 `O_CREATE`，尝试使用 `ICACHE.create()` 创建普通文件；
    ///    否则通过 `ICACHE.namei()` 查找现有文件；
    /// 3. 根据 inode 类型判断处理逻辑：
    ///    - 若为 `Directory`，只允许 `O_RDONLY` 打开；
    ///    - 若为 `File`，根据 `O_TRUNC` 标志判断是否截断文件；
    ///    - 若为 `Device`，检查 major 编号合法性并封装为设备文件；
    /// 4. 构造 `File` 结构体并返回其 `Arc` 包装；
    /// 5. 所有路径在出错时需释放 inode 并结束日志操作。
    ///
    /// # 参数
    /// - `path`: 文件路径，使用字节数组形式表示（如 C 字符串）；
    /// - `flags`: 打开标志，支持组合位，如 `O_CREATE`, `O_RDONLY`, `O_WRONLY`, `O_RDWR`, `O_TRUNC` 等。
    ///
    /// # 返回值
    /// - `Some(Arc<File>)`：打开成功时，返回封装的文件对象；
    /// - `None`：打开或创建文件失败时返回。
    ///
    /// # 可能的错误
    /// - 路径不存在且未指定 `O_CREATE`；
    /// - 创建文件失败（如目录不存在或权限问题）；
    /// - 尝试以非只读方式打开目录；
    /// - 打开设备文件但 major 编号非法；
    /// - 日志事务未正确结束（通过提前 return 路径确保处理）。
    ///
    /// # 安全性
    /// - 使用 `Arc<File>` 保证跨线程安全共享；
    /// - `offset` 字段通过 `UnsafeCell` 表示内部可变性，由 inode 锁保护并发访问；
    /// - inode 在函数内生命周期受控，出错路径确保正确释放资源与日志。
    pub fn open(path: &[u8], flags: i32) -> Option<Arc<Self>> {
        LOG.begin_op();

        let inode: Inode;
        if flags & O_CREATE > 0 {
            match ICACHE.create(&path, InodeType::File, 0, 0, true) {
                Some(i) => inode = i,
                None => {
                    LOG.end_op();
                    return None
                }
            }
        } else {
            match ICACHE.namei(&path) {
                Some(i) => inode = i,
                None => {
                    LOG.end_op();
                    return None
                }
            }
        }

        let mut idata = inode.lock();
        let inner;
        let readable = (flags & O_WRONLY) == 0;
        let writable = ((flags & O_WRONLY) | (flags & O_RDWR)) > 0;
        match idata.get_itype() {
            InodeType::Empty => panic!("empty inode"),
            InodeType::Directory => {
                if flags != O_RDONLY {
                    drop(idata); drop(inode); LOG.end_op();
                    return None
                }
                drop(idata);
                inner = FileInner::Regular(FileRegular { offset: UnsafeCell::new(0), inode: Some(inode) });
            },
            InodeType::File => {
                if flags & O_TRUNC > 0 {
                    idata.truncate();
                }
                drop(idata);
                inner = FileInner::Regular(FileRegular { offset: UnsafeCell::new(0), inode: Some(inode) });
            },
            InodeType::Device => {
                let (major, _) = idata.get_devnum();
                if major as usize >= NDEV {
                    drop(idata); drop(inode); LOG.end_op();
                    return None
                }
                drop(idata);
                inner = FileInner::Device(FileDevice { major, inode: Some(inode) });
            }
        }

        LOG.end_op();
        Some(Arc::new(File {
            inner,
            readable,
            writable
        }))
    }

    /// Read from file to user buffer at `addr` in total `count` bytes.
    /// Return the acutal conut of bytes read.
    pub fn fread(&self, addr: usize, count: u32) -> Result<u32, ()> {
        if !self.readable {
            return Err(())
        }

        match self.inner {
            FileInner::Pipe(ref pipe) => pipe.read(addr, count),
            FileInner::Regular(ref file) => {
                let mut idata = file.inode.as_ref().unwrap().lock();
                let offset = unsafe { &mut *file.offset.get() };
                match idata.try_iread(Address::Virtual(addr), *offset, count.try_into().unwrap()) {
                    Ok(read_count) => {
                        *offset += read_count;
                        drop(idata);
                        Ok(read_count)
                    },
                    Err(()) => Err(())
                }
            },
            FileInner::Device(ref dev) => {
                let dev_read = DEVICES[dev.major as usize].as_ref().ok_or(())?.read;
                dev_read(Address::Virtual(addr), count)
            },
        }
    }

    /// Write user data from `addr` to file in total `count` bytes.
    /// Return the acutal conut of bytes written.
    pub fn fwrite(&self, addr: usize, count: u32) -> Result<u32, ()> {
        if !self.writable {
            return Err(())
        }

        match self.inner {
            FileInner::Pipe(ref pipe) => pipe.write(addr, count),
            FileInner::Regular(ref file) => {
                let batch = ((MAXOPBLOCKS-4)/2*BSIZE) as u32;
                let mut addr = Address::Virtual(addr);
                for i in (0..count).step_by(batch as usize) {
                    let write_count = min(batch, count - i);
                    LOG.begin_op();
                    let mut idata = file.inode.as_ref().unwrap().lock();
                    let offset = unsafe { &mut *file.offset.get() };
                    let ret = idata.try_iwrite(addr, *offset, write_count);
                    if let Ok(actual_count) = ret {
                        *offset += actual_count;
                    }
                    drop(idata);
                    LOG.end_op();

                    match ret {
                        Ok(actual_count) => {
                            if actual_count != write_count {
                                return Ok(i+actual_count)
                            }
                        },
                        Err(()) => return Err(()),
                    }
                    addr = addr.offset(write_count as usize);
                }
                Ok(count)
            },
            FileInner::Device(ref dev) => {
                let dev_write = DEVICES[dev.major as usize].as_ref().ok_or(())?.write;
                dev_write(Address::Virtual(addr), count)
            },
        }
    }

    /// Copy the file status to user memory.
    pub fn fstat(&self, stat: &mut FileStat) -> Result<(), ()> {
        let inode: &Inode;
        match self.inner {
            FileInner::Pipe(_) => return Err(()),
            FileInner::Regular(ref file) => inode = file.inode.as_ref().unwrap(),
            FileInner::Device(ref dev) => inode = dev.inode.as_ref().unwrap(),
        }
        let idata = inode.lock();
        idata.istat(stat);
        Ok(())
    }
}

impl Drop for File {
    /// Close the file.
    fn drop(&mut self) {
        match self.inner {
            FileInner::Pipe(ref pipe) => pipe.close(self.writable),
            FileInner::Regular(ref mut file) => {
                LOG.begin_op();
                drop(file.inode.take());
                LOG.end_op();
            },
            FileInner::Device(ref mut dev) => {
                LOG.begin_op();
                drop(dev.inode.take());
                LOG.end_op();
            },
        }
    }
}

#[derive(Debug)]
enum FileInner {
    Pipe(Arc<Pipe>),
    Regular(FileRegular),
    Device(FileDevice),
}

#[derive(Debug)]
struct FileRegular {
    /// offset is protected by inode's lock
    offset: UnsafeCell<u32>,
    inode: Option<Inode>,
}

#[derive(Debug)]
struct FileDevice {
    major: u16,
    inode: Option<Inode>,
}
