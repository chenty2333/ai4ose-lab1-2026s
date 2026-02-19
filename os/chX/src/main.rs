//! Chapter X: Buddy Allocator
//!
//! 本章在 ch4 的基础上，将内核堆分配器替换为学生自己实现的 buddy allocator。
#![no_std]
#![no_main]
#![cfg_attr(target_arch = "riscv64", deny(warnings, missing_docs))]
#![cfg_attr(not(target_arch = "riscv64"), allow(dead_code, unused_imports))]

#[allow(dead_code, unused_variables, unused_imports)]
mod allocator;
mod process;

#[macro_use]
extern crate tg_console;

extern crate alloc;

use crate::{
    impls::{Sv39Manager, SyscallContext},
    process::Process,
};
use alloc::{alloc::alloc, vec::Vec};
use core::{alloc::Layout, cell::UnsafeCell};
use impls::Console;
use riscv::register::*;
#[cfg(not(target_arch = "riscv64"))]
use stub::Sv39;
use tg_console::log;
use tg_kernel_context::{foreign::MultislotPortal, LocalContext};
#[cfg(target_arch = "riscv64")]
use tg_kernel_vm::page_table::Sv39;
use tg_kernel_vm::{
    page_table::{MmuMeta, VAddr, VmFlags, VmMeta, PPN, VPN},
    AddressSpace,
};
use tg_sbi;
use tg_syscall::Caller;
use xmas_elf::ElfFile;

/// 构建 VmFlags。
#[cfg(target_arch = "riscv64")]
const fn build_flags(s: &str) -> VmFlags<Sv39> {
    VmFlags::build_from_str(s)
}

/// 解析 VmFlags。
#[cfg(target_arch = "riscv64")]
fn parse_flags(s: &str) -> Result<VmFlags<Sv39>, ()> {
    s.parse()
}

#[cfg(not(target_arch = "riscv64"))]
use stub::{build_flags, parse_flags};

// 应用程序内联进来。
#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!(env!("APP_ASM")));
// 定义内核入口。
#[cfg(target_arch = "riscv64")]
tg_linker::boot0!(rust_main; stack = 6 * 4096);
// 物理内存容量 = 24 MiB。
const MEMORY: usize = 24 << 20;
// 传送门所在虚页。
const PROTAL_TRANSIT: VPN<Sv39> = VPN::MAX;
// 进程列表。
struct ProcessList(UnsafeCell<Vec<Process>>);

unsafe impl Sync for ProcessList {}

impl ProcessList {
    const fn new() -> Self {
        Self(UnsafeCell::new(Vec::new()))
    }

    unsafe fn get_mut(&self) -> &mut Vec<Process> {
        &mut *self.0.get()
    }
}

static PROCESSES: ProcessList = ProcessList::new();

extern "C" fn rust_main() -> ! {
    let layout = tg_linker::KernelLayout::locate();
    // bss 段清零
    unsafe { layout.zero_bss() };
    // 初始化 `console`
    tg_console::init_console(&Console);
    tg_console::set_log_level(option_env!("LOG"));
    tg_console::test_log();
    // 初始化内核堆（buddy allocator）
    unsafe {
        allocator::init(layout.end(), MEMORY - layout.len());
    };
    // 建立异界传送门
    let portal_size = MultislotPortal::calculate_size(1);
    let portal_layout = Layout::from_size_align(portal_size, 1 << Sv39::PAGE_BITS).unwrap();
    let portal_ptr = unsafe { alloc(portal_layout) };
    assert!(portal_layout.size() < 1 << Sv39::PAGE_BITS);
    // 建立内核地址空间
    let mut ks = kernel_space(layout, MEMORY, portal_ptr as _);
    let portal_idx = PROTAL_TRANSIT.index_in(Sv39::MAX_LEVEL);
    // 加载应用程序
    for (i, elf) in tg_linker::AppMeta::locate().iter().enumerate() {
        let base = elf.as_ptr() as usize;
        log::info!("detect app[{i}]: {base:#x}..{:#x}", base + elf.len());
        if let Some(process) = Process::new(ElfFile::new(elf).unwrap()) {
            // 映射异界传送门
            process.address_space.root()[portal_idx] = ks.root()[portal_idx];
            unsafe { PROCESSES.get_mut().push(process) };
        }
    }

    // 建立调度栈
    const PAGE: Layout =
        unsafe { Layout::from_size_align_unchecked(2 << Sv39::PAGE_BITS, 1 << Sv39::PAGE_BITS) };
    let pages = 2;
    let stack = unsafe { alloc(PAGE) };
    ks.map_extern(
        VPN::new((1 << 26) - pages)..VPN::new(1 << 26),
        PPN::new(stack as usize >> Sv39::PAGE_BITS),
        build_flags("_WRV"),
    );
    // 建立调度线程，目的是划分异常域。调度线程上发生内核异常时会回到这个控制流处理
    let mut scheduling = LocalContext::thread(schedule as *const () as _, false);
    *scheduling.sp_mut() = 1 << 38;
    unsafe { scheduling.execute() };
    log::error!("stval = {:#x}", stval::read());
    panic!("trap from scheduling thread: {:?}", scause::read().cause());
}

extern "C" fn schedule() -> ! {
    // 初始化异界传送门
    let portal = unsafe { MultislotPortal::init_transit(PROTAL_TRANSIT.base().val(), 1) };
    // 初始化 syscall
    tg_syscall::init_io(&SyscallContext);
    tg_syscall::init_process(&SyscallContext);
    tg_syscall::init_scheduling(&SyscallContext);
    tg_syscall::init_clock(&SyscallContext);
    tg_syscall::init_trace(&SyscallContext);
    tg_syscall::init_memory(&SyscallContext);
    while !unsafe { PROCESSES.get_mut().is_empty() } {
        let ctx = unsafe { &mut PROCESSES.get_mut()[0].context };
        unsafe { ctx.execute(portal, ()) };
        match scause::read().cause() {
            scause::Trap::Exception(scause::Exception::UserEnvCall) => {
                use tg_syscall::{SyscallId as Id, SyscallResult as Ret};

                let ctx = &mut ctx.context;
                let id_raw = ctx.a(7);
                let id: Id = id_raw.into();
                let args = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];

                // 统计 syscall 调用次数
                let process = unsafe { &mut PROCESSES.get_mut()[0] };
                if id_raw < process.syscall_count.len() {
                    process.syscall_count[id_raw] += 1;
                }

                // trace(2, target_id, _) 时通过 caller.flow 传递查询结果
                let trace_query_count = if id == Id::TRACE && args[0] == 2 {
                    process.syscall_count.get(args[1]).copied().unwrap_or(0)
                } else {
                    0
                };

                match tg_syscall::handle(Caller { entity: 0, flow: trace_query_count }, id, args) {
                    Ret::Done(ret) => match id {
                        Id::EXIT => unsafe {
                            PROCESSES.get_mut().remove(0);
                        },
                        _ => {
                            *ctx.a_mut(0) = ret as _;
                            ctx.move_next();
                        }
                    },
                    Ret::Unsupported(_) => {
                        log::info!("id = {id:?}");
                        unsafe { PROCESSES.get_mut().remove(0) };
                    }
                }
            }
            e => {
                log::error!(
                    "unsupported trap: {e:?}, stval = {:#x}, sepc = {:#x}",
                    stval::read(),
                    ctx.context.pc()
                );
                unsafe { PROCESSES.get_mut().remove(0) };
            }
        }
    }
    tg_sbi::shutdown(false)
}

/// Rust 异常处理函数，以异常方式关机。
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{info}");
    tg_sbi::shutdown(true)
}

fn kernel_space(
    layout: tg_linker::KernelLayout,
    memory: usize,
    portal: usize,
) -> AddressSpace<Sv39, Sv39Manager> {
    let mut space = AddressSpace::<Sv39, Sv39Manager>::new();
    for region in layout.iter() {
        log::info!("{region}");
        use tg_linker::KernelRegionTitle::*;
        let flags = match region.title {
            Text => "X_RV",
            Rodata => "__RV",
            Data | Boot => "_WRV",
        };
        let s = VAddr::<Sv39>::new(region.range.start);
        let e = VAddr::<Sv39>::new(region.range.end);
        space.map_extern(
            s.floor()..e.ceil(),
            PPN::new(s.floor().val()),
            build_flags(flags),
        )
    }
    log::info!(
        "(heap) ---> {:#10x}..{:#10x}",
        layout.end(),
        layout.start() + memory
    );
    let s = VAddr::<Sv39>::new(layout.end());
    let e = VAddr::<Sv39>::new(layout.start() + memory);
    space.map_extern(
        s.floor()..e.ceil(),
        PPN::new(s.floor().val()),
        build_flags("_WRV"),
    );
    space.map_extern(
        PROTAL_TRANSIT..PROTAL_TRANSIT + 1,
        PPN::new(portal >> Sv39::PAGE_BITS),
        build_flags("__G_XWRV"),
    );
    println!();
    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().val()) };
    space
}

/// 各种接口库的实现。
mod impls {
    use crate::{build_flags, Sv39, PROCESSES};
    use alloc::alloc::alloc_zeroed;
    use core::{alloc::Layout, ptr::NonNull};
    use tg_console::log;
    use tg_kernel_vm::{
        page_table::{MmuMeta, Pte, VAddr, VmFlags, PPN, VPN},
        PageManager,
    };
    use tg_syscall::*;

    #[repr(transparent)]
    pub struct Sv39Manager(NonNull<Pte<Sv39>>);

    impl Sv39Manager {
        const OWNED: VmFlags<Sv39> = unsafe { VmFlags::from_raw(1 << 8) };

        #[inline]
        fn page_alloc<T>(count: usize) -> *mut T {
            unsafe {
                alloc_zeroed(Layout::from_size_align_unchecked(
                    count << Sv39::PAGE_BITS,
                    1 << Sv39::PAGE_BITS,
                ))
            }
            .cast()
        }
    }

    impl PageManager<Sv39> for Sv39Manager {
        #[inline]
        fn new_root() -> Self {
            Self(NonNull::new(Self::page_alloc(1)).unwrap())
        }

        #[inline]
        fn root_ppn(&self) -> PPN<Sv39> {
            PPN::new(self.0.as_ptr() as usize >> Sv39::PAGE_BITS)
        }

        #[inline]
        fn root_ptr(&self) -> NonNull<Pte<Sv39>> {
            self.0
        }

        #[inline]
        fn p_to_v<T>(&self, ppn: PPN<Sv39>) -> NonNull<T> {
            unsafe { NonNull::new_unchecked(VPN::<Sv39>::new(ppn.val()).base().as_mut_ptr()) }
        }

        #[inline]
        fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Sv39> {
            PPN::new(VAddr::<Sv39>::new(ptr.as_ptr() as _).floor().val())
        }

        #[inline]
        fn check_owned(&self, pte: Pte<Sv39>) -> bool {
            pte.flags().contains(Self::OWNED)
        }

        #[inline]
        fn allocate(&mut self, len: usize, flags: &mut VmFlags<Sv39>) -> NonNull<u8> {
            *flags |= Self::OWNED;
            NonNull::new(Self::page_alloc(len)).unwrap()
        }

        fn deallocate(&mut self, _pte: Pte<Sv39>, _len: usize) -> usize {
            todo!()
        }

        fn drop_root(&mut self) {
            todo!()
        }
    }

    pub struct Console;

    impl tg_console::Console for Console {
        #[inline]
        fn put_char(&self, c: u8) {
            tg_sbi::console_putchar(c);
        }
    }

    pub struct SyscallContext;

    impl IO for SyscallContext {
        fn write(&self, caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            match fd {
                STDOUT | STDDEBUG => {
                    const READABLE: VmFlags<Sv39> = build_flags("RV");
                    if let Some(ptr) = unsafe { PROCESSES.get_mut() }
                        .get_mut(caller.entity)
                        .unwrap()
                        .address_space
                        .translate::<u8>(VAddr::new(buf), READABLE)
                    {
                        print!("{}", unsafe {
                            core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                                ptr.as_ptr(),
                                count,
                            ))
                        });
                        count as _
                    } else {
                        log::error!("ptr not readable");
                        -1
                    }
                }
                _ => {
                    log::error!("unsupported fd: {fd}");
                    -1
                }
            }
        }
    }

    impl Process for SyscallContext {
        #[inline]
        fn exit(&self, _caller: Caller, _status: usize) -> isize {
            0
        }

        fn sbrk(&self, caller: Caller, size: i32) -> isize {
            if let Some(process) = unsafe { PROCESSES.get_mut() }.get_mut(caller.entity) {
                if let Some(old_brk) = process.change_program_brk(size as isize) {
                    old_brk as isize
                } else {
                    -1
                }
            } else {
                -1
            }
        }
    }

    impl Scheduling for SyscallContext {
        #[inline]
        fn sched_yield(&self, _caller: Caller) -> isize {
            0
        }
    }

    impl Clock for SyscallContext {
        #[inline]
        fn clock_gettime(&self, caller: Caller, clock_id: ClockId, tp: usize) -> isize {
            const WRITABLE: VmFlags<Sv39> = build_flags("W_V");
            match clock_id {
                ClockId::CLOCK_MONOTONIC => {
                    if let Some(mut ptr) = unsafe { PROCESSES.get_mut() }
                        .get_mut(caller.entity)
                        .unwrap()
                        .address_space
                        .translate::<TimeSpec>(VAddr::new(tp), WRITABLE)
                    {
                        let time = riscv::register::time::read() * 10000 / 125;
                        *unsafe { ptr.as_mut() } = TimeSpec {
                            tv_sec: time / 1_000_000_000,
                            tv_nsec: time % 1_000_000_000,
                        };
                        0
                    } else {
                        log::error!("ptr not readable");
                        -1
                    }
                }
                _ => -1,
            }
        }
    }

    impl Trace for SyscallContext {
        fn trace(&self, caller: Caller, trace_request: usize, id: usize, data: usize) -> isize {
            const READABLE: VmFlags<Sv39> = build_flags("U__RV");
            const WRITABLE: VmFlags<Sv39> = build_flags("U_W_V");
            let process = unsafe { PROCESSES.get_mut() }
                .get_mut(caller.entity)
                .unwrap();
            match trace_request {
                // request=0: 读取用户地址 id 处的 1 字节
                0 => {
                    if let Some(ptr) = process.address_space.translate::<u8>(VAddr::new(id), READABLE) {
                        (unsafe { *ptr.as_ptr() }) as isize
                    } else {
                        -1
                    }
                }
                // request=1: 向用户地址 id 处写入 data 的低 8 位
                1 => {
                    if let Some(mut ptr) = process.address_space.translate::<u8>(VAddr::new(id), WRITABLE) {
                        unsafe { *ptr.as_mut() = data as u8 };
                        0
                    } else {
                        -1
                    }
                }
                // request=2: 返回目标 syscall 的调用次数（由调度器通过 caller.flow 传入）
                2 => caller.flow as isize,
                _ => -1,
            }
        }
    }

    impl Memory for SyscallContext {
        fn mmap(
            &self,
            caller: Caller,
            addr: usize,
            len: usize,
            prot: i32,
            _flags: i32,
            _fd: i32,
            _offset: usize,
        ) -> isize {
            const PAGE_SIZE: usize = 1 << <Sv39 as MmuMeta>::PAGE_BITS;

            // addr 必须页对齐
            if addr % PAGE_SIZE != 0 {
                return -1;
            }
            // prot 低 3 位不能全为 0
            if prot & 0x7 == 0 {
                return -1;
            }
            // prot 高位必须为 0
            if prot & !0x7 != 0 {
                return -1;
            }

            // len 向上取整到页
            let len_aligned = (len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            if len_aligned == 0 {
                return 0;
            }

            let start_vpn = VPN::<Sv39>::new(addr >> <Sv39 as MmuMeta>::PAGE_BITS);
            let end_vpn = VPN::<Sv39>::new((addr + len_aligned) >> <Sv39 as MmuMeta>::PAGE_BITS);

            let process = unsafe { PROCESSES.get_mut() }
                .get_mut(caller.entity)
                .unwrap();

            // 检查是否与已有映射重叠
            for area in &process.address_space.areas {
                if start_vpn < area.end && end_vpn > area.start {
                    return -1;
                }
            }

            // 构建 flags: U + prot 对应的 R/W/X + V
            let mut flags_str: [u8; 5] = *b"U___V";
            if prot & 0x4 != 0 {
                flags_str[1] = b'X';
            }
            if prot & 0x2 != 0 {
                flags_str[2] = b'W';
            }
            if prot & 0x1 != 0 {
                flags_str[3] = b'R';
            }
            let flags = crate::parse_flags(
                unsafe { core::str::from_utf8_unchecked(&flags_str) }
            ).unwrap();

            process.address_space.map(start_vpn..end_vpn, &[], 0, flags);
            0
        }

        fn munmap(&self, caller: Caller, addr: usize, len: usize) -> isize {
            const PAGE_SIZE: usize = 1 << <Sv39 as MmuMeta>::PAGE_BITS;

            // addr 必须页对齐
            if addr % PAGE_SIZE != 0 {
                return -1;
            }

            let len_aligned = (len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            if len_aligned == 0 {
                return 0;
            }

            let start_vpn = VPN::<Sv39>::new(addr >> <Sv39 as MmuMeta>::PAGE_BITS);
            let end_vpn = VPN::<Sv39>::new((addr + len_aligned) >> <Sv39 as MmuMeta>::PAGE_BITS);

            let process = unsafe { PROCESSES.get_mut() }
                .get_mut(caller.entity)
                .unwrap();

            // 检查 [start_vpn, end_vpn) 中每一页都已被映射
            let mut vpn = start_vpn;
            while vpn < end_vpn {
                let covered = process.address_space.areas.iter().any(|area| {
                    vpn >= area.start && vpn < area.end
                });
                if !covered {
                    return -1;
                }
                vpn = vpn + 1;
            }

            process.address_space.unmap(start_vpn..end_vpn);
            0
        }
    }
}

/// 非 RISC-V64 架构的占位实现
#[cfg(not(target_arch = "riscv64"))]
mod stub {
    use tg_kernel_vm::page_table::{MmuMeta, VmFlags};

    /// Sv39 占位类型
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
    pub struct Sv39;

    impl MmuMeta for Sv39 {
        const P_ADDR_BITS: usize = 56;
        const PAGE_BITS: usize = 12;
        const LEVEL_BITS: &'static [usize] = &[9, 9, 9];
        const PPN_POS: usize = 10;

        #[inline]
        fn is_leaf(value: usize) -> bool {
            value & 0b1110 != 0
        }
    }

    /// 构建 VmFlags 占位。
    pub const fn build_flags(_s: &str) -> VmFlags<Sv39> {
        unsafe { VmFlags::from_raw(0) }
    }

    /// 解析 VmFlags 占位。
    pub fn parse_flags(_s: &str) -> Result<VmFlags<Sv39>, ()> {
        Ok(unsafe { VmFlags::from_raw(0) })
    }

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
