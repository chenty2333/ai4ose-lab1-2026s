use tg_kernel_context::LocalContext;
use tg_syscall::{Caller, SyscallId};

/// 任务控制块。
///
/// 包含任务的上下文、状态和资源。
pub struct TaskControlBlock {
    ctx: LocalContext,
    pub finish: bool,
    task_id: usize,
    syscall_count: [usize; 512],
    stack: [usize; 1024], // 8KB 用户栈，避免栈溢出覆盖上下文
}

/// 调度事件。
pub enum SchedulingEvent {
    None,
    Yield,
    Exit(usize),
    UnsupportedSyscall(SyscallId),
}

impl TaskControlBlock {
    pub const ZERO: Self = Self {
        ctx: LocalContext::empty(),
        finish: false,
        task_id: 0,
        syscall_count: [0; 512],
        stack: [0; 1024],
    };

    /// 初始化一个任务。
    pub fn init(&mut self, entry: usize, task_id: usize) {
        self.stack.fill(0);
        self.finish = false;
        self.task_id = task_id;
        self.syscall_count.fill(0);
        self.ctx = LocalContext::user(entry);
        *self.ctx.sp_mut() = self.stack.as_ptr() as usize + core::mem::size_of_val(&self.stack);
    }

    /// 执行此任务。
    #[inline]
    pub unsafe fn execute(&mut self) {
        self.ctx.execute();
    }

    /// 处理系统调用，返回是否应该终止程序。
    pub fn handle_syscall(&mut self) -> SchedulingEvent {
        use tg_syscall::{SyscallId as Id, SyscallResult as Ret};
        use SchedulingEvent as Event;

        let id_raw = self.ctx.a(7);
        let id = id_raw.into();
        if id_raw < self.syscall_count.len() {
            self.syscall_count[id_raw] += 1;
        }
        let args = [
            self.ctx.a(0),
            self.ctx.a(1),
            self.ctx.a(2),
            self.ctx.a(3),
            self.ctx.a(4),
            self.ctx.a(5),
        ];
        let trace_query_count = if id == Id::TRACE && args[0] == 2 {
            self.syscall_count.get(args[1]).copied().unwrap_or(0)
        } else {
            0
        };
        let caller = Caller {
            entity: self.task_id,
            flow: trace_query_count,
        };
        match tg_syscall::handle(caller, id, args) {
            Ret::Done(ret) => match id {
                Id::EXIT => Event::Exit(self.ctx.a(0)),
                Id::SCHED_YIELD => {
                    *self.ctx.a_mut(0) = ret as _;
                    self.ctx.move_next();
                    Event::Yield
                }
                _ => {
                    *self.ctx.a_mut(0) = ret as _;
                    self.ctx.move_next();
                    Event::None
                }
            },
            Ret::Unsupported(_) => Event::UnsupportedSyscall(id),
        }
    }
}
