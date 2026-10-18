# 冷启动插件预览误报与首次激活回归

日期：2026-10-14。用户报告 Markdown 0.17.2 在全局启用状态下出现 `InvalidState: File provider is disabled` 与 `InvalidState: Preview plugin is disabled`。本次修复宿主通用预览生命周期；依据为[社区平台规格](../specs/plugin-api-community-foundation.md)的按需激活、原始意图和精确版本规则，以及[插件 UI 规格](../specs/plugin-ui-decoupling.md)的文件提供者与源文本边界。

源码修复、六项相关回归、规定检查及两轴代码复审已完成。另有一项既有测试的过时错误文案断言在未修改上游同样失败，详见下文。没有提交、推送、关闭 issue 或发布版本，也没有替换用户正在运行的程序。

## 比较点与改动归属

最新上游 `main` 为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`，本次复审前重新读回 `git ls-remote origin refs/heads/main` 与 HEAD，结果相同。前一批共用布局已提交；不将其计为本次修复。

本次生产代码只有 `crates/editor-app/src/extensions/worker/runner.rs`、`extensions/preview.rs`，另在 `extensions/worker.rs` 声明 `src/tests/plugin_preview_activation.rs` 测试模块。测试文件位于跨功能测试目录，保持私有 actor 接缝，不扩大生产 API 可见性。

工作区同时存在菜单、运行工具栏、Markdown HTML 与图片呈现等其他任务改动。验证候选从上述上游提交直接导出到 `C:/Users/Tmiracle/.codex/tmp/preview-error-20261014-1`，只覆盖本次四个 Rust 文件；不包含其他任务的改动。主工作区与候选的四文件 SHA-256 已逐一核对相同。

本次开始时 `Cargo.lock` 已有仅换行差异，原始字节 SHA-256 为 `46f6fb29216824eac10793a136bd74ab8e089cba2fcba93554fe0148a8eacb11`；保留该状态，没有修改依赖、公开协议、SDK、权限声明或插件包版本。冻结 `markdown-0.17.2.zip` 保持原字节，SHA-256 为 `cac840637b5348c553da810e0d101f318088714caec18e30710bbc97c8327e13`。

## 已验证原因与修复

1. 原生打开文件时发送 FilePreview / Preview，静态提供者已经可选，但真实实例仍未因可见绘制启动。Manager 在缺少 live owner 时返回截图中的两个 InvalidState；全局启用与包校验本身正常。Worker 先执行原有 Manager 校验，仅在源回调、无实例、无已记录实例错误、有效 availability 且 typed InvalidState 同时成立时接受这个冷状态。首次实例发布仍通过 epoch 变化清除原生已发送标记，再发送最新源文本；没有唤醒隐藏视图。
2. 加载期间编辑同一文档会改变 revision。原先 `view_demand` 将完整版本变化当作撤销原始意图，实际首次激活失败为 `InvalidHandle: Activation original intent was revoked`。现在仅在 ID/path 改变或视图失活时取消；同一文档编辑和同一文件刷新保留视图意图。源发送、工具事件与 guest 场景仍检查完整版本，旧 revision 不获得新文档的权限。

没有插件 ID 白名单，没有按错误文案吞并所有故障。权限、非法输入、过期回调、真正禁用、受限工作区及真实 guest 故障继续沿现有路径处理；生产 Manager 与公开契约未改动。

## 真实回归入口与红绿证据

三个新增用例使用冻结真实 ZIP → 公开 Package/Manager 安装 → 重新打开冷 Worker → 生产 GPUI 文件窗口。冷启动症状用例在实际打开文件后直接使用 checked shutdown 作为通知队列屏障，刻意不绘制或人为 demand，防止预热掩盖问题。

首次激活正例由真实原生绘制触发 demand，通过实际编辑器焦点、剪贴板和 `ctrl-a ctrl-v` 输入 `# Latest 中文`；断言编辑器内存文本、当前 DocumentVersion、真实 guest 场景内容和原生预览边界，磁盘仍保持原文。负例向同一冷 actor 提交无绑定的非空 Preview 与未声明 FilePreview 面板，分别保留 LimitExceeded、PermissionDenied 两条错误。

| 阶段 | 实际结果 | 本机日志（仓库根 target） |
| --- | --- | --- |
| 原行为截图症状回归 | 0 passed、1 failed；18.78s，包含两个 disabled 错误 | `plugin-disabled-red-3.log` |
| 同一原行为重复探针 | 0 passed、1 failed；18.98s，两个错误再次出现 | `plugin-disabled-red-repeat.log` |
| 冷回调修复后 | 1 passed、0 failed、0 ignored；17.13s | `plugin-disabled-green.log` |
| 同文档加载期间编辑，原取消条件 | 0 passed、1 failed；33.74s，实际 entry.error 为原始激活意图已撤销 | `plugin-disabled-isolated-birth-probe.log` |
| 身份取消条件修复后 | 1 passed、0 failed、0 ignored；42.52s，最新未保存中文实际呈现 | `plugin-disabled-isolated-birth-green.log` |
| 最终三项显式 opt-in 回归 | 3 passed、0 failed、0 ignored；68.28s | `plugin-disabled-isolated-final-native.log` |

早期测试入口曾因私有接缝与遗漏导入编译失败；修正测试模块连接与导入后才取得上述行为红灯。主工作区还曾遇到其他任务尚未创建的 `run_ui_tests/toolbar.rs` 导致编译失败，因此改为独立候选验证。扩大窗口的中间尝试仍超时，不能将窗口尺寸视为根因；最终通过实际 entry.error 定位到原始意图撤销。各中间失败未计为通过。

## 规定检查与复用回归

使用现有 VS 18 x64 工具链、锁定依赖，`CARGO_TARGET_DIR=C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/target01`、`CARGO_BUILD_JOBS=2`、`RUST_MIN_STACK=16777216`。没有安装新工具或调用历史构建脚本。

| 实际命令/过滤词 | 结果 | 本机日志（仓库根 target） |
| --- | --- | --- |
| `cargo fmt --check` | exit 0 | `plugin-disabled-isolated-fmt.log` |
| `cargo test -p editor-app preview_activation_tests -- --ignored --nocapture --test-threads=1` | 3 passed、0 failed、0 ignored | `plugin-disabled-isolated-final-native.log` |
| `cargo test -p editor-app extensions::preview_tests -- --test-threads=1` | 1 passed、0 failed、0 ignored；0.75s | `plugin-disabled-isolated-preview-tests.log` |
| 同一测试二进制，`obsolete_ui_callbacks_are_information_but_real_rejections_are_errors --exact --ignored --nocapture --test-threads=1` | 1 passed、0 failed、0 ignored；49.02s | `plugin-disabled-isolated-stale-callbacks.log` |
| 同一测试二进制，`native_cold_menu_and_view_wait_keep_exact_owner_without_eager_birth --exact --ignored --nocapture --test-threads=1` | 1 passed、0 failed、0 ignored；86.93s | `plugin-disabled-isolated-view-retirement.log` |
| 同一测试二进制，`operation_errors_retain_plugin_ownership_through_worker_publication --exact --nocapture --test-threads=1` | 0 passed、1 failed；0.12s，过时文案断言；未修改上游同样失败 | `plugin-disabled-isolated-operation-ownership.log` |
| 未修改上游的 `cargo test -p editor-app extensions::worker::worker_tests::operation_errors_retain_plugin_ownership_through_worker_publication -- --exact --nocapture --test-threads=1` | 0 passed、1 failed；0.03s，相同断言；独立归属对照 | `plugin-disabled-baseline-operation-ownership.log` |
| `cargo test --workspace --exclude editor-app` | exit 0；267 passed、0 failed、255 ignored | `plugin-disabled-isolated-workspace-tests.log` |
| `cargo check --workspace` | exit 0；15.44s | `plugin-disabled-isolated-check.log` |
| 本次范围 `git diff --check`、新增文件空白与文档链接/路径检查 | exit 0 | 终端读回 |

本次六项不同的相关用例实际通过；非 UI 的 255 项 ignored 未执行，不计为通过。上述额外原生用例直接运行同一 `editor_app-d4e1859b30e4cb6a.exe`，完整路径位于共享缓存的 `debug/deps`；过滤词实际带完整模块名。最终候选测试二进制在这些回归执行时的 SHA-256 为 `3e7702a938f0f3bca2668a7ed4cba32471ca93178aaf3d93b738d14ee48fe1cd`。之后上游对照构建复用了该输出位置，当前缓存文件不能当作原候选二进制。源码未再修改；主工作区与已验证候选四文件的摘要仍相同。编译存在既有未使用项与 Tree-sitter/Wasmtime 链接警告，不宣称零警告。

既有 `operation_errors_retain_plugin_ownership_through_worker_publication` 仍匹配硬编码 `Plugin command owner changed`。未改动的 `worker/demands.rs` 使用 i18n 键 `plugins.command_owner_changed`，其当前英文文案为 `The plugin command was replaced or disabled.`，中文为 `插件命令已被替换或禁用。`。从同一提交原样导出的 `C:/Users/Tmiracle/.codex/tmp/preview-error-baseline-20261014-1` 重新构建，原测试在相同断言失败。该失败是已有测试与文案不一致，不计为本次修复通过，也未顺手修改其行为或断言。

## Standards

独立代理按仓库约定和十二项 Fowler 基线审查固定点到最终四文件差异：硬性违规 0，Fowler 坏味道 0，无阻断项。两处生命周期判定、typed 错误边界、模块接缝、注释和真实回归入口均检查；没有编辑文件或自行构建。

## Spec

独立代理按社区平台与插件 UI 规格审查同一范围：缺失 0、范围蔓延 0、错误实现 0，未解决问题 0。复核保留同身份首次激活、切换/隐藏撤销、精确源版本与通用插件准入，不将其他工作区改动纳入结论。两轴代码复审不替代上述实际测试或交付核对。

## 交付与限制

本次为宿主源码修复；用户当前运行的旧二进制与既有错误日志不会自动改变，重新构建宿主并重启后才使用新行为。未安装、重装或升级用户插件，没有清理插件数据或日志。

Windows 自动化原生回归已覆盖真实键盘输入、冷可见绘制和实际 guest 结果；本轮没有人工桌面操作、独立 IME 组合过程、所有插件逐包检查或 macOS/Linux 实测。不会将未执行的场景宣称通过。
