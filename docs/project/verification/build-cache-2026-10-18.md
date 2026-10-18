# 主工程启动构建缓存修复

日期：2026-10-18。状态：主工程 Debug 构建、workspace 编译检查与非 UI 测试已通过；实际 EXE 的 SDK CLI 已通过。格式检查仍有两处本轮开始前已有的差异。

## 症状与反馈循环

用户从主工程启动构建，截图显示 111 个错误，其中项目克隆界面无法找到 `editor_core::display_path`。当前主工程 `crates/editor-core/src/lib.rs` 明确导出该接口，`crates/platform-windows/src/lib.rs` 也有转导出；本轮不修改这些既有 Rust 源码来补造重复接口。

从 `C:/Projects/RustProjects/Editor` 执行真实入口：

```powershell
cargo build -p editor-app --bin editor-app
```

首次实际 RED 7.51 秒，报 `platform_windows::display_path` 缺失；`-vv` 重复为 RED 3.90 秒，十处同类错误。日志分别为 `target/startup-build-2026-10-18-red.log` 和 `target/startup-build-verbose-red.log`。单独 `cargo build -p plugin-runtime --lib` 为 GREEN，说明必须保留完整 editor-app 的依赖功能组合，不能用单 crate 成功替代原始入口。

## 原因与修复

依次检查旧产物混用、错误工作区入口、功能组合差异。`cargo metadata --no-deps --format-version 1` 确认全部本地依赖均指向主工程，`target_directory` 为主工程的 `target`。verbose 日志却将缺失接口的上游 crate 判为 Fresh；同目录 dep-info 还保留 `target/git-ticket119-source/..` 快照构建及没有 `paths.rs` 的旧 core 记录。

此前本任务将内容不同的 Nanobug 源码快照与主工程共用 Cargo target，留下了与当前源码不匹配的依赖产物。只失效工作区自身五个 crate 的缓存、保留源码和第三方依赖缓存后，原始命令成功，证实问题在编译产物复用；没有以新增接口、修改调用方或升级依赖掩盖问题。

清理前核对绝对目录为 `C:/Projects/RustProjects/Editor/target`，执行：

```powershell
cargo clean --target-dir C:/Projects/RustProjects/Editor/target -p editor-core -p platform-windows -p plugin-schema -p plugin-protocol -p plugin-runtime
cargo build -p editor-app --bin editor-app
```

日志 `target/startup-cache-clean.log` 记录删除 33,206 个编译产物文件／99.3GiB；本命令按 Cargo 包清理 debug、release 及测试编译缓存，不删除源文件或独立交付 ZIP。重建日志 `target/startup-root-build.log`：71 秒、exit0。既有 unused 与 Wasmtime／Tree-sitter LNK4217 警告仍按实际输出保留。

主工程 `target/debug/editor-app.exe` 已生成，SHA-256 `718992670be5f0e93177850f0e97e083cbf687f95feeb7fed5963b3ced568369`。实际调用此 EXE 的 `--export-plugin-sdk target/startup-root-sdk-smoke-2026-10-18` exit0、51 文件，说明当前主工程的可执行文件可以正常执行 CLI 启动；不将 CLI 验证表述成 GUI 全流程验收。当前主工程包含此前其他工作区修改，SDK 文件数与先前冻结快照不同。

## 后续约定与验证

主工程继续使用自身 target；其他内容的源码副本从副本根构建，并使用独立输出目录。构建前检查真实 `workspace_root`、`target_directory` 及显式目录覆盖。若怀疑缓存，先复现、核对真实依赖来源，再按包失效当前工作区自己的产物；不需要清空全部第三方依赖或修改产品接口。

本轮同步双语 README 和仓库 AGENTS.md 中的构建隔离约定。metadata 再核对主工程输出为 `Editor/target`，快照默认输出为 `Editor/target/git-ticket119-source/target`，两者不同；此前错误来自为快照显式指定主工程输出目录。核对记录为 `target/startup-target-isolation.json`，本轮没有再次将快照构建到主工程目录。

| 检查 | 实际结果 | 日志 |
| --- | --- | --- |
| 重复执行原始 Debug 构建 | exit0，0.72 秒 | `target/startup-build-repeat.log` |
| `cargo check --workspace --jobs 1` | exit0，16.55 秒 | `target/startup-workspace-check.log` |
| `cargo test -p plugin-runtime --test application_management --jobs 1` | 4 passed／0 failed／2 ignored | `target/startup-application-management-test.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | exit0，78 组，289 passed／0 failed／305 ignored；编译阶段 7 分 03 秒 | `target/startup-nonui-workspace-tests.log` |
| 完整测试后再执行原始 Debug 构建 | exit0，1.21 秒 | `target/startup-post-tests-build.log` |
| `cargo fmt --check` | exit1，仅已有的 `extensions/preview/toolbar.rs:65`、`extensions/surface.rs:86` 两处格式差异；本轮不修改无关 Rust 源码 | `target/startup-root-format.log` |

汇总见 `target/startup-validation-results.json` 和 `target/startup-workspace-test-totals.json`。ignored 测试没有执行，不计为通过；本轮未修改 editor-app 产品源码，未追加 GUI 交互验收，也不将非 UI 测试替代此前独立候选的原生验收。

此前暂存面板验收将根工作区 `application_management` 的 `Manager::select_application` 缺失归因于其他任务源码不完整，该归因有误：当前源码已有实现，清理混用缓存后同一测试实际编译并通过。旧失败日志与独立候选的历史结果保留，以本记录补充根工作区重新验证结果；不改写先前交付 ZIP 或其摘要。

返回[文档总目录](../../README.md)。
