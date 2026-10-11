# Clink POSIX Native

A bounded POSIX-style interactive command executor for native Windows programs.

Windows x64 Rust 原型，为 Clink 提供明确、有边界的交互语法。Lua 负责显式接入，Rust 负责解析、展开和进程执行。无需 Python 运行时、Cargo 第三方依赖或常驻服务。

**状态：实验性 v0.1。** 不是 Bash 或完整 `.sh` 兼容层，不建议直接替换现有全局 Clink 配置。当前 `cd` / `export` 只在本次提交的整行中生效。

## 构建与测试

需要 Windows、Rust 1.98+ 和 PowerShell；测试额外需要 Python 3.10+。在仓库根目录运行：

```powershell
./build.ps1
python tests/check.py
```

构建使用 `--locked --offline`，在根目录生成 `.exe`，不会覆盖已有的 `tools.tsv`。新 checkout 会从注释模板生成本地空映射。测试编译自己的原生夹具，不需要 Scoop、GNU utilities、BusyBox 或旧 Python 版本。

## 使用

在 PowerShell 7 中：

```powershell
./clink-posix-native.exe -c 'echo "$(echo hello)"'
./clink-posix-native.exe -c 'false; echo $?'
```

复杂命令推荐用 `--request UTF8_FILE`，避免调用者的 shell 先展开特殊字符。请求只接受单个物理行，文件末尾也不要附加换行。

### 选择原生工具

编辑本地 `tools.tsv`，格式为 `命令名<TAB>绝对 exe 路径<TAB>可选固定参数`，或通过 `--tools PATH` 指定另一份映射。参考 [模板](tools.example.tsv)。文件被 Git 忽略，不随仓库发布。

未映射命令从 PATH 查找 `.exe/.com`。Windows 和 GNU 工具可能存在同名冲突，尤其 `sort`、`find`；需要 Unix 选项时应明确映射到对应工具，不依赖 PATH 顺序。

配置好 `printf`、`sort` 和 `diff` 后：

```powershell
./clink-posix-native.exe -c "printf '%s\n' b a | sort"
./clink-posix-native.exe -c "diff <(printf 'same\n') <(printf 'same\n')"
```

### Clink 预览

`native-preview.lua` 提供显式 `nx <整行>` 入口。先在隔离会话中验证：

```powershell
./tests/clink-smoke.ps1 -ClinkExe C:/path/to/clink_x64.exe
```

这个检查需要实际控制台/PTY，会启动独立 CMD、运行测试并退出。它不安装全局脚本。已有输入改写扩展可能与预览适配器竞争，正式集成前需要统一入口。

## 支持边界

支持单双引号、空参数、`$NAME` / `${NAME}` / `$?`、嵌套 `$()`、有限内容的 `<()`、并发管道、顺序重定向、here-string、`;` / `&&` / `||`，以及行内目录/环境变量状态。展开后的参数不重新解释为 shell 源码。

- `<()` 通过临时文件传递有限数据，单次最多 8 MiB；不是 Bash 的流式 FIFO。
- 管道保留字节，命令替换要求 UTF-8；普通 Windows 路径反斜杠采取明确的兼容规则。
- `/dev/null` 目前只在重定向位置映射为 Windows NUL；任意程序的路径参数不自动改写。
- 不支持 source、完整脚本、glob、花括号、算术、命令组、DOSKEY 字符串宏或后台作业。未支持语法明确报错。
- 父 CMD 状态同步、取消/后代进程回收和控制台编码尚需正式集成验收。

详见 [模型边界](DESIGN.md) 和 [下一版虚拟设备提案](docs/rfcs/0001-virtual-devices.md)。提案不代表设备已经实现。

## 性能与验证

[历史性能报告](PERFORMANCE.md)记录了原型在一台 Windows 机器上的测量：简单命令约从 182 ms 降至 16 ms，执行器峰值工作集约从 20–21 MiB 降至 4–5 MiB。它是已归档原型的历史结果，不是本仓库每次构建的性能保证。

```powershell
python tests/benchmark.py
```

当前便携基准使用自带夹具；历史报告使用 uutils/BusyBox，两种负载不可直接混为同一测量。CPU、内存和 I/O 计数仅覆盖执行器进程，延迟包含工具执行，不含 Clink 提示符和父状态同步。

更多信息：[贡献与验证流程](CONTRIBUTING.md)、[本地验证记录](docs/VALIDATION.md)、[变更记录](CHANGELOG.md)。仓库包含 Windows CI 配置；远程 CI 尚需在推送后运行。

尚未选择开源许可证，`publish = false`；不会自动发布到 crates.io。
