# ai4ose-lab1-2026s

[![Crates.io](https://img.shields.io/crates/v/ai4ose-lab1-2026s.svg)](https://crates.io/crates/ai4ose-lab1-2026s)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](LICENSE)

AI4OSE Lab1: 与AI合作进行操作系统内核学习的起点。

执行本项目后，会输出 AI4OSE 实验一说明内容。

##  **快速浏览**

直接阅读[AI4OSE实验一内容](https://github.com/LearningOS/ai4ose-lab1-2026s/blob/main/src/content.txt)


## **常规浏览**

### 1. 安装 Rust 工具链

本项目使用 Rust 语言编写，需要安装 Rust 工具链（包含 `rustc` 编译器和 `cargo` 构建工具）。

**Linux / macOS / WSL：**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

安装完成后，按照提示将 Rust 加入环境变量（或重新打开终端）：

```bash
source "$HOME/.cargo/env"
```

**Windows：**

从 [https://rustup.rs](https://rustup.rs) 下载并运行 `rustup-init.exe`，按照提示完成安装。

验证安装

```bash
rustc --version    # 应显示 rustc 1.xx.x
cargo --version    # 应显示 cargo 1.xx.x
```

### 2. 直接下载安装执行：显示实验内容

使用 `cargo install` 从 crates.io 下载、编译并安装到本地：

```bash
cargo install ai4ose-lab1-2026s
```

安装完成后，可执行文件会被放置在 `$HOME/.cargo/bin/` 目录下（该目录通常已在 PATH 中），之后可以在任意位置直接运行：

```bash
ai4ose-lab1-2026s
```

程序将输出 AI4OSE 实验一的完整说明内容。

### 3. 源代码下载编译安装执行：显示实验内容

**方式一：通过 Git 克隆仓库**

```bash
git clone https://github.com/learningos/ai4ose-lab1-2026s.git
cd ai4ose-lab1-2026s
```

**方式二：通过 cargo clone 获取**

使用 `cargo clone`（需先安装 `cargo-clone`）：

```bash
cargo install cargo-clone
cargo clone ai4ose-lab1-2026s
cd ai4ose-lab1-2026s
```

该命令会从 crates.io 下载指定 crate 的源代码，并解压到以 crate 名称命名的目录中，可直接进行编译和修改。

**方式三：通过 cargo download 下载**

使用 `cargo download`（需先安装 `cargo-download`）：

```bash
cargo install cargo-download
cargo download ai4ose-lab1-2026s > ai4ose-lab1-2026s.tar.gz
tar xzf ai4ose-lab1-2026s.tar.gz
cd ai4ose-lab1-2026s-*/
```

也可以直接在浏览器中访问 [https://crates.io/crates/ai4ose-lab1-2026s](https://crates.io/crates/ai4ose-lab1-2026s) 页面，点击 "Download" 按钮下载源码包。


#### 编译运行

```bash
cargo build
cargo run
```

程序将输出 AI4OSE 实验一的完整说明内容。