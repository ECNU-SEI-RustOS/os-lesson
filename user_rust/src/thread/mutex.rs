use alloc::vec::Vec;

use crate::thread::{signal_task, yield_task, Runtime};



pub struct Mutex<T> {
    resource: T,
    locked: bool,
    waits: Vec<usize>
}

impl<T> Mutex<T>{
    fn new(resource: T) -> Self {
        Mutex { resource, locked: false, waits: Vec::new() }
    }

    fn get(&mut self,r_ptr: *const Runtime) -> &mut T {
        while self.locked{
            yield_task(r_ptr);
        }
        self.locked == true;
        &mut self.resource
    }

}

impl<T> Drop for Mutex<T>{
    fn drop(&mut self) {
        self.locked = false;
    }
}