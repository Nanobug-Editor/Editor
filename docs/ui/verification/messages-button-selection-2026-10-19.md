# 消息按钮选中状态修正

日期：2026-10-19。

## 原因与修复

消息面板打开时，按钮已正确报告展开状态，但 `expanded` 只设置 Base 的无障碍语义，没有绘制持续选中的背景。消息按钮现在依据 `messages_visible` 使用与其他窗口按钮相同的 `PanelToggle.selected` 背景和前景；面板收起或所在停靠区关闭时取消选中，保留原有提醒、焦点和点击行为。

实现位于 `crates/editor-app/src/ui/messages.rs`，要求同步至[宿主消息窗口规格](../specs/host-messages.md)。不修改工作区已有的停靠布局与会话逻辑。

## 自动验证

新增 `tests::host_messages::button::host_messages_button_selection_tracks_panel_visibility`，通过生产按钮输入打开和收起面板，移开指针后检查实际绘制背景、面板可见性和展开语义。覆盖浅色 100%、深色 150%、底部按钮关闭、面板标题栏关闭以及整个右侧停靠区关闭后重新打开。

- 修复前连续两次失败，均为面板可见而按钮未绘制选中背景，运行分别为 0.15 秒、0.13 秒。
- 修复后针对性回归通过，1 passed / 0 failed，0.68 秒。
- `cargo test -p editor-app host_messages -- --nocapture`：7 passed / 4 failed / 0 ignored。
- 修复前与修复后测试程序分别串行运行 `host_messages --nocapture --test-threads=1`：6 passed / 5 failed 与 7 passed / 4 failed。四个共同失败点如下；本次只消除了新增选中状态回归的失败，未修复其他问题。

| 既有失败测试 | 失败位置 |
| --- | --- |
| `host_messages_confirmation_and_clear_preserve_plugin_logs` | `app/plugins/status_tests/host_messages.rs:46`，初始空消息断言 |
| `host_messages_run_admission_retains_host_refusals` | `run/run_ui_tests/host_messages.rs:84`，拒绝运行的消息断言 |
| `host_messages_extend_legacy_layout_and_support_native_keyboard` | `tests/host_messages.rs:456`，旧布局加载后的隐藏断言 |
| `host_messages_record_file_failure_and_reopen_without_losing_history` | `tests/host_messages.rs:552`，重新打开后的会话显隐断言 |

`cargo check --workspace`、`cargo build -p editor-app --bin editor-app` 均 exit0。`cargo test --workspace --exclude editor-app` exit0，295 passed / 0 failed / 309 ignored；ignored 未执行，不计为通过。编译保留已有 unused 与 Wasmtime/Tree-sitter 链接警告。

`cargo fmt --check` 中途执行为 exit0，最后复查为 exit1，仅报告并发工作新增的 `ui/controls/aligned_editor/geometry.rs`、`ui/controls/aligned_editor/gutter.rs`、`ui/controls/aligned_pointer.rs` 格式差异，本次未修改这些文件。本次三个 Rust 文件的 `rustfmt --check --edition 2024` 及 `git diff --check` 通过；最终格式日志为 `target/messages-button-format-final.log`。

复现日志位于 `target/messages-button-red-run.log`、`target/messages-button-red-repeat.log`；对照日志为 `target/messages-button-baseline-serial.log`、`target/messages-button-green-serial.log`。独立命名测试 EXE 避开了另一项原生测试占用默认 EXE 引起的 LNK1104；未终止其他任务进程。

## Windows 原生复验

原有 Release 窗口中已复现打开面板、移开指针后缺少选中背景。修复后的 Windows/D3D 窗口使用生产 `EditorApp`、隔离工作区、独立配置和显式受限会话，英文浅色主题下通过 Computer Use 实际输入与截图检查确认：

- 点击消息入口打开面板，再点击标题栏移开指针，消息按钮保持与资源管理器入口一致的蓝色选中背景。
- 点击面板标题栏收起按钮后，面板消失，消息入口恢复未选中背景。
- 再次点击入口能重新打开；入口聚焦后 Space 收起、Enter 重新打开均正常。

普通隔离启动遇到首次工作区信任提示后结束该测试进程，未接受信任提示；临时原生测试入口直接构造受限会话，不加载用户插件或修改用户配置。该入口仅承载真实窗口，样式由实际输入与截图检查；颜色断言由上述 GPUI 回归测试负责。验收完成后等待项目清理并退出窗口，入口已从产品源码树移除，源码和运行日志保留在 `target/messages-button-native/acceptance-source.rs`、`target/messages-button-native/run.log`。

返回[原生 UI 文档目录](../README.md)。
