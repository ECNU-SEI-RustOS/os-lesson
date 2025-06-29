mod task;
mod config;
mod mutex;
use alloc::vec;
use alloc::vec::Vec;

use config::{DEFAULT_STACK_SIZE,MAX_TASKS};
use task::{TaskContext,Task,TaskState};
use mutex::*;
use core::{arch::global_asm};


global_asm!(include_str!("./switch.S"));
unsafe extern  "C" {
    fn switch(old: *mut TaskContext, new: *const TaskContext);
}
pub struct Runtime {
    tasks: Vec<Task>,
    current: usize,
    waits: Vec<usize>
}

impl Runtime{
    pub fn new() -> Self {
        // This will be our base task, which will be initialized in the `running` state
        let base_task = Task {
            id: 0,
            stack: vec![0_u8; DEFAULT_STACK_SIZE],
            ctx: TaskContext::default(),
            state: TaskState::Running,
            r_ptr: 0
        };

        // We initialize the rest of our tasks.
        let mut tasks = vec![base_task];
        let mut available_tasks: Vec<Task> = (1..MAX_TASKS).map(|i| Task::new(i)).collect();
        tasks.append(&mut available_tasks);
        let waits = vec![0;MAX_TASKS];
        Runtime { tasks, current: 0, waits}
    }

    /// This is cheating a bit, but we need a pointer to our Runtime stored so we can call yield on it even if
    /// we don't have a reference to it.
    pub fn init(&mut self) -> u64 {
        unsafe {
            let r_ptr: *const Runtime = self;
            let r_ptr = r_ptr as u64;
            for t in self.tasks.iter_mut(){
                t.r_ptr = r_ptr;
            }
            r_ptr
        }
    }
    /// This is where we start running our runtime. If it is our base task, we call yield until
    /// it returns false (which means that there are no tasks scheduled) and we are done.
    pub fn run(&mut self) {
        while self.t_yield() {}
    }
    /// This is our return function. The only place we use this is in our `guard` function.
    /// If the current task is not our base task we set its state to Available. It means
    /// we're finished with it. Then we yield which will schedule a new task to be run.
    fn t_return(&mut self) {
        for (index, value) in self.waits.iter_mut().enumerate() {
            if *value == usize::MAX || *value == self.current {
                *value = 0;
                self.tasks[index].state = TaskState::Ready;
            }
        }
        if self.current != 0 {
            self.tasks[self.current].state = TaskState::Available;
            self.t_yield();
        }
    }
    fn t_wait(&mut self, id: usize) {
        if self.current != 0 {
            self.tasks[self.current].state = TaskState::Sleep;
            if id == 0 {
                self.waits[self.current] = usize::MAX;
            } else {
                self.waits[self.current] = id;
            }
            self.t_yield();
        }
    }
    fn t_signal(&mut self, id: usize){
        if self.waits[id] == usize::MAX {
            self.tasks[id].state = TaskState::Ready;
            self.waits[id] = 0;
        }else if self.waits[id] == self.current {
            self.tasks[id].state = TaskState::Ready;
            self.waits[id] = 0;
        }
        self.t_yield();
    }
    fn t_gettid(&mut self) -> usize{
        self.current
    }
    #[inline(never)]
    fn t_yield(&mut self) -> bool {
        let mut pos = (self.current + 1) % MAX_TASKS;
        let mut temp = 0usize;
        while self.tasks[pos].state == TaskState::Sleep || self.tasks[pos].state == TaskState::Available {
            pos += 1;
            if pos == self.tasks.len() {
                pos = 1;
                if temp == 1 {
                    pos = 0;
                }
                temp = 1;
            }
            if pos == 0 && pos == self.current {
                if !self.waits.iter().any(|&x| x != 0){
                    return false;
                }
            }

        }
        if self.tasks[self.current].state != TaskState::Sleep {
            if self.tasks[self.current].state != TaskState::Available {
                self.tasks[self.current].state = TaskState::Ready;
            }
        }


        self.tasks[pos].state = TaskState::Running;
        let old_pos = self.current;
        self.current = pos;
        if old_pos == pos {
            return  self.tasks.len() > 0
        }
        unsafe {
            switch(&mut self.tasks[old_pos].ctx, &self.tasks[pos].ctx);
        }

        // NOTE: this might look strange and it is. Normally we would just mark this as `unreachable!()` but our compiler
        // is too smart for it's own good so it optimized our code away on release builds. Curiously this happens on windows
        // and not on linux. This is a common problem in tests so Rust has a `black_box` function in the `test` crate that
        // will "pretend" to use a value we give it to prevent the compiler from eliminating code. I'll just do this instead,
        // this code will never be run anyways and if it did it would always be `true`.
        self.tasks.len() > 0
    }

    /// While `yield` is the logically interesting function I think this the technically most interesting.
    ///
    /// When we spawn a new task we first check if there are any available tasks (tasks in `Parked` state).
    /// If we run out of tasks we panic in this scenario but there are several (better) ways to handle that.
    /// We keep things simple for now.
    ///
    /// When we find an available task we get the stack length and a pointer to our u8 bytearray.
    ///
    /// The next part we have to use some unsafe functions. First we write an address to our `guard` function
    /// that will be called if the function we provide returns. Then we set the address to the function we
    /// pass inn.
    ///
    /// Third, we set the value of `sp` which is the stack pointer to the address of our provided function so we start
    /// executing that first when we are scheuled to run.
    ///
    /// Lastly we set the state as `Ready` which means we have work to do and is ready to do it.
    pub fn spawn(&mut self, f: fn(*const Runtime, u64), params: u64) -> usize {
        let available = self
            .tasks
            .iter_mut()
            .find(|t| t.state == TaskState::Available)
            .expect("no available task.");

        //println!("RUNTIME: spawning task {} and r_ptr {:x}", available.id, available.r_ptr);
        let size = available.stack.len();
        unsafe {
            let s_ptr = available.stack.as_mut_ptr().offset(size as isize);

            // make sure our stack itself is 8 byte aligned - it will always
            // offset to a lower memory address. Since we know we're at the "high"
            // memory address of our allocated space, we know that offsetting to
            // a lower one will be a valid address (given that we actually allocated)
            // enough space to actually get an aligned pointer in the first place).
            let s_ptr = (s_ptr as usize & !7) as *mut u8;

            available.ctx.x1 = guard as u64; //ctx.x1  is old return address
            available.ctx.nx1 = f as u64; //ctx.nx2 is new return address
            available.ctx.x2 = s_ptr.offset(-32) as u64; //cxt.x2 is sp
            available.ctx.r_ptr = available.r_ptr;
            available.ctx.params = params;
        }
        available.state = TaskState::Ready;

        available.id
    }
}

/// This is our guard function that we place on top of the stack. All this function does is set the
/// state of our current task and then `yield` which will then schedule a new task to be run.
pub fn guard(r_ptr: *const Runtime) {
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_return();
    };
}

/// We know that Runtime is alive the length of the program and that we only access from one core
/// (so no datarace). We yield execution of the current task  by dereferencing a pointer to our
/// Runtime and then calling `t_yield`
pub fn yield_task(r_ptr: *const Runtime) {
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_yield();
    };
}
/// wait for other tasks signaling. If id is 0, wait for any of tasks signaling
pub fn waittid_task(r_ptr: *const Runtime, id: usize) {
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_wait(id);
    };
}
pub fn wait_task(r_ptr: *const Runtime){
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_wait(0);
    };
}

pub fn signal_task(r_ptr: *const Runtime, id: usize) {
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_signal(id);
    };
}

pub fn gettid_task(r_ptr: *const Runtime)-> usize{
    unsafe {
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_gettid()
    }
}