<!-- 历史验收：此记录引用主工作区 2026-10-09 的 Rust 合并成果，不代表市场工单 03 已重新执行这些命令或已发布这些产物。本次独立版本与新验证以市场工单 03 记录为准。 -->

# Rust 语言、Cargo 与调试器合并验收

2026-10-09，用户授权“开始合并”。对应[方案](../specs/rust-plugin-consolidation.md)。本次修改工作区与开发产物，没有提交、推送或发布，也没有修改真实用户的插件安装与设置。

## 交付范围

- Rust 0.6.0 保留安装身份 `rust`，语言配置、Cargo 和调试策略共同编译到一个行为组件 `rust.wasm`。Tree-sitter grammar 仍是独立 WASM 资源。
- CodeLLDB 仍固定为 1.12.3；Windows x86_64 附带原生桥。安装和普通激活不获取适配器，首次 `debug.session.start` 在后台准备，后续复用校验过的缓存。
- 通用 `dependencies 1.2` 提供按方法准备、贡献启用设置和有限替代；宿主没有按 Rust 或 CodeLLDB 身份执行的业务分支。协议仍为 7，独立 SDK crate 仍为 0.2.0。
- 原 `rust-debugger` 0.1.x–0.2.x 在具体安装确认后替代，保留旧私有文件、逻辑快照、缓存收据、用户/项目启用偏好及明确的提供者选择。未知版本、跨作用域或其他工作区仍持有旧实例时明确拒绝。
- 非 Windows x86_64 打包过滤适配器、桥、相应权限与替代规则，保留语言和 Cargo。

Windows 插件包为 `dist/plugins/rust-0.6.0.zip`，SHA-256：

```text
fc87c9da512da15b3e7344b05f953d55a8a55c66a00bc6e84ab92874691f376f
```

开发市场目录里的旧 Rust ZIP 已移到忽略的 `target/rust-merge-baseline/market-originals/`，该目录只保留合并版 Rust 候选。性能和替代夹具使用独立保存的旧 `rust-0.5.1.zip` 与 `rust-debugger-0.2.0.zip`。

## 构建与环境

Windows x86_64、现有 MSVC 工具链，使用仓库锁定依赖，没有安装或升级工具。构建直接调用 Cargo 和宿主入口，没有使用仓库或存档脚本：

```powershell
cargo build -p editor-app
./target/debug/editor-app.exe --plugin-package plugins/rust --output dist/plugins
./target/debug/editor-app.exe --plugin-package plugins/run-target-example --output dist/plugins
./target/debug/editor-app.exe --plugin-package plugins/rust --platform linux-x86_64 --output target/rust-merge-portable
```

链接测试使用现有 Visual Studio Developer Command Prompt。直接 PowerShell 启动的完整测试最初未找到 `msvcrt.lib`，通过现有 `VsDevCmd.bat -arch=x64 -host_arch=x64` 设置工具链环境后重跑。原生 GPUI 测试使用 `RUST_MIN_STACK=16777216`。测试编译和同名测试进程串行运行，避免 Windows 可执行文件占用导致 LNK1104。

真实 WASM 测试读取当前 ZIP、实际编译的原生桥和本地已固定的 VSIX；测试仅替换声明中的依赖来源，SHA-256 与 guest 行为保持实际包的契约。故障测试使用公开契约和独立故障适配器，不新增宿主测试专用 API。

## 验证结果

| 检查 | 结果与覆盖 |
| --- | --- |
| `cargo fmt --check` | 通过；Rust guest 与原生桥另执行 `rustfmt --check` 通过 |
| `cargo test --workspace --exclude editor-app` | 最终 232 passed、186 ignored；忽略项不计为通过 |
| `cargo check --workspace` | 通过，19.54 秒 |
| Rust guest 单元测试 | 6 passed：语言项目发现/路径/配置、Cargo 绑定及调试协议解析 |
| `cargo test -p plugin-runtime --lib` | 最终 workspace 执行的 runtime lib 为 83 passed，含项目启用和中断恢复检查 |
| `cargo test -p plugin-runtime --test rust_merge -- --ignored --skip compare_plugin_activation --test-threads=1` | 4 passed，178.95 秒 |
| `cargo test -p plugin-runtime --test real_debugging -- --ignored --test-threads=1` | 7 passed，130.95 秒 |
| `cargo test -p plugin-runtime --test real_targets -- --ignored --test-threads=1` | 5 passed，103.31 秒 |
| 原生安装确认 `second_market_install` | 1 passed：确认页实际显示旧身份替代及受影响会话 |
| 原生配置 `plugin_configuration_rollout_real_cargo_debug_pause_inspect_step_and_stop` | 1 passed，59.29 秒：真实表单、Cargo/MSVC 产物、CodeLLDB 断点/变量/F10/停止，使用改名的普通提供者 |
| 原生并行与卸载 `cargo_artifacts_debug_in_parallel_and_retire_through_native_management` | 1 passed，49.06 秒：两份真实 Cargo 产物调试、独立暂停/检查、F11/F10/Shift-F11/F5/F6、长栈滚动、深浅主题/中英文/字号、取消卸载与确认卸载、无关 PowerShell 执行保留 |
| SDK 导出、Rust grammar 和实际 Rust Analyzer | SDK 4 passed；grammar 1 passed；实际 Rust Analyzer 1 passed，32.09 秒：独立嵌套 guest 的补全、hover、定义跳转，源码清单不变且无本地 SDK 副本 |
| 平台过滤回归与实际 Linux 目标 ZIP | Windows 上打包通过；ZIP 不含桥和调试贡献，仍包含语言/Cargo；未在 Linux 上运行 |
| `git diff --check` 与文档链接 | 通过；核对 8 份受影响 Markdown 的相对文件链接 |

新增 `rust_merge` 的四项真实包验收覆盖：

1. 缺失适配器来源时安装成功；首次调试失败只结束该请求，语言资源与 Cargo 发现仍可用，无目标进程或插件禁用。
2. 本地 HTTP 接收后暂停响应，准备等待期间 Cargo 可完成；取消使下载等待失效，不启动迟到适配器或目标。
3. 使用真实旧 0.2.0 WASM，拒绝未批准的替代；批准后保留旧作用域内的私有文件、禁用偏好及用户/项目提供者选择。重启、重新启用调试和再次安装保留显式设置。
4. 在有日志的提交阶段撤回请求，恢复注册表、设置和提供者选择，旧实例继续可用；其他项目已选的独立调试器继续有效。

通用单元检查另覆盖项目单独启用和已有目标设置优先，以及有/无持久提交标记的中断恢复。后者验证注册表、私有文件、设置、提供者选择和新增替代历史的一致版本，并验证恢复可再次执行。

CodeLLDB 回归包括唯一目标的 stdin、PDB 源码断点、局部变量及步入/步过/步出。故障适配器覆盖初始化失败、过大检查回复、协作退出、停止超时和进程树清理；这些故障测试不能替代真实 CodeLLDB 的协议证据。Cargo 回归覆盖虚拟工作区、default-members、同名 bin、profile、准确编译产物、取消后的实际退出与无关执行保留。

## 加载对比

使用 `rust_merge::compare_plugin_activation_without_language_server_or_adapter_startup`。三组各安装一次，在独立临时存储中关闭后恢复管理器 7 次，记录中位数、最小/最大耗时和行为组件字节。计时期间未并行运行本任务的其他构建或测试；每次恢复断言全部预期组件已激活、无安装错误、没有启动原生进程。最终 1 passed，370.48 秒。

这是 Debug 宿主中插件管理器恢复、WASM 编译与实例初始化的比较，不是 Release 应用整窗冷启动，也没有测量内存峰值；磁盘缓存和编译优化会影响绝对时间。

| 包组合 | 行为 WASM 字节 | 活实例数 | 恢复中位数 | 最小/最大 |
| --- | ---: | ---: | ---: | ---: |
| 原 Rust 0.5.1 | 1,076,259 | 1 | 9,186.01 ms | 9,144.78 / 9,223.21 ms |
| 原 Rust + rust-debugger 0.2.0 | 2,396,756 | 2 | 20,564.40 ms | 20,503.56 / 20,685.10 ms |
| Rust 0.6.0 合并包 | 1,873,413 | 1 | 16,133.77 ms | 16,075.34 / 16,238.42 ms |

合并后的行为组件比两个原组件总字节少约 21.8%，恢复中位数比同时启用两个旧包少约 21.5%。相较仅语言/Cargo 包，合并包恢复增加约 75.6%；单个较大的 WASM 仍需要编译和实例初始化，关闭调试贡献不会消除其全部组件加载成本。CodeLLDB 的下载和进程启动已延后到首次调试，不能把这部分延迟准备等同于 WASM 代码完全按需加载。

## 修正与限制

- 实际安装回归发现，带本地语言服务的包在查询依赖计划时不应强制读取 SDK/发现项目。新增显式 `Context.preparing_dependencies`，实际语言准备仍执行原策略，未用空工作区或空根路径猜测调用阶段。
- 延迟准备使用准确的提供者实例声明，避免已停放的旧版本被当前安装记录的计划替换；独立方法计划使用独立收据，保留已有缓存引用。
- 原并行 UI 夹具沿用旧配置、内嵌窗口、旧终端包身份和旧调试进程数量。已改为当前公开配置表单、单一父视图、内置 Shell、每会话的桥与目标 PTY，并通过当前 Run Tab 选择检查对象。未新增旧配置兼容入口。
- 组合测试还捕获了内置 Shell 表单输入被丢弃的现存问题：事件接收端只承认插件开发配置，没有使用包含 Shell 的统一内置贡献者判断。修正后两类内置表单共用既有事件队列，配置夹具断言脚本校验成功，避免把“保存窗口关闭”当成输入已生效。
- Windows 的弃用/未使用项及 Wasmtime/Tree-sitter LNK4217 警告仍存在，不把这些警告报告为本次新增缺陷。
- 本机原生证据为 GPUI 实际布局、鼠标/键盘及真实进程测试，未额外执行手工可见窗口或操作系统 IME 候选验收。macOS/Linux 原生调试、平台运行和安装尚未验收。
- `target/debug/editor-app.exe` 已构建；既有 `dist/editor` Release 暂存和安装器未重新生成，不可把其中旧程序/旧 ZIP 当成本次交付。发行前按[安装器说明](../../../installer/README.md)重新构建宿主和包、核对文件清单及摘要。
- 工作区原有 SDK/文档整理、终端和其他改动保留，不将全部未提交 diff 归因于本次合并。

日志保存在忽略的 `target/rust-merge-*.log`，包括 guest、workspace、真实包、原生 UI、打包、平台过滤和加载测试；版本、覆盖和限制以本记录中的实际结果为准。
