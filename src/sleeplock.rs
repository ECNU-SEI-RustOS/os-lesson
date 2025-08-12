//! 睡眠锁模块
//!
//! 提供基于进程休眠/唤醒机制的同步原语，适用于可能长时间持有的锁。
//! 当锁被占用时，尝试获取锁的进程会进入休眠状态，避免忙等待。

use core::ops::{Deref, DerefMut, Drop};
use core::cell::{Cell, UnsafeCell};

use crate::process::{CPU_MANAGER, PROC_MANAGER};
use crate::spinlock::SpinLock;

/// 睡眠锁结构，提供阻塞式同步机制
///
/// 与自旋锁不同，当锁被占用时，尝试获取的进程会进入休眠状态，
/// 直到锁被释放后被唤醒。这避免了忙等待，适用于可能长时间持有的锁。
///
/// # 类型参数
/// - `T`: 被保护的数据类型
///
/// # 字段说明
/// - `lock`: 内部自旋锁，保护`locked`状态的原子访问
/// - `locked`: 表示锁是否已被占用
/// - `name`: 锁的标识名称，用于调试
/// - `data`: 被保护的数据，通过`UnsafeCell`实现内部可变性
pub struct SleepLock<T: ?Sized> {
    lock: SpinLock<()>,
    locked: Cell<bool>,
    name: &'static str,
    data: UnsafeCell<T>,
}

// 为SleepLock实现Sync，允许跨线程共享（要求T是Send）
unsafe impl<T: ?Sized + Send> Sync for SleepLock<T> {}

// 不需要
// unsafe impl<T: ?Sized + Send> Send for SleepLock<T> {}

impl<T> SleepLock<T> {
    /// 创建一个新的睡眠锁实例
    ///
    /// # 参数
    /// - `data`: 需要被保护的数据
    /// - `name`: 锁的标识名称
    ///
    /// # 返回值
    /// 初始化完成的`SleepLock<T>`实例
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            lock: SpinLock::new((), "sleeplock"),
            locked: Cell::new(false),
            name,
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> SleepLock<T> {
    /// blocking, might sleep if this sleeplock is already locked
    pub fn lock(&self) -> SleepLockGuard<'_, T> {
        let mut guard = self.lock.lock();
        while self.locked.get() {
            unsafe {
                CPU_MANAGER.my_proc().sleep(self.locked.as_ptr() as usize, guard);
            }
            guard = self.lock.lock();
        }
        self.locked.set(true);
        drop(guard);
        SleepLockGuard {
            lock: &self,
            data: unsafe { &mut *self.data.get() }
        }
    }

    /// Called by its guard when dropped
    fn unlock(&self) {
        let guard = self.lock.lock();
        self.locked.set(false);
        self.wakeup();
        drop(guard);
    }

    fn wakeup(&self) {
        unsafe {
            PROC_MANAGER.wakeup(self.locked.as_ptr() as usize);
        }
    }
}

pub struct SleepLockGuard<'a, T: ?Sized> {
    lock: &'a SleepLock<T>,
    data: &'a mut T,
}

impl<'a, T: ?Sized> Deref for SleepLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &*self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SleepLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.data
    }
}

impl<'a, T: ?Sized> Drop for SleepLockGuard<'a, T> {
    /// The dropping of the SpinLockGuard will call spinlock's release_lock(),
    /// through its reference to its original spinlock.
    fn drop(&mut self) {
        self.lock.unlock();
    }
}
