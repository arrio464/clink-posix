# Clink POSIX Native

A bounded POSIX-style interactive command executor for native Windows programs.

Windows x64 / Rust 0.2。已补齐旧 Python px 的常用交互功能，直接提供 `px`、auto/on/off 和跨行目录/环境状态。Lua 只负责 Clink 接入；无需 Python 运行时、Cargo 第三方依赖或常驻服务。

仍是实验性工具，不是 Bash 或完整 .sh 实现。具体差异与资源限制见 [DESIGN.md](DESIGN.md)。

## 构建和验证

```powershell
./build.ps1
python tests/check.py
./tests/clink-smoke.ps1 -ClinkExe C:/path/to/clink_x64.exe
```

构建需要 Windows、Rust 1.98+、PowerShell；测试需要 Python 3.10+。离线构建使用锁定版本。测试自己编译原生夹具，不依赖 Scoop、GNU 工具或旧 Python px。Clink 会话检查需要真实控制台/PTY，不要重定向整个检查脚本的输出。

可选旧版差分：`python tests/check.py --python-reference C:/path/to/native_shell.py`。

## 安装和升级

先构建，再运行 `./install.ps1`；已经安装本项目预览版时用 `./install.ps1 -Upgrade`。完成后新开 CMD。

安装器使用当前用户的 `%USERPROFILE%/.clink`。升级前按安装记录校验受管理文件，备份 exe、Lua、工具配置、加载器和记录；保留现有 tools.tsv。不会改写 Python px 文件。新加载器在原生适配器加载成功后阻止旧 px 重复注册。

停用：`./disable.ps1`，然后新开 CMD。它停用原生加载器，已有 Python px 加载器会重新生效，原生运行文件和备份保留。

## 日常使用

```text
px status
px echo "$(echo hello)"
px echo {a,b} {1..5} $((2*(3+4)))
px echo "${MISSING:-default}"
px echo *.rs
px (echo b; echo a) | sort
px export MODE=dev
px echo "$MODE"
px cd 'D:/project'
px pwd
px printf '%s\n' b a | sort
px diff <(echo same) <(echo same)
px command -v sort
```

- `px <命令>`：强制使用原生语法；`nx` 保留为兼容别名。
- `px auto`：沿用旧版启发式路由，识别 POSIX 语法和常见 Unix 命令，保留 CMD 内建命令。
- `px on`：会话中的输入都使用原生语法；`px off`：普通输入走 CMD。
- `px cmd <原始 CMD 命令>`：明确使用 CMD，包括 .cmd/.bat。
- mode 命令只改变当前会话；默认值仍由 Clink 的 `posix.mode` 控制。

auto 是方便使用的兼容模式，有歧义时用显式 px 或 px cmd。同一行不会在失败后换 Python/Bash/CMD 重试。

## 工具映射

在被 Git 忽略的 `tools.tsv` 中使用 `名称<TAB>绝对 exe 路径<TAB>固定参数`，参考 [模板](tools.example.tsv)。没有映射时查找 PATH 中的 .exe/.com。Unix 工具需要自行安装；尤其 sort/find 与 Windows 同名程序应明确映射。

独立运行可用 `clink-posix-native.exe -c "echo hello"`，或 `--request UTF8_FILE`。请求文件支持换行、续行和注释，但不支持脚本控制结构。

## 支持和限制

支持引号、变量、参数默认值/替代值、整数算术、glob、花括号列表/数值范围、~、嵌套 $()、有限 <()、命令分组、管道、重定向、here-string、短路执行、简单 DOSKEY 宏，以及父会话 cwd/env/status 同步。

额外提供 `${#NAME}`、`:`、`command -v`、`cd -- DIR`。

- source、函数/循环、作业控制、反引号、heredoc、递归 ** glob 仍不支持。
- <() 是最多 8 MiB 的临时文件替换，不是持续流。命令替换要求 UTF-8，管道保留原始字节。
- 算术使用检查溢出的 i64；普通 glob 不匹配点文件，无匹配保留原文字面量，保留模式里的路径分隔符。
- /dev/null 仅在重定向位置映射为 NUL；zero/random/urandom 仍是 [后续提案](docs/rfcs/0001-virtual-devices.md)。
- CMD 父目录同步面向本地/盘符路径；不要依赖 UNC cwd。含 ! 的目录在启用 CMD 延迟展开时尚未保证。
- Ctrl+C、后代进程统一回收、全局资源预算和不同控制台编码仍需专项完善。

## 性能

0.2 本机 25 轮测量：内建 echo 中位数 17.0 ms，已安装 Python 版 201.1 ms；执行器峰值工作集中位数约 4.4–5.2 MiB，exe 563 KiB。测量包含工具执行，未包含 Clink 提示符和父状态同步；不是所有机器的性能保证。

见 [当前验证记录](docs/VALIDATION.md)、[0.2 原始测量](docs/benchmarks/2026-10-11-parity.json) 和 [历史原型报告](PERFORMANCE.md)。当前夹具与历史 uutils/BusyBox 负载不同，不能直接对比两版外部命令耗时。

没有选择开源许可证，`publish = false`；代码只在本地整理，远程 CI 需要推送后执行。
