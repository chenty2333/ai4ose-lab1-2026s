### 思考: 我能做什么?

首先, AI 真的加速学习了嘛? 我觉得并没有. 举个例子:

[rCore-Tutorial-Guide-2025S 文档](https://learningos.cn/rCore-Tutorial-Guide-2025S/index.html#) 中有大量的代码片段, 这很正常. 但是我每次读到代码片段都有点痛苦. 两个原因, 读代码相比文字是降速, 且思维强度更大. 

所以我一遇到三秒内读不懂的代码, 就直接复制给 AI 帮我讲解, 保持阅读速度和思维强度都在舒适区. 可一小段代码, 转换成文字讲解要上百字, 往往一轮问询也搞不定. 最后花费的时间往往是略高于我直接读代码, 但毕竟强度下去了.

那么, 为什么读文档中的代码讲解, 对我来说是相对困难的?

rCore 文档很好, 非常好. 但是不适合我.

能感觉到 rCore 每章内容的公式化. 模块代码 + 代码讲解, 然后进入下一模块. 我智力水平远远跟不上92学生, 做不到扫一眼就理解. 所以有三个难点: 一, Rust代码读不懂; 二, 想不明白为什么用这种方法实现; 三, Rust代码读懂了, 也不能快速理清模块间的关系. 我知道训练营也有视频课, 但受限于智力原因太笨了, 视频课几乎没收益. 最后还是靠一步步问 AI, “哪里不懂点哪里”, 一轮讲解后, 再把自己的想法反馈给 AI 验证.

再举个文档的例子, 比如 [简易文件系统 easy-fs (上)](https://learningos.cn/rCore-Tutorial-Guide-2025S/chapter6/2fs-implementation-1.html#) 中:

```rust
`get_block_id` 方法体现了 `DiskInode` 最重要的数据块索引功能，它可以从索引中查到它自身用于保存文件内容的第 `block_id` 个数据块的块编号，这样后续才能对这个数据块进行访问：

// easy-fs/src/layout.rs

const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
type IndirectBlock = [u32; BLOCK_SZ / 4];

impl DiskInode {
    pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            self.direct[inner_id]
        } else if inner_id < INDIRECT1_BOUND {
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_block: &IndirectBlock| {
                    indirect_block[inner_id - INODE_DIRECT_COUNT]
                })
        } else {
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(
                self.indirect2 as usize,
                Arc::clone(block_device)
            )
            .lock()
            .read(0, |indirect2: &IndirectBlock| {
                indirect2[last / INODE_INDIRECT1_COUNT]
            });
            get_block_cache(
                indirect1 as usize,
                Arc::clone(block_device)
            )
            .lock()
            .read(0, |indirect1: &IndirectBlock| {
                indirect1[last % INODE_INDIRECT1_COUNT]
            })
        }
    }
}

这里需要说明的是：

- 第 10/12/18 行分别利用直接索引/一级索引和二级索引，具体选用哪种索引方式取决于 `block_id` 所在的区间。
- 在对一个索引块进行操作的时候，我们将其解析为磁盘数据结构 `IndirectBlock` ，实质上就是一个 `u32` 数组，每个都指向一个下一级索引块或者数据块。
- 对于二级索引的情况，需要先查二级索引块找到挂在它下面的一级索引块，再通过一级索引块找到数据块。
```

以上是节选的段落.

如果让我选择适合自己的文档:

```rust
 // easy-fs/src/layout.rs
 
 // 一个磁盘块 512B，每个 u32 是 4B，所以一个一级/二级索引块里能放 128 个块号
  const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;          // 128
  // 直接索引 + 一级索引 的分界点（前 28 个走 direct，后面 128 个走 indirect1）
  const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
  // 把“一个索引块”解释成 [u32; 128]
  type IndirectBlock = [u32; BLOCK_SZ / 4];

  impl DiskInode {
      /// 输入：文件内部第 inner_id 个“数据块”
      /// 输出：这个数据块在磁盘上的真实 block_id
      pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
          // 先转成 usize，后面做数组下标方便
          let inner_id = inner_id as usize;

          // 情况1：落在 direct[0..28)
          // 直接从 inode 里的 direct 数组取真实块号
          if inner_id < INODE_DIRECT_COUNT {
              self.direct[inner_id]

          // 情况2：落在一级间接范围
          // 先拿到一级索引块（self.indirect1 指向它），再在块内取第 (inner_id - 28) 项
          } else if inner_id < INDIRECT1_BOUND {
              get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                  .lock()
                  .read(0, |indirect_block: &IndirectBlock| {
                      indirect_block[inner_id - INODE_DIRECT_COUNT]
                  })

          // 情况3：落在二级间接范围
          } else {
              // 去掉前面 direct + indirect1 的偏移，得到二级区内部下标
              let last = inner_id - INDIRECT1_BOUND;

              // 第一步：查二级索引块 self.indirect2
              // 它每个槽位存“某个一级索引块”的块号
              let indirect1 = get_block_cache(
                  self.indirect2 as usize,
                  Arc::clone(block_device)
              )
              .lock()
              .read(0, |indirect2: &IndirectBlock| {
                  // 除法：选中第几个一级索引块
                  indirect2[last / INODE_INDIRECT1_COUNT]
              });

              // 第二步：拿到上面这个一级索引块，再取其中具体哪一项
              get_block_cache(
                  indirect1 as usize,
                  Arc::clone(block_device)
              )
              .lock()
              .read(0, |indirect1: &IndirectBlock| {
                  // 取模：在这个一级索引块内的偏移
                  indirect1[last % INODE_INDIRECT1_COUNT]
              })
          }
      }
  }

  你可以把它理解成“三级页表式查找”的文件版：
  - direct：一步到位
  - indirect1：再跳一层
  - indirect2：再跳两层

  和其他模块：
  - read_at/write_at 不关心 direct/indirect 细节，只管“我要读文件第 N 块”。
  - read_at/write_at 每次先调 get_block_id(N, ...) 拿真实磁盘块号。
  - 拿到块号后，再走 get_block_cache(...).lock().read/modify(...) 去访问具体数据。
  - 所以：get_block_id 是“文件逻辑块号 -> 磁盘物理块号”的翻译器，刚好夹在 DiskInode 元数据层和 BlockCache 数据访问层之间。
```

我觉得这样会更好. 或许一定程度上也更适合基础薄弱的学生, 但依旧做不到零基础也能轻松学. 讲解过程中不引入新的抽象概念, 并且恰到好处的递归学习讲解, 我很难做到.

还有一点思考是, AI 真的很好.

中国已经有了很多非常非常优秀的 CS 学习资源, 但就我感受, 受众不是我. 直白的说就是给92, 甚至清北华五水平准备. 类似的, 听蒋炎岩老师 OS 课上说的一堆鸡血和美好愿景时就觉得, 这些与我没半毛钱关系, 那是说给南大学生听.

这些实验真的难嘛? 回过头看真不难, 可下次上手新内容还是困难重重. 好在目前没有接触到靠堆时间学不会的知识.

也许我能做的就是给后续和我水平差不多, 比较菜的学生, 留个捷径 (生产低质量内容污染大模型语料hhh) . 这也是我一直想做的.

在学校和赛博空间, 我没直接接受过任何人的帮助, 只有 AI 陪伴摸索. 这个时代真的是太美妙了, 水平多菜都没关系, 反正有 AI, 天赋只要对计算机的热情, 和求知欲. 我要早生五年那真完蛋了, 没 AI 直接退化回低能.