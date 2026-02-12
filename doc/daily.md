## day1
### 事件1: rCore ch6
阅读了两天的文档和rCore相关的代码. 对于从磁盘上的 SuperBlock, Inode_bitmap, Inode_area, Data_bitmap, Data_area, 到操作系统提供的访问磁盘上数据结构的 easyfs, 再到内核中使用 easyfs的 Inode 有了一定了解. 
在Inode中实现了 link api, 作为后续实现 linkat 的基础

### 事件2: rCore-Tutorial-in-single-workspace
阅读了https://github.com/rcore-os/rCore-Tutorial-in-single-workspace/tree/test/ 的结构, 了解了怎么把公共模块拆成一个个 tg-xx 的 crate 并发布到 crate.io 中. 
比如 ch4 需要 tg-kernel-vm, ch4/Cargo.toml 里写好 tg-kernel-vm = { version = "..."}, cargo run 时直接拉取依赖. 如果我想 hack, 可以 cargo clone tg-kernel-vm, 然后在 ch4/Cargo.toml 中把依赖改成'tg-kernel-vm = { path = "./tg-kernel-vm" }'.
目前在自己的 chenty2333/ai4ose-lab1-2026s 仓库中新建了 os 文件夹, 然后把 ch1-3, rust-toolchain.toml 和 tg-user 移动进 os文件夹. 把TG_USER_DIR设为本地路径.
在 ch3 运行了一下 cargo run --features exercise, 发现 [ERROR] Panicked at src/bin/ch3_trace.rs:26, assertion failed: 3 <= count_syscall(SYS_CLOCK_GETTIME)
然后发现在 main.rs 中需要 impl Trace for SyscallContext. 了解了 syscall 不再是一个具体的 sys_xxx 函数, 而是 trait 形式出现.

### 问题
对于 single-workspace 版的 rCore还有一些机制上的疑问, 比如我能对一个 tg-xx hack 到什么程度.
### 计划
先完成 rCore ch6 的 linkat 等. 然后尝试 single-workspace 版的 ch6



## day 2

### 事件1: rCore水水的一天

首先是, 又推进了一点点 rCore, ch6 给的 Inode 又实现了一个 unlink api, 还是和 link 的逻辑有点不一样的. unlink 也需要 fs.lock() 保护, 虽然在 link 中 self.increase_size() 中需要锁, unlink 直接 self.size -= 1 不需要, 但还是 “一把大锁保平安”, 避免潜在的一致性问题.

还有就是要遍历一遍, 判断没有其他 DirEntry 指向相同的 Inode, 就可以对 Inode 进行释放. 这部分代码 AI 帮我写的, 虽然也挺简单, 就纯懒…

### 事件二: 小更新了doc

增加了一个 aichat_history 目录, 把 ch6 学习过程中, 问 opus4.6 的过程记录进去.

### 问题

今天也没遇到啥问题. 其实也没干多少事, 进度有点点慢. 不过16号前, ch8 大概率还是能完成, 先结束 rCore 再说. 

### 计划

如上, 16号前做完 rCore. 同时也要花一点事件继续完善自己的操作系统, 目标是在树莓派上启动, 现在问题还蛮多的, USB xHCI 啥的还没做.



## day 3

### 事件1: rCore ch6 完成

补上了 sys_linkat, sys_unlinkat, sys_stat 的实现, 发现之前在 Inode 上做了个 count 统计硬链接数量是错的, 应该放 DiskInode 里面做持久化的统计, 虽然听实验的视频课讲的好像就是用 Inode 统计? 然后就是 link 和 unlink 加一个往 DiskInode 里更新 nlink 的逻辑.

另外就是问答作业还没完成. 不过先提交到仓库里了, ch6-8 之后一起提交吧.

## 计划

想了想我能在 AI4OSE 的实验中做点什么?

我 100% 是做不到像助教们那样, 开发新的实验平台和工具. 即使能借助 AI 开发, 但本质上没创新, 上我 + AI 的成果任何人同样能通过 AI 实现, 无非时间长短问题.

不过我可以记录下我是怎么借助 AI 学习的, 以及留下些更低上手能力要求的文档, 做我喜欢的事. 

目前计划依然是 16 号完成 ch8.

后续会进入到 single-workspace 的实验. 边做边写文档. 实验的各种 test 感觉也没啥好 hack 或者创新的, 我比助教差远了, 助教的设计就是最优的 (至少我目前的水平, 觉得是最优的). tg-* 这些模块真的很有意思! 还好现在 AI 足够强, 我应该也能按自己的想法做些有意思的小破坏 (不影响 AI4OSE 实验, 纯自己想玩) .