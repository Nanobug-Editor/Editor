# 插件日志分流与消息浮窗点击关闭

日期：2026-11-02。状态：实现、定向回归、Windows 原生验收与仓库检查通过；未发布。

按用户截图与本轮要求，插件自身日志仅保留在插件管理的“运行日志”；服务准备、LSP 生命周期／输出和显式通知继续进入宿主消息窗口。消息浮窗点击后关闭，并保留原有的历史展开与精确定位。此要求替代 2026-10-29 全量导入插件日志及 2026-10-30 点击后保留卡片的行为。

实现沿用现有日志消费边界，仅接纳宿主定义的 `lsp/`、`language.prepare:`、`dependencies/prepare` 服务来源和公开通知的 `host.message`／`host.notification`。过滤不删除、不读取原日志，也不倒退消费游标。未知内部来源默认留在插件管理；不按插件 ID、消息文案或等级特判。

本地通知控件在点击时关闭卡片，再执行宿主的历史定位。显式 `Notify` 同时以现有 `Dismissed` 结果正常结束请求；进度、取消、超时及实例撤销规则保持原样。关闭按钮不触发历史跳转。

任务工作区：`C:/Users/Tmiracle/.codex/worktrees/plugin-message-routing/Editor`，分支 `codex/plugin-message-routing`。使用自身 `target/`，未共享其他源码快照的编译输出。截图与日志集中在 `C:/Users/Tmiracle/.codex/task-evidence/plugin-message-routing-20261102/`。

## 回归与原生验收

- 修复前：`cargo test -p editor-app host_messages --locked --offline -j 8 -- --test-threads=1`，25 passed、2 failed；新增测试分别复现内部日志进入消息窗口，以及点击定位后浮窗仍存在。日志：`regression-before.log`。
- 修复后：`cargo test -p editor-app --bin editor-app host_messages --locked --offline -j 8 -- --test-threads=1`，28 passed、0 failed。覆盖真实插件管理日志 Tab、信息／警告／错误分流、工作区隔离、清空不回灌、日志已读不变，以及点击关闭、合并目标、清空／淘汰目标。补入可选服务依赖准备失败的来源后，同一组 28 项再次通过。日志：`regression-after.log`、`regression-final.log`。
- 实际插件：`cargo test -p editor-app --bin editor-app native_plugin_input_confirms_unicode_and_restores_editor_focus --locked --offline -j 8 -- --ignored --nocapture --test-threads=1`，1 passed。使用固定账本中的真实 `capability-example-0.19.0` ZIP；验证 `Notify` 正文点击移除卡片、以 `Dismissed` 完成，并保留消息来源与完整历史。原输入焦点、取消和进度用例继续通过。日志：`plugin-notify.log`。
- Windows 原生：设置 `NANOBUG_WINDOW_TOOLBAR_OUTPUT` 为外部证据目录下的 `native/`，执行 `cargo test -p editor-app --bin editor-app window_toolbar_native_windows_acceptance --locked --offline -j 8 -- --ignored --nocapture --test-threads=1`，1 passed。浅色 100%／深色 150% 的 Windows/D3D 窗口均验证点击卡片后可见通知数归零、消息面板打开；内部编译和 WASI 输出不生成卡片，LSP stderr 仍合并为一张警告。已查看两种主题的 `*-message-revealed.png` 及浅色 `*-message-preview.png`，确认卡片消失而目标完整四行正文仍可见。日志：`native.log`、`native/native-process.log`。
- `cargo fmt --check` 与 `git diff --check` 通过；五个变更文档的 375 个相对链接有效。
- `cargo test --workspace --exclude editor-app --locked --offline -j 8`：127 组、877 passed、0 failed、446 ignored。跳过项不计为通过；上面的两个实际包／原生用例另行显式执行。日志：`workspace-test.log`。
- `cargo check --workspace --locked --offline -j 8`：通过，保留仓库既有未使用代码等警告。日志：`workspace-check.log`。

未发布版本，未修改插件包或公开协议。
