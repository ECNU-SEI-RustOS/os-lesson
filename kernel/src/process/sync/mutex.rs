use crate::spinlock::SpinLock;



pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
}


pub struct MutexSpin {
    locked: SpinLock<usize>
}