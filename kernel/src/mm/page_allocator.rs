use crate::{consts::{KERNEL_HEAP_END, PHYSTOP}};
use crate::mm::{addr::PhysPageNum, PhysAddr};
use crate::spinlock::SpinLock;

use core::fmt::{self, Debug, Formatter};
use alloc::vec::Vec;
use lazy_static::*;

pub struct PageTracker {
    pub ppn: PhysPageNum,
}

impl PageTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}

impl Debug for PageTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

impl Drop for PageTracker {
    fn drop(&mut self) {
        //page_dealloc(self.ppn);
    }
}

trait PageAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

pub struct StackPageAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
}

impl StackPageAllocator {
    fn init(&mut self , start: PhysPageNum, end: PhysPageNum)  {
        self.current = start.0;
        self.end = end.0;
    }
}
impl PageAllocator for StackPageAllocator{
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else if self.current == self.end {
            None
        } else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // validity check
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // recycle
        self.recycled.push(ppn);
    }
}

lazy_static! {
    pub static ref PAGE_ALLOCATOR: SpinLock<StackPageAllocator> = SpinLock::new(StackPageAllocator::new(), "PAGEALLOCATOR");
}

pub fn init_page_allocator() {
    let start = unsafe {
        PhysAddr::from(KERNEL_HEAP_END).ceil()
    };
    let end = unsafe {
        PhysAddr::from(PHYSTOP).floor()
    };
    kinfo!("[kernel] pageallocator area [{:08x},{:08x})",PhysAddr::from(start).into_raw(),PhysAddr::from(PHYSTOP).into_raw());
    PAGE_ALLOCATOR.lock().init(start,end);
}

pub fn page_alloc() -> Option<PhysAddr> {
    let res = PAGE_ALLOCATOR.lock().alloc();
    if let None = res {
        panic!("[kernel] memory is not enough");
    }
    let ppn = res.unwrap();
    for i in ppn.get_bytes_array() {
        *i = 0;
    }
    Some(ppn.into())
}

pub fn page_dealloc(pa: PhysAddr) {
    let ppn = pa.into();
    PAGE_ALLOCATOR.lock().dealloc(ppn);
}