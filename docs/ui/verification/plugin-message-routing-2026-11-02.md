# 插件日志分流与消息浮窗点击关闭

日期：2026-11-02。状态：用户后续澄清的服务日志范围已实现，定向回归、实际插件、Windows 原生验收及仓库检查通过；未发布。

按用户截图和后续澄清，插件自身日志、服务准备成功／失败、LSP 生命周期／通知／stderr／故障重试及可选依赖准备失败均只保留在底栏插件状态／异常摘要和插件管理的“运行日志”；只有公开 `Notify`／`PublishMessage` 显式发布的消息进入宿主消息窗口。消息浮窗点击后关闭，并保留原有的历史展开与精确定位。此要求替代当日首轮“服务日志继续进入消息窗口”、2026-10-29 全量导入插件日志及 2026-10-30 点击后保留卡片的行为。

实现沿用现有日志消费边界，仅接纳公开通知的 `host.message`／`host.notification`。`lsp/`、`language.prepare:`、`dependencies/prepare` 及其他日志来源均被排除，严重程度不例外放行。过滤不删除、不标记原日志已读，也不倒退消费游标；底栏继续展示加载状态和每插件最新异常摘要，完整日志保存所有等级。已移除无适用来源的 stderr 浮窗时间戳去重分支，显式消息按来源、等级和完整正文去重。

本地通知控件在点击时关闭卡片，再执行宿主的历史定位。显式 `Notify` 同时以现有 `Dismissed` 结果正常结束请求；进度、取消、超时及实例撤销规则保持原样。关闭按钮不触发历史跳转。

任务工作区：`C:/Users/Tmiracle/.codex/worktrees/plugin-message-routing/Editor`，分支 `codex/plugin-message-routing`。使用自身 `target/`，未共享其他源码快照的编译输出。截图与日志集中在 `C:/Users/Tmiracle/.codex/task-evidence/plugin-message-routing-20261102/`。

## 首轮回归与原生验收（服务日志范围已由后续澄清替代）

- 修复前：`cargo test -p editor-app host_messages --locked --offline -j 8 -- --test-threads=1`，25 passed、2 failed；新增测试分别复现内部日志进入消息窗口，以及点击定位后浮窗仍存在。日志：`regression-before.log`。
- 修复后：`cargo test -p editor-app --bin editor-app host_messages --locked --offline -j 8 -- --test-threads=1`，28 passed、0 failed。覆盖真实插件管理日志 Tab、信息／警告／错误分流、工作区隔离、清空不回灌、日志已读不变，以及点击关闭、合并目标、清空／淘汰目标。补入可选服务依赖准备失败的来源后，同一组 28 项再次通过。日志：`regression-after.log`、`regression-final.log`。
- 实际插件：`cargo test -p editor-app --bin editor-app native_plugin_input_confirms_unicode_and_restores_editor_focus --locked --offline -j 8 -- --ignored --nocapture --test-threads=1`，1 passed。使用固定账本中的真实 `capability-example-0.19.0` ZIP；验证 `Notify` 正文点击移除卡片、以 `Dismissed` 完成，并保留消息来源与完整历史。原输入焦点、取消和进度用例继续通过。日志：`plugin-notify.log`。
- Windows 原生：设置 `NANOBUG_WINDOW_TOOLBAR_OUTPUT` 为外部证据目录下的 `native/`，执行 `cargo test -p editor-app --bin editor-app window_toolbar_native_windows_acceptance --locked --offline -j 8 -- --ignored --nocapture --test-threads=1`，1 passed。浅色 100%／深色 150% 的 Windows/D3D 窗口均验证点击卡片后可见通知数归零、消息面板打开；内部编译和 WASI 输出不生成卡片，LSP stderr 仍合并为一张警告。已查看两种主题的 `*-message-revealed.png` 及浅色 `*-message-preview.png`，确认卡片消失而目标完整四行正文仍可见。日志：`native.log`、`native/native-process.log`。
- `cargo fmt --check` 与 `git diff --check` 通过；五个变更文档的 375 个相对链接有效。
- `cargo test --workspace --exclude editor-app --locked --offline -j 8`：127 组、877 passed、0 failed、446 ignored。跳过项不计为通过；上面的两个实际包／原生用例另行显式执行。日志：`workspace-test.log`。
- `cargo check --workspace --locked --offline -j 8`：通过，保留仓库既有未使用代码等警告。日志：`workspace-check.log`。

未发布版本，未修改插件包或公开协议。

## 服务日志范围澄清后的复验

- `cargo test -p editor-app --bin editor-app host_messages --locked --offline -j 8 -- --test-threads=1`：29 passed、0 failed。覆盖三类服务日志的信息／警告／错误全部不进入宿主历史、浮窗或提示点；底栏实际点击摘要可打开对应插件完整日志；查看宿主消息不改变插件日志、未读与提醒。显式发布消息的工作区隔离、一次消费、去重及点击关闭／定位继续通过。日志：`service-routing-regression.log`。
- `cargo test -p editor-app --bin editor-app native_plugin_input_confirms_unicode_and_restores_editor_focus --locked --offline -j 8 -- --ignored --nocapture --test-threads=1`：1 passed。固定真实 `capability-example-0.19.0` 包经过公开 Manager，普通信息级 `Notify` 仍进入消息历史，点击完成 `Dismissed` 并关闭卡片；输入、焦点、取消和进度继续通过。日志：`service-routing-plugin-notify.log`。
- 设置 `NANOBUG_WINDOW_TOOLBAR_OUTPUT` 为证据目录下的 `service-routing-native/`，执行 `cargo test -p editor-app --bin editor-app window_toolbar_native_windows_acceptance --locked --offline -j 8 -- --ignored --nocapture --test-threads=1`：1 passed。浅色 100%／深色 150% 的真实 Windows/D3D 窗口验证服务日志不生成宿主卡片或提示点，8 条原始插件日志完整保留。已查看两种主题的 `*-service-logs-excluded.png`，确认消息面板为空而底栏插件错误提醒仍在；`*-message-revealed.png` 确认点击宿主通知后卡片消失、完整四行目标正文可见。日志：`service-routing-native.log` 及 `service-routing-native/native-process.log`。
- `cargo test --workspace --exclude editor-app --locked --offline -j 8`：127 组、877 passed、0 failed、446 ignored；跳过项不计为通过。日志：`service-routing-workspace-test.log`。
- `cargo check --workspace --locked --offline -j 8`：通过，保留既有未使用代码等警告。日志：`service-routing-workspace-check.log`。
- `cargo fmt --check`、`git diff --check` 通过；五个变更文档的 375 个相对链接有效。

本轮复验未修改实际包、SDK 或公开协议，未执行其他 ignored 用例。首轮日志和截图原样保留，旧服务日志浮窗结果不作为当前行为证据。
