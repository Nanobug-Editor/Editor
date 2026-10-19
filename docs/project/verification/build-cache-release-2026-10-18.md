# 主分支 Release 启动复查

日期：2026-10-18。对应用户 19:41 的主分支启动报错。状态：同样的 111 个错误已复现并消除；IDE 的 Release 构建及实际 EXE 的 CLI 启动均通过。本轮没有修改产品 Rust 源码或依赖。

## 实际入口与反馈循环

主分支 `eace01458ff422cdb37947c2a7674681336550ee` 开始时工作区干净。只读取本地 IDE 配置的命令、工作目录和 profile：build 为 `build -p editor-app --release`，run 为 `run --release`，工作目录为项目根；没有改变 IDE 配置。

Debug 实际构建通过，26.37 秒。随后执行 IDE 的真实构建命令：

```powershell
cargo build -p editor-app --release
```

首次 exit101、9.28 秒，恰为 111 个错误，包含截图中的两处 `editor_core::display_path` 缺失，以及 `ui::ContentStyle`、`Layout.align/margin` 和 Manager 应用级管理接口缺失。`-vv` 再次同样失败，9.34 秒。日志为 `target/main-release-startup-red.log` 与 `target/main-release-startup-verbose-red.log`。

从 verbose 的应用 rustc 命令提取其真实 `--extern editor_core`，指向 `target/release/deps/libeditor_core-a97477efe4c51abe.rlib`。只通过这个库进行一次 `display_path` 调用的 metadata 编译，精确复现 `E0425`；无需加载 UI 或链接整套程序。源码导出存在，Release dep-info 却没有 `paths.rs`，库文件仍为 14:36 的旧产物，而 Cargo 将相关五个工作区库判为 Fresh。metadata 确认依赖仍来自主工程，相关 core 没有可排除该接口的 feature，排除错误工作区和接口条件编译假设。

最小探针与输入、RED/GREEN 日志保留在 `target/main-release-api-probe*`；探针直接使用真实 extern，不伪造或补造产品接口。

## 原因与恢复

先前跨源码副本共用 target 留下旧产物；首轮只恢复了 Debug，漏掉用户实际运行的 Release。此前“按包清理会同时覆盖 debug/release”的说明有误。

用没有第三方依赖的独立小项目实际分别构建 Debug 和 Release，再执行 `cargo clean -p nanobug-clean-profile-probe`：Debug 消失、Release 仍在；加 `--release` 后 Release 才消失。该诊断有独立 workspace/target，没有复用主工程输出；结果见 `target/cache-clean-profile-probe-results.json`。这将 profile 差异变成实际 CLI 回归，而不是文档推断。

确认输出目录确为 `C:/Projects/RustProjects/Editor/target`，target 与 release 不是重解析点，且没有运行中的程序占用该 Release 目录后，只清理五个工作区库：

```powershell
cargo clean --release --target-dir C:/Projects/RustProjects/Editor/target -p editor-core -p platform-windows -p plugin-schema -p plugin-protocol -p plugin-runtime
cargo build -p editor-app --release
```

实际删除 167 个文件、405.7MiB；不清空第三方依赖、Debug 产物、源码或独立交付 ZIP。清理日志为 `target/main-release-package-cache-clean.log`。

## 结果与范围

| 检查 | 实际结果 | 日志 |
| --- | --- | --- |
| 同一 extern 的最小接口探针 | 清理前 exit1／E0425；重建后 exit0 | `target/main-release-api-probe-{red,green}.log` |
| 原始 IDE Release 构建 | exit0，1 分 17 秒 | `target/main-release-startup-green.log` |
| 重复同一 Release 构建 | exit0，0.63 秒 | `target/main-release-startup-repeat.log` |
| `cargo run --release -- --export-plugin-sdk C:/Projects/RustProjects/Editor/target/main-release-sdk-smoke` | exit0，真实 Release EXE 执行；导出 51 文件 | `target/main-release-run-sdk-smoke.log` |

生成的 `target/release/editor-app.exe` 为 118,476,288 字节，SHA-256 `7ece4d1c03152c79d7166ef03b3fa4e4a8008c5b55d7ebd9d8707fd9c8c8c449`。汇总为 `target/main-release-startup-verification.json`。unused 与 Wasmtime／Tree-sitter LNK4217 警告按实际输出保留。

这次验证覆盖用户所报的 Release 编译失败和实际 Release CLI 启动，没有将 SDK 导出冒充 GUI 全流程。本轮不重复无关 Rust 全仓或原生交互测试；只修复本机失效缓存并更正文档，文档检查链接与 `git diff --check`。既有测试结果继续保留在[主分支整合记录](main-integration-2026-10-18.md)。

返回[文档总目录](../../README.md)。
