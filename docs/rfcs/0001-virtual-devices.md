# RFC 0001：重定向范围内的虚拟设备

状态：提案，尚未实现。目标版本：v0.2。日期：2026-10-11。

## 为什么值得做

`/dev/null` 用于丢弃输出或提供空输入，`/dev/zero` 用于生成零字节测试数据，`/dev/urandom` 用于生成随机测试输入。这些用途适合集中在执行器的 I/O 层，不需要扩大命令语言。

当前版本已经在**重定向目标**中把 `/dev/null` 映射为 Windows NUL。下一步是补齐契约和测试，并设计可取消的流提供器。

## 决策一：只支持执行器拥有的文件打开操作

第一阶段支持：

```text
tool.exe > /dev/null 2>&1
head -c 1024 < /dev/zero
head -c 32 < /dev/urandom
dd bs=1024 count=8 < /dev/zero > sample.bin
```

`head` / `dd` 仍需用户提供相应工具。设备只是 stdin/stdout 的来源或去向。下面这些**不在第一阶段范围内**：

```text
head -c 1024 /dev/zero
dd if=/dev/zero of=sample.bin
cat /dev/urandom
```

这些路径存在于普通 argv 中，由外部程序自己打开。执行器不应猜测任意程序的参数约定，或把所有含 `/dev/` 的字符串改写成其他路径；否则会误改配置、日志文本和表达式。

不增加文件系统驱动、不注入目标程序、不钩住 Win32 文件 API、不模拟完整 `/dev`。少数工具的显式适配可以另行评估，但需要定义打开方式、seek/mmap 等契约，不能承诺透明兼容。

## 决策二：设备契约

| 设备 | 读取 | 写入/追加 | 实现和边界 |
|---|---|---|---|
| `/dev/null` | 立即 EOF | 丢弃 | 复用 Windows NUL；当前已有核心映射 |
| `/dev/zero` | 持续产生零字节 | 丢弃 | 读端用有界缓冲生产者，写端用空接收器 |
| `/dev/urandom` | Windows 系统随机源提供的字节 | 第一阶段拒绝 | 使用系统 CSPRNG，不模拟“加熵” |
| `/dev/random` | 暂缓；若开放，显式声明为同一 Windows 随机源的别名 | 第一阶段拒绝 | 不模拟 Linux 熵池和阻塞策略，不声称比 urandom 更安全 |

优先级：null 完整验收 → zero → urandom。random 的名字容易让人期待 Linux 特有语义，最后再决定是否需要。

Windows 随机源拟使用 `BCryptGenRandom(NULL, buffer, length, BCRYPT_USE_SYSTEM_PREFERRED_RNG)`，每次检查返回状态。失败不得回退到时间戳、固定种子或自写 PRNG。生产随机源和可复现的测试数据生成器必须分开。

Linux 设备定义是参考背景，不代表本提案承诺完全兼容。尤其不采用“random 总是阻塞、urandom 总是不阻塞”这种简化描述作为 Windows 实现契约。

## 决策三：有边界的命名空间

- 只在重定向展开后的单个路径值上，精确、区分大小写地匹配设备名。
- `< '/dev/zero'` 仍指向同一设备；变量展开也遵循相同规则。
- 在 Windows 路径规范化前匹配；不把 `./dev/zero`、`C:/dev/zero` 或反斜杠变体猜成设备。
- 未支持的 `/dev/...` 明确报错，避免意外创建普通文件。
- 不承诺 seek、mmap、文件长度、文件属性或 ioctl。
- 新行为纳入 dialect/能力版本，不静默改变已有真实文件路径含义。

## 决策四：I/O 抽象只解决 I/O

把命令执行中“识别路径并直接 OpenOptions”的职责收敛为 `open_redirect(target, access, context)`，返回 `PreparedEndpoint`：

1. 可传给子进程的 Windows 句柄。
2. 可选生产者/接收者任务及生命周期所有者。
3. 错误、取消和完成状态。

端点可来自普通文件、NUL 或虚拟流。解析器不需要知道字节如何生成；工具映射也不携带设备实现。执行器负责按原有重定向顺序选择端点和保留句柄别名。

`cmd < /dev/zero < input.txt` 不能留下被覆盖的 zero 生产者。`2>&1 > /dev/null` 中 stderr 仍指向原 stdout，不能因路径特殊改变顺序。优先“准备最终端点后启动必要任务”，或确保被覆盖端点能取消并回收任务。不注册全局设备、不引入常驻服务。

## 最难的是停止生产

zero 和 random 是无限源，不能复用 `<()` 的全部捕获路径，否则永远等不到完成。也不能简单开线程无限 WriteFile，再无条件 join。

Windows 匿名管道满时写入会阻塞；阻塞期间单纯检查 atomic bool 不够，必须能解除阻塞。需要先验证一种方案：

- 匿名管道配合同步 I/O 取消机制，以及明确的线程/句柄归属。
- 或内部使用支持 overlapped 的管道和取消事件，代价是更复杂的实现。

内部管道只向程序传递标准句柄，不因此承诺普通 argv 路径也能访问设备。

所有结束路径都要覆盖：消费者读足退出、不读直接退出、程序启动失败、重定向覆盖、Ctrl+C、随机源失败、整条管道取消。消费者正常关闭导致的 broken pipe 可作为收尾；其他错误不能静默忽略。

句柄不能泄漏到无关子进程，否则 EOF/破管道条件可能一直不成立。设备任务与 Job Object/后代进程回收共用生命周期，不建立第二套清理系统。

当前外部命令的执行路径在等待子进程时仍持有 `Streams.input`；如果它是虚拟设备管道的读端，即使子进程已退出，生产者也可能暂时看不到“全部读端已关闭”。实现前要调整所有权：把读端交给子进程后，执行器只保留必要的控制句柄，并确保在等待生产者收尾前关闭自己的读端副本。多段管道、重定向覆盖和错误退出都应做同样的句柄审计。

## 资源和性能

- 例如每次生成 64 KiB，管道背压限制速度；不按总输出长度分配内存。
- 共享执行树的任务/句柄预算；限制并发数量，不悄悄截短设备内容。
- 无限流由消费者或用户取消终止，不套用捕获数据的 8 MiB 总量限制。
- `$()` 中消费无限设备仍受捕获预算限制；超限必须停止对应执行子树，不能只停止读取。
- 分别测吞吐、CPU、工作集、首次字节延迟和取消完成时间。
- 随机源按块请求系统服务，不逐字节调用；块大小由测量决定。

## 验收用例

1. null：读取 EOF、丢弃 stdout/stderr、append 和重定向顺序正确。
2. zero：读取 0/1/4096/65537 字节均为零，大流内存有界。
3. 消费者提前退出或不读即退出，生产者及时收尾。
4. 启动失败、重定向覆盖、Ctrl+C 后没有任务/句柄残留。
5. urandom：长度正确、系统错误可注入并传播；不能用统计外观测试证明密码学安全。
6. 多个消费者不共享可变缓冲；句柄别名符合重定向语义。
7. `echo '/dev/zero'` 和普通 argv 保持字面量；未知设备报错。
8. `$()` 中无限数据超出捕获预算时，消费者与生产者都能取消。

## 难度与版本建议

- null：低，主要补契约和回归测试。
- zero：生成数据简单，可靠取消和生命周期中等难度。
- urandom：系统 RNG 接入不复杂，错误传播和取消中等难度。
- Linux random 语义复刻：收益有限，维护成本高，不建议。
- 任意程序路径透明兼容：另一类系统工程，超出轻量执行器边界。

建议 v0.2 先交付可取消 I/O 和 null/zero，再加入 urandom。父 CMD 状态同步仍是独立功能，不与设备协议混在一起。无需纳入 source 或完整 Bash。

## 参考资料

- Linux man-pages: [null(4)](https://man7.org/linux/man-pages/man4/null.4.html)、[random(4)](https://man7.org/linux/man-pages/man4/random.4.html)。
- Microsoft: [BCryptGenRandom](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom)。
- Microsoft: [Anonymous Pipe Operations](https://learn.microsoft.com/en-us/windows/win32/ipc/anonymous-pipe-operations)、[CancelSynchronousIo](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelsynchronousio)。

平台资料支持设备和管道行为；版本范围、模块边界与验收标准是本项目设计选择。
