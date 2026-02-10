## day1
### 事件1: rCore ch6
阅读了两天的文档和rCore相关的代码. 对于从磁盘上的SuperBlock, Inode_bitmap, Inode_area, Data_bitmap, Data_area, 到操作系统提供的访问磁盘上数据结构的easyfs, 再到内核中使用easyfs的Inode有了一定了解. 
在Inode中实现了link api, 作为后续实现linkat的基础
### 事件2: rCore-Tutorial-in-single-workspace
阅读了https://github.com/rcore-os/rCore-Tutorial-in-single-workspace/tree/test/的结构, 了解了怎么把公共模块拆成一个个tg-xx的crate并发布到crate.io中. 
比如 ch4 需要 tg-kernel-vm, ch4/Cargo.toml 里写好 tg-kernel-vm = { version = "..."}, cargo run 时直接拉取依赖. 如果我想 hack, 可以 cargo clone tg-kernel-vm, 然后改在 ch4/Cargo.toml 中把依赖改成'tg-kernel-vm = { path = "./tg-kernel-vm" }'.
目前在自己的 chenty2333/ai4ose-lab1-2026s 仓库中新建了 os 文件夹, 然后把 ch1-3, rust-toolchain.toml 和 tg-user 移动进 os文件夹. 把TG_USER_DIR设为本地路径.
在 ch3 运行了一下 cargo run --features exercise, 发现 [ERROR] Panicked at src/bin/ch3_trace.rs:26, assertion failed: 3 <= count_syscall(SYS_CLOCK_GETTIME)
然后发现在 main.rs 中需要 impl Trace for SyscallContext. 了解了 syscall 不再是一个具体的 sys_xxx 函数, 而是 trait 形式出现.
### 问题
对于 single-workspace 版的 rCore还有一些机制上的疑问, 比如我能对一个 tg-xx hack 到什么程度.
### 计划
先完成 rCore ch6 的 linkat 等. 然后尝试 single-workspace 版的 ch6