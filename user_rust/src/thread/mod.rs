mod task;
mod config;

use alloc::vec;
use alloc::vec::Vec;
use config::{DEFAULT_STACK_SIZE,MAX_TASKS};
use task::{TaskContext,Task,TaskState};

use core::{arch::global_asm, default};
global_asm!(include_str!("./switch.S"));
unsafe extern  "C" {
    fn switch(old: *mut TaskContext, new: *const TaskContext);
}
pub struct Runtime {
    tasks: Vec<Task>,
    current: usize,
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

        Runtime { tasks, current: 0 }
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
        println!("All tasks finished!");
    }
    /// This is our return function. The only place we use this is in our `guard` function.
    /// If the current task is not our base task we set its state to Available. It means
    /// we're finished with it. Then we yield which will schedule a new task to be run.
    fn t_return(&mut self) {
        if self.current != 0 {
            self.tasks[self.current].state = TaskState::Available;
            self.t_yield();
        }
    }

    #[inline(never)]
    fn t_yield(&mut self) -> bool {
        let mut pos = self.current;
        println!("{:x}",pos);
        while self.tasks[pos].state != TaskState::Ready {
            pos += 1;
            if pos == self.tasks.len() {
                pos = 0;
            }
            if pos == self.current {
                return false;
            }
        }

        if self.tasks[self.current].state != TaskState::Available {
            self.tasks[self.current].state = TaskState::Ready;
        }

        self.tasks[pos].state = TaskState::Running;
        let old_pos = self.current;
        self.current = pos;

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
    pub fn spawn(&mut self, f: fn(u64)) {
        let available = self
            .tasks
            .iter_mut()
            .find(|t| t.state == TaskState::Available)
            .expect("no available task.");

        println!("RUNTIME: spawning task {} and r_ptr {:x}", available.id, available.r_ptr);
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
        }
        available.state = TaskState::Ready;
    }
}

use core::arch::asm;
/// This is our guard function that we place on top of the stack. All this function does is set the
/// state of our current task and then `yield` which will then schedule a new task to be run.
fn guard() {

    let value: u64;
    unsafe {
        asm!(
            "mv {}, t1",
            out(reg) value, // 绑定到 Rust 变量
        );
        
        let rt_ptr = value as *mut Runtime;
        (*rt_ptr).t_return();
    };
}

/// We know that Runtime is alive the length of the program and that we only access from one core
/// (so no datarace). We yield execution of the current task  by dereferencing a pointer to our
/// Runtime and then calling `t_yield`
pub fn yield_task(r_ptr: u64) {
    unsafe {
        let value:u64;
        asm!(
            "mv {}, t1", // 将 x27 的值移动到输出寄存器
            out(reg) value, // 绑定到 Rust 变量
        );
        println!("assert {:x} {:x}",value,r_ptr);
        let rt_ptr = r_ptr as *mut Runtime;
        (*rt_ptr).t_yield();
    };
}