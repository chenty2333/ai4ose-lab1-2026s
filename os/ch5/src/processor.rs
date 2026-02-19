use crate::process::Process;
use alloc::collections::{BTreeMap, VecDeque};
use core::cell::UnsafeCell;
use tg_task_manage::{Manage, PManager, ProcId, Schedule};

/// stride 调度的大步长常数
const BIG_STRIDE: usize = 0x7fff_ffff;

pub struct Processor {
    inner: UnsafeCell<PManager<Process, ProcManager>>,
}

unsafe impl Sync for Processor {}

impl Processor {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(PManager::new()),
        }
    }

    #[inline]
    pub fn get_mut(&self) -> &mut PManager<Process, ProcManager> {
        unsafe { &mut (*self.inner.get()) }
    }
}

pub static PROCESSOR: Processor = Processor::new();

/// 任务管理器
/// `tasks` 中保存所有的任务实体
/// `ready_queue` 保存就绪进程的 id
pub struct ProcManager {
    tasks: BTreeMap<ProcId, Process>,
    ready_queue: VecDeque<ProcId>,
}

impl ProcManager {
    /// 新建任务管理器
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            ready_queue: VecDeque::new(),
        }
    }
}

impl Manage<Process, ProcId> for ProcManager {
    /// 插入一个新任务
    #[inline]
    fn insert(&mut self, id: ProcId, task: Process) {
        self.tasks.insert(id, task);
    }
    /// 根据 id 获取对应的任务
    #[inline]
    fn get_mut(&mut self, id: ProcId) -> Option<&mut Process> {
        self.tasks.get_mut(&id)
    }
    /// 删除任务实体
    #[inline]
    fn delete(&mut self, id: ProcId) {
        self.tasks.remove(&id);
    }
}

impl Schedule<ProcId> for ProcManager {
    /// 添加 id 进入调度队列
    fn add(&mut self, id: ProcId) {
        self.ready_queue.push_back(id);
    }
    /// stride 调度：从就绪队列中取出 stride 最小的进程
    fn fetch(&mut self) -> Option<ProcId> {
        if self.ready_queue.is_empty() {
            return None;
        }
        let mut min_idx = 0;
        let mut min_stride = usize::MAX;
        for (i, &id) in self.ready_queue.iter().enumerate() {
            if let Some(proc) = self.tasks.get(&id) {
                if proc.stride < min_stride {
                    min_stride = proc.stride;
                    min_idx = i;
                }
            }
        }
        let id = self.ready_queue.remove(min_idx).unwrap();
        // 更新 stride
        if let Some(proc) = self.tasks.get_mut(&id) {
            proc.stride += BIG_STRIDE / proc.priority;
        }
        Some(id)
    }
}
