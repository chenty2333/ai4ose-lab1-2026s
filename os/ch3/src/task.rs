use tg_kernel_context::LocalContext;
use tg_syscall::{Caller, SyscallId};

/// 任务控制块。
///
/// 包含任务的上下文、状态和资源。
pub struct TaskControlBlock {
    ctx: LocalContext,
    pub finish: bool,
    // 当前任务在任务数组里的编号，用于在 syscall 中标识“调用者是谁”。
    task_id: usize,
    // 记录“本任务”每个 syscall id 的调用次数：syscall_count[id] = 次数。
    // 512 足够覆盖本章会用到的 syscall 编号（包含 TRACE=410）。
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
        // 默认任务编号先置 0，真实值会在 init() 时写入。
        task_id: 0,
        // 默认所有 syscall 计数都为 0。
        syscall_count: [0; 512],
        stack: [0; 1024],
    };

    /// 初始化一个任务。
    /// `entry` 是用户程序入口地址，`task_id` 是该任务在任务表中的下标。
    pub fn init(&mut self, entry: usize, task_id: usize) {
        self.stack.fill(0);
        self.finish = false;
        // 记录当前任务编号，后续通过 Caller.entity 传到 syscall 组件层。
        self.task_id = task_id;
        // 每次任务重新初始化时都清空计数，防止上次运行残留影响本次结果。
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

        // a7 按 RISC-V 调用约定保存 syscall id。
        let id_raw = self.ctx.a(7);
        // 把 usize 形式的 syscall id 转成 tg_syscall::SyscallId 便于后续 match。
        let id = id_raw.into();
        // 先计数再处理：这样 trace_request=2 查询时，当前这次调用会被计入统计。
        if id_raw < self.syscall_count.len() {
            self.syscall_count[id_raw] += 1;
        }
        // 按约定提取 syscall 参数（a0..a5）。
        let args = [
            self.ctx.a(0),
            self.ctx.a(1),
            self.ctx.a(2),
            self.ctx.a(3),
            self.ctx.a(4),
            self.ctx.a(5),
        ];
        // 仅当本次调用是 trace(2, target_id, _) 时，预先计算 target_id 的计数结果。
        // 这个结果通过 Caller.flow 传给 Trace::trace，避免引入全局可变状态。
        let trace_query_count = if id == Id::TRACE && args[0] == 2 {
            // args[1] 是用户传入要查询的 syscall id，越界时按 0 处理。
            self.syscall_count.get(args[1]).copied().unwrap_or(0)
        } else {
            // 非 trace(2) 路径不需要该值，置 0 即可。
            0
        };
        // entity 表示任务身份；flow 这里被复用为“trace(2) 查询值”传给 Trace 组件实现。
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
