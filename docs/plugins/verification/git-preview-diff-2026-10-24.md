# 提交文件仅差异预览与行尾比较

2026-10-24。独立 Git 源码：`C:/Projects/RustProjects/Git-for-Nanobug`。

## 复现与变更

- 实际 0.1.38 包通过公开管理器打开工作区差异时，原生回归 `git_commit_file_diff_opens_only_comparison_preview` 失败：创建了额外可见的源文件标签。日志 `target/git-preview-diff-2026-10-24/source-tab-red.log`。
- `mixed_line_endings_do_not_turn_unchanged_context_into_changes` 在纯 LF/CRLF 内容完全相同时失败；日志 `eol-red.log`。旧算法直接比较含分隔符的行字符串。
- 新增实际包版本核对用例后，`git_commit_comparisons_use_exact_index_and_worktree_versions` 复现了右侧仍显示上次 `INDEX version`、磁盘已是 `WORKTREE version` 的问题；日志 `native-source-tabs.log`。复用干净原生会话前通过既有文件协调流程读取磁盘，并抑制更早的文件监听结果；已脏或尚未发出 Change 的原生输入保持原文。首轮新功能回归还发现 `diff-open` 准备遗漏释放，已纳入相同完成路径，随后复测。
- 核对精确 revision 时发现，静默磁盘／虚拟源刷新已递增 revision，但未同步原生内容身份；延迟 Notify 会再递增一次，使返回的版本立即过期。两条刷新路径均在返回元数据前封存同一原生值的身份，保留后续真实编辑失效规则。
- 版本核对还复现了 `diff-index` 命令在上一比较收尾期间被 `ui_busy` 静默忽略，界面保留旧 worktree 比较。两种只读比较命令改用文件树同一捕获队列，保留最后的文件和暂存模式，在当前 Git／原生请求结束后启动，不取消正在进行的写入。纯队列回归及实际包断言共同覆盖。
- 通用 `editor.documents 1.2` 新增 `OpenDocumentSource` 和 `ReleaseDocumentSource`；沿用原生 DocumentSession，不建第二份可变文本。加载不导航；已打开标签保留。准备有 32 个隐藏本地源上限、60 秒未挂载宽限和实例撤销清理，视图保留有效源。最后一个比较关闭后回收干净源、显示未保存源。
- Git 0.1.39 改用上述公开机制，并在比较成功／失败时释放准备。历史快照仍使用现有 `OpenVirtualSource`。比较键只归一化 LF/CRLF，区间仍以原始 UTF-8 字节计数，末行缺失换行仍是差异。对齐器原有锚点已忽略行尾分隔符，不重复修改其模型。

## 验证进度

- `cargo test -p editor-app editor::comparison::diff::tests --jobs 2 -- --nocapture`：3 passed，包含上述红灯的修复后复测；日志 `eol-green.log`。
- 配套宿主 `cargo rustc -p editor-app --release --jobs 2 -- -o .../Nanobug.exe`：通过，日志 `release-build.log`。
- 配套宿主 `--plugin-cargo ./Cargo.toml test --lib`：最终 185 passed，日志 `guest-queued-tests.log`。
- `cargo test --workspace --exclude editor-app --jobs 2`：551 passed、401 ignored；`cargo check --workspace --jobs 2` 通过。日志 `workspace-tests.log`、`workspace-check.log`，跳过项不算已验收。
- `editor::comparison::` 针对性检查 12 passed，包含行尾、换行对齐、行背景、共享滚动条和三源导航。实际包兼容回归 `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs` 1 passed，覆盖已打开源、原生中文编辑、折叠和导航；日志 `comparison-regressions.log`、`compatibility.log`。
- 完整工作台原生 smoke 在进入差异流程前的 `incoming selection filtered native history` 等待失败，记录在 `native-smoke/dark-zh-CN-100/native.log`，未计为通过。新增独立的 comparison-only 验证入口，明确拒绝与其他缩短模式混用，且完整 smoke／matrix 拒绝这个模式；单独校验预览标签、原生输入、滚动和保存，不能代替完整工作台验收。
- 首次 `cargo test -p editor-app --no-run` 被已有示例 `svg_preview_render` 未处理 `Paint::TextBox` 阻断；主程序 `cargo test -p editor-app --bin editor-app --no-run --jobs 2` 通过。未将示例失败记为通过。
- 最终实际包 `source_tabs::` 两项均 passed：HEAD／index／worktree 原生内容准确；没有额外源标签；替换比较回收干净源；外部磁盘变化不能覆盖原生中文草稿；关闭脏比较保留同一会话。日志 `native-queued-versions.log`。测试读取当前实际左侧焦点会话，避免误读已保留的其他只读源。
- 独立 Windows `git_native_comparison_probe` 通过：dark-zh-CN-100 与 light-en-150 两组均通过原生标签、组合输入、Undo、独立滚动、保存及窗口退出验证。日志 `native-comparison.log`；分组收据与截图位于 `native-comparison/<case>/`。夹具已显式打开的文件标签保留，前后可见文件 ID 均为 `[1]`；新文件不产生源标签由上述实际包回归单独验证。组合接口已验证，系统 IME 候选窗口仍按收据标记为未验证。
- 本次涉及的 Rust 文件 `rustfmt --check --config skip_children=true ...`、独立插件 `cargo fmt --check` 和 `git diff --check` 通过。最后全仓 `cargo fmt --check` 仍被其他并行改动的 `crates/editor-app/src/extensions/dock_tests/drag/live.rs` 与 `drag/live/exclusive.rs` 阻断；日志 `fmt-last.log`，未顺带格式化这些改动。

## 交付

以下两项需配套使用，Git 包声明 `editor.documents >=1.2,<2`；没有增加安装权限。尚未提交、推送、发布或替换用户当前安装。

| 文件（仓库根目录相对路径） | SHA-256 |
| --- | --- |
| `target/git-preview-diff-2026-10-24/final/Nanobug.exe` | `94d014e153124a7c2c364ed986ea0a72e534d412daeb66178d3c666e1a1efd46` |
| `target/git-preview-diff-2026-10-24/final/git-0.1.39.zip` | `96e90fec708305c430e3af1b5c6b380169747ea773bad45580d043d3b8939704` |

身份读回见同目录证据根的 `artifact-identity.json`。以上结论限定为本轮提交文件预览与比较修复，不将失败的完整工作台 smoke、示例编译或全仓格式检查算作通过。
