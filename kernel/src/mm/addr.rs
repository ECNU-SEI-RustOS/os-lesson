use core::convert::TryFrom;
use core::result::Result;
use core::ops::{Add, Sub};
use core::fmt::{self, Debug, Formatter};

use crate::consts::{ConstAddr, PAGE_SIZE_BITS, MAXVA, PGMASK, PGMASKLEN, PGSHIFT, PAGE_SIZE, PHYSTOP};
use crate::mm::pagetable::PageTableEntry;

const PA_WIDTH_SV39: usize = 56;
const VA_WIDTH_SV39: usize = 39;
const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;
const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;

pub trait Addr {
    fn data_ref(&self) -> &usize;

    fn data_mut(&mut self) -> &mut usize;

    #[inline]
    fn pg_round_up(&mut self) {
        *self.data_mut() = (*self.data_mut() + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
    }

    #[inline]
    fn pg_round_down(&mut self) {
        *self.data_mut() = *self.data_mut() & !(PAGE_SIZE - 1)
    }

    #[inline]
    fn add_page(&mut self) {
        *self.data_mut() += PAGE_SIZE;
    }

    #[inline]
    fn as_usize(&self) -> usize {
        *self.data_ref()
    }

    #[inline]
    fn as_ptr(&self) -> *const u8 {
        *self.data_ref() as *const u8
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        *self.data_mut() as *mut u8
    }
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]

pub struct PhysAddr(usize);

impl PhysAddr {
    pub fn get_ref<T>(&self) -> &'static T {
        unsafe { (self.0 as *const T).as_ref().unwrap() }
    }
    pub fn get_mut<T>(&self) -> &'static mut T {
        unsafe { (self.0 as *mut T).as_mut().unwrap() }
    }
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }
    pub fn ceil(&self) -> PhysPageNum {
        if self.0 == 0 {
            PhysPageNum(0)
        } else {
            PhysPageNum((self.0 - 1 + PAGE_SIZE) / PAGE_SIZE)
        }
    }
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
}

impl Addr for PhysAddr {
    #[inline]
    fn data_ref(&self) -> &usize {
        &self.0
    }

    #[inline]
    fn data_mut(&mut self) -> &mut usize {
        &mut self.0
    }
}

impl PhysAddr {
    /// Construct a [`PhysAddr`] from a trusted usize.
    /// SAFETY: The caller should ensure that the raw usize is acutally a valid [`PhysAddr`].
    #[inline]
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    /// Leak the [`PhysAddr`]'s inner address.
    #[inline]
    pub fn into_raw(self) -> usize {
        self.0
    }
}

impl TryFrom<usize> for PhysAddr {
    type Error = &'static str;

    fn try_from(addr: usize) -> Result<Self, Self::Error> {
        if addr % PAGE_SIZE != 0 {
            return Err("PhysAddr addr not aligned");
        }
        if addr > usize::from(PHYSTOP) {
            return Err("PhysAddr addr bigger than HEAP_END");
        }
        Ok(PhysAddr(addr))
    }
}

impl From<ConstAddr> for PhysAddr {
    fn from(const_addr: ConstAddr) -> Self {
        Self(const_addr.into())
    }
}

/// Wrapper of usize to represent the virtual address
///
/// For 64-bit virtual address, it guarantees that 38-bit to 63-bit are zero
/// reason for 38 instead of 39, from xv6-riscv:
/// one beyond the highest possible virtual address.
/// MAXVA is actually one bit less than the max allowed by
/// Sv39, to avoid having to sign-extend virtual addresses
/// that have the high bit set.
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct VirtAddr(usize);

impl Addr for VirtAddr {
    #[inline]
    fn data_ref(&self) -> &usize {
        &self.0
    }

    #[inline]
    fn data_mut(&mut self) -> &mut usize {
        &mut self.0
    }
}

impl VirtAddr {
    /// Construct a [`VirtAddr`] from a trusted usize.
    /// SAFETY: The caller should ensure that the raw usize is acutally a valid [`VirtAddr`].
    #[inline]
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    /// Leak the [`VirtAddr`]'s inner address.
    #[inline]
    pub fn into_raw(self) -> usize {
        self.0
    }

    /// retrieve the vpn\[level\] of the virtual address
    /// only accepts level that is between 0 and 2
    #[inline]
    pub fn page_num(&self, level: usize) -> usize {
        (self.0 >> (PGSHIFT + level * PGMASKLEN)) & PGMASK
    }

    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }
    pub fn ceil(&self) -> VirtPageNum {
        if self.0 == 0 {
            VirtPageNum(0)
        } else {
            VirtPageNum((self.0 - 1 + PAGE_SIZE) / PAGE_SIZE)
        }
    }
    #[inline]
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    #[inline]
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
}

impl TryFrom<usize> for VirtAddr {
    type Error = &'static str;

    fn try_from(addr: usize) -> Result<Self, Self::Error> {
        if addr > MAXVA.into() {
            Err("value for VirtAddr should be smaller than 1<<38")
        } else {
            Ok(Self(addr))
        }
    }
}

impl From<ConstAddr> for VirtAddr {
    fn from(const_addr: ConstAddr) -> Self {
        Self(const_addr.into())
    }
}

impl Add for VirtAddr {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for VirtAddr {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}


#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);


impl PhysPageNum {
    pub fn get_pte_array(&self) -> &'static mut [PageTableEntry] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut PageTableEntry, 512) }
    }
    pub fn get_bytes_array(&self) -> &'static mut [u8] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut u8, 4096) }
    }
    pub fn get_mut<T>(&self) -> &'static mut T {
        let pa: PhysAddr = (*self).into();
        pa.get_mut()
    }
}

impl From<usize> for PhysPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << PPN_WIDTH_SV39) - 1))
    }
}
impl From<PhysAddr> for usize {
    fn from(v: PhysAddr) -> Self {
        v.0
    }
}
impl From<PhysPageNum> for usize {
    fn from(v: PhysPageNum) -> Self {
        v.0
    }
}
impl From<PhysAddr> for PhysPageNum {
    fn from(v: PhysAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}
impl From<PhysPageNum> for PhysAddr {
    fn from(v: PhysPageNum) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
    }
}
#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtPageNum(pub usize);

impl VirtPageNum {
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0usize; 3];
        for i in (0..3).rev() {
            idx[i] = vpn & 511;
            vpn >>= 9;
        }
        idx
    }
}

impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VPN_WIDTH_SV39) - 1))
    }
}

impl From<VirtAddr> for usize {
    fn from(v: VirtAddr) -> Self {
        if v.0 >= (1 << (VA_WIDTH_SV39 - 1)) {
            v.0 | (!((1 << VA_WIDTH_SV39) - 1))
        } else {
            v.0
        }
    }
}
impl From<VirtPageNum> for usize {
    fn from(v: VirtPageNum) -> Self {
        v.0
    }
}

impl From<VirtAddr> for VirtPageNum {
    fn from(v: VirtAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}
impl From<VirtPageNum> for VirtAddr {
    fn from(v: VirtPageNum) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
    }
}


impl Debug for VirtAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VA:{:#x}", self.0))
    }
}
impl Debug for VirtPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VPN:{:#x}", self.0))
    }
}
impl Debug for PhysAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PA:{:#x}", self.0))
    }
}
impl Debug for PhysPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PPN:{:#x}", self.0))
    }
}


pub trait StepByOne {
    fn step(&mut self);
}
impl StepByOne for VirtPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}
impl StepByOne for PhysPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}

#[derive(Copy, Clone)]
pub struct SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    l: T,
    r: T,
}
impl<T> SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    pub fn new(start: T, end: T) -> Self {
        assert!(start <= end, "start {:?} > end {:?}!", start, end);
        Self { l: start, r: end }
    }
    pub fn get_start(&self) -> T {
        self.l
    }
    pub fn get_end(&self) -> T {
        self.r
    }
}
impl<T> IntoIterator for SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;
    type IntoIter = SimpleRangeIterator<T>;
    fn into_iter(self) -> Self::IntoIter {
        SimpleRangeIterator::new(self.l, self.r)
    }
}
pub struct SimpleRangeIterator<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    current: T,
    end: T,
}
impl<T> SimpleRangeIterator<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    pub fn new(l: T, r: T) -> Self {
        Self { current: l, end: r }
    }
}
impl<T> Iterator for SimpleRangeIterator<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            None
        } else {
            let t = self.current;
            self.current.step();
            Some(t)
        }
    }
}