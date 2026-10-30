# 带时间戳 stderr 警告的浮窗去重

日期：2026-10-30。状态：实现及最终自动化检查通过；Windows 原生验收范围见下；未发布。

用户截图的三条 `rust · lsp/analysis/stderr` 警告正文相同，只有行首时间戳不同。原浮窗以完整正文作为去重键，因此同一文件监听警告同时占满三张卡片。最小回放通过真实 `RuntimeLogs`、扩展观察器及主窗口消息入口，不启动或重启语言服务；旧实现稳定得到三张卡片而预期一张。现有重复导入和完全相同正文用例通过，排除了重复消费同一记录的解释。

修复只调整宿主自动浮窗的比较身份：宿主采集的 `lsp/<provider>/stderr` 与 `wasi/stderr` 忽略有效的开头 RFC 3339 时间戳和随后水平空白，剩余正文、来源和等级继续精确匹配。其他来源、无效时间及正文内时间不归一化。统一历史、插件日志和首张浮窗保持完整原文，关闭后新记录仍能提醒；没有按 Rust、插件 ID 或具体警告内容设置过滤器。

语言服务器仍可能生成文件监听警告。[rust-analyzer 上游 #19560](https://github.com/rust-lang/rust-analyzer/issues/19560)记录了相同消息；本轮没有更改其文件监听、工具版本、用户插件安装或日志等级，不宣称已修复上游文件监听失败。

## 回归与实际窗口

| 检查 | 实际结果 |
| --- | --- |
| 最小回放 `unified_messages_timestamped_stderr_coalesces_without_losing_history` | 旧实现 0 passed / 1 failed，断言 `left: 3 / right: 1`；运行 0.23 秒 |
| 旧实现 `unified_messages_` 对照组 | 6 passed / 1 failed，唯一失败为上述时间戳回放 |
| 修复后首次 `unified_messages_` | 9 passed / 0 failed，含两项时间戳边界测试 |
| 最终 `cargo test -p editor-app --bin editor-app messages -j 2 -- --nocapture` | 29 passed / 0 failed / 1 ignored；覆盖原始日志、关闭后新提醒、不同内容、跨插件、严重程度隔离及点击定位最新收据 |
| Windows 原生 `window_toolbar_native_windows_acceptance`（整合主分支导航变更前） | 重试 1 passed / 0 failed；浅色 100% 与深色 150% 均只显示一张重复警告卡片，真实鼠标关闭一次后为零，三条原日志仍在 |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test --workspace --exclude editor-app -j 2` | 874 passed / 0 failed / 446 ignored；未执行的 ignored 用例不计入通过数 |
| `cargo check --workspace -j 2` | 通过；存在基线已有的未使用代码及导入警告，本轮未扩大清理范围 |

首次原生验收已通过浅色警告场景，随后在既有工具栏拖拽步骤报 `held movement did not start native drag`。未修改测试或实现，使用相同命令重试后完整两组通过（13.97 秒）；保留首次失败日志，不将其描述为本次修复的稳定性保证。原生截图已人工核对，普通 GPUI 消息回归另覆盖焦点保持与中文输入。

交付前整合主分支 `be27a3ce` 的两行浮窗与点击定位历史变更，保留其行为，并新增真实 `RuntimeLogs` 驱动的点击回归：不同时间戳合并为一张卡片后，点击定位到最新原始收据。上述最终自动化结果来自整合后的源码。用户已通过物理 Esc 停止桌面操作，因此整合后没有再运行原生窗口验收；此前原生通过结果不代表该组合版本的完整桌面验收。

首次默认并发的 workspace 测试在编译阶段因 Windows 错误 1455（页面文件太小，无法 mmap `plugin-runtime` 产物）中断，并引发编译器退出。重试仅增加 `-j 2` 限制本次构建并发，沿用同一工作区及 `target/`，没有改变系统页面文件、全局 Cargo 配置或工具链。

原生命令：

```powershell
$env:NANOBUG_WINDOW_TOOLBAR_OUTPUT = 'C:/Users/Tmiracle/.codex/task-artifacts/rust-warning-20261030/native-retry'
cargo test -p editor-app --bin editor-app window_toolbar_native_windows_acceptance -- --ignored --exact tests::window_toolbar::native::window_toolbar_native_windows_acceptance --nocapture --test-threads=1
```

本机任务工作区为 `C:/Users/Tmiracle/.codex/worktrees/f154/Editor`，构建输出使用该工作区自己的 `target/`；`cargo metadata --no-deps --format-version 1` 已核对根目录和输出目录。证据集中于项目外 `C:/Users/Tmiracle/.codex/task-artifacts/rust-warning-20261030/`，包含 `red.log`、`controls.log`、`green.log`、`messages-integrated.log`、`workspace-test-integrated.log`、`workspace-check-integrated.log`、两轮原生日志及深浅主题截图。

返回[宿主消息规格](../specs/host-messages.md)与[原生 UI 目录](../README.md)。
