//! 第三章：多道程序与分时多任务
//!
//! 本章实现了一个多道程序操作系统，支持多道程序并发执行，能够依次加载并运行多个用户程序。
#![no_std]
#![no_main]
#![cfg_attr(target_arch = "riscv64", deny(warnings, missing_docs))]
#![cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]

mod task;

#[macro_use]
extern crate tg_console;

use core::cell::UnsafeCell;
use impls::{Console, SyscallContext};
use riscv::register::*;
use task::TaskControlBlock;
use tg_console::log;
use tg_sbi;

// 应用程序内联进来。
#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!(env!("APP_ASM")));
// 应用程序数量。
const APP_CAPACITY: usize = 32;

// 用一个包裹类型承载任务表，内部用 UnsafeCell 提供可变访问能力。
struct TaskTable(UnsafeCell<[TaskControlBlock; APP_CAPACITY]>);

// 这里手动实现 Sync：在本章单核执行模型里，任务表由内核串行访问，不存在并发写竞争。
unsafe impl Sync for TaskTable {}

// 把任务表放到静态区（.bss/.data），避免在 rust_main 栈上分配过大数组导致栈溢出。
static TASK_TABLE: TaskTable = TaskTable(UnsafeCell::new([TaskControlBlock::ZERO; APP_CAPACITY]));
// 定义内核入口。
#[cfg(target_arch = "riscv64")]
tg_linker::boot0!(rust_main; stack = (APP_CAPACITY + 2) * 8192);

extern "C" fn rust_main() -> ! {
    // bss 段清零
    unsafe { tg_linker::KernelLayout::locate().zero_bss() };
    // 初始化 `console`
    tg_console::init_console(&Console);
    tg_console::set_log_level(option_env!("LOG").or(Some("info")));
    tg_console::test_log();
    // 初始化 syscall
    tg_syscall::init_io(&SyscallContext);
    tg_syscall::init_process(&SyscallContext);
    tg_syscall::init_scheduling(&SyscallContext);
    tg_syscall::init_clock(&SyscallContext);
    tg_syscall::init_trace(&SyscallContext);
    // 任务控制块
    // 从静态任务表拿到可变引用；生命周期覆盖整个内核主循环。
    let tcbs = unsafe { &mut *TASK_TABLE.0.get() };
    let mut index_mod = 0;
    // 初始化
    for (i, app) in tg_linker::AppMeta::locate().iter().enumerate() {
        let entry = app.as_ptr() as usize;
        log::info!("load app{i} to {entry:#x}");
        // 初始化每个任务时传入它自己的 task_id=i，供 trace 统计按任务隔离。
        tcbs[i].init(entry, i);
        index_mod += 1;
    }
    println!();
    // 打开中断
    unsafe { sie::set_stimer() };
    // 多道执行
    let mut remain = index_mod;
    let mut i = 0usize;
    while remain > 0 {
        let tcb = &mut tcbs[i];
        if !tcb.finish {
            loop {
                #[cfg(not(feature = "coop"))]
                tg_sbi::set_timer(time::read64() + 12500);
                unsafe { tcb.execute() };

                use scause::*;
                let finish = match scause::read().cause() {
                    Trap::Interrupt(Interrupt::SupervisorTimer) => {
                        tg_sbi::set_timer(u64::MAX);
                        log::trace!("app{i} timeout");
                        false
                    }
                    Trap::Exception(Exception::UserEnvCall) => {
                        use task::SchedulingEvent as Event;
                        match tcb.handle_syscall() {
                            Event::None => continue,
                            Event::Exit(code) => {
                                log::info!("app{i} exit with code {code}");
                                true
                            }
                            Event::Yield => {
                                log::debug!("app{i} yield");
                                false
                            }
                            Event::UnsupportedSyscall(id) => {
                                log::error!("app{i} call an unsupported syscall {}", id.0);
                                true
                            }
                        }
                    }
                    Trap::Exception(e) => {
                        log::error!("app{i} was killed by {e:?}");
                        true
                    }
                    Trap::Interrupt(ir) => {
                        log::error!("app{i} was killed by an unexpected interrupt {ir:?}");
                        true
                    }
                };
                if finish {
                    tcb.finish = true;
                    remain -= 1;
                }
                break;
            }
        }
        i = (i + 1) % index_mod;
    }
    tg_sbi::shutdown(false)
}

/// Rust 异常处理函数，以异常方式关机。
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    tg_sbi::shutdown(true)
}

/// 各种接口库的实现
mod impls {
    use tg_syscall::*;

    pub struct Console;

    impl tg_console::Console for Console {
        #[inline]
        fn put_char(&self, c: u8) {
            tg_sbi::console_putchar(c);
        }
    }

    pub struct SyscallContext;

    impl IO for SyscallContext {
        #[inline]
        fn write(&self, _caller: tg_syscall::Caller, fd: usize, buf: usize, count: usize) -> isize {
            match fd {
                STDOUT | STDDEBUG => {
                    print!("{}", unsafe {
                        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                            buf as *const u8,
                            count,
                        ))
                    });
                    count as _
                }
                _ => {
                    tg_console::log::error!("unsupported fd: {fd}");
                    -1
                }
            }
        }
    }

    impl Process for SyscallContext {
        #[inline]
        fn exit(&self, _caller: tg_syscall::Caller, _status: usize) -> isize {
            0
        }
    }

    impl Scheduling for SyscallContext {
        #[inline]
        fn sched_yield(&self, _caller: tg_syscall::Caller) -> isize {
            0
        }
    }

    impl Clock for SyscallContext {
        #[inline]
        fn clock_gettime(
            &self,
            _caller: tg_syscall::Caller,
            clock_id: ClockId,
            tp: usize,
        ) -> isize {
            match clock_id {
                ClockId::CLOCK_MONOTONIC => {
                    let time = riscv::register::time::read() * 10000 / 125;
                    *unsafe { &mut *(tp as *mut TimeSpec) } = TimeSpec {
                        tv_sec: time / 1_000_000_000,
                        tv_nsec: time % 1_000_000_000,
                    };
                    0
                }
                _ => -1,
            }
        }
    }

    impl Trace for SyscallContext {
        #[inline]
        fn trace(
            &self,
            // caller 由 TaskControlBlock::handle_syscall 构造；这里主要使用 caller.flow。
            caller: tg_syscall::Caller,
            // trace_request 决定本次 trace 调用要执行的操作类型（读/写/查计数）。
            trace_request: usize,
            // 语义随 trace_request 变化：
            // - request=0/1 时它是用户指针地址；
            // - request=2 时它是目标 syscall id。
            id: usize,
            // request=1 时作为待写入字节的来源（仅低 8 位有效）。
            data: usize,
        ) -> isize {
            match trace_request {
                // request=0: 把 id 当作 *const u8，从用户地址读取 1 字节并返回。
                0 => unsafe { *(id as *const u8) as isize },
                1 => {
                    // request=1: 把 id 当作 *mut u8，将 data 的低 8 位写回用户地址。
                    unsafe { *(id as *mut u8) = data as u8 };
                    // 约定写成功返回 0。
                    0
                }
                // request=2: 直接返回 handle_syscall 预先计算好的“目标 syscall 调用次数”。
                2 => caller.flow as isize,
                // 其他 request 值都视为非法，按实验要求返回 -1。
                _ => -1,
            }
        }
    }
}

/// 非 RISC-V64 架构的占位实现
#[cfg(not(target_arch = "riscv64"))]
mod stub {
    #[no_mangle]
    pub extern "C" fn main() -> i32 {
        0
    }

    #[no_mangle]
    pub extern "C" fn __libc_start_main() -> i32 {
        0
    }

    #[no_mangle]
    pub extern "C" fn rust_eh_personality() {}
}
