# 统一消息与右下角浮窗

日期：2026-10-29。状态：实现、定向回归与原生行为验收通过；未发布。

用户要求 Git 错误以及其他警告、日志统一交给宿主消息管理；警告和错误还需在编辑器右下角显示浮窗。此次要求替代旧消息规格对插件来源及自动浮窗的排除。

实现边界：沿用 `RuntimeLogs` 保存插件归属，主窗口观察既有扩展状态更新并按记录身份消费新日志。统一列表保持 500 条与每次 20 条展开；清空不删除插件原日志、不重置消费边界。宿主重要消息和插件日志的警告/错误使用本地非模态通知控件，自动浮窗有界并合并在场重复项。公开 `ui.interaction.Notify` 保留原请求的关闭、取消和超时语义，记录入历史只生成一个浮窗。

新增通用 `ui.interaction` 1.1 `PublishMessage`（既有同名权限）：标题最多 256 UTF-8 字节、正文最多 4096 UTF-8 字节、显式信息/警告/错误级别。宿主留痕后返回 `Published`，浮窗由宿主持有；无需长时间占用访客请求额度。1.0 的 `Notify` 不改变完成语义。SDK 注释和辅助函数为公开使用入口。

`PublishMessage` 与 `Notify` 在写入不可撤销历史前进入既有请求副作用边界：取消先赢时不写历史；接纳后取消为停止等待，不能报告“从未执行”，也不清除已产生历史。`Notify` 仍由原卡片关闭、取消或到期决定终态。

共享运行时按生产者克隆绑定规范化工作区，应用级记录无工作区绑定；当前窗口只导入自身工作区及应用级记录。停放实例、语言后台工作、延迟诊断继续使用原来源。旧插件管理页的全量日志和已读状态不变。

没有插件归属的管理器错误（例如运行时目录无法打开、工作区路由失败）在消费 worker 一次性状态时进入宿主错误历史和浮窗，不伪造插件标识。保持当前状态或清空历史不会重复发布；相同文案的新一次失败仍产生新历史条目。管理器原有详情与确认入口独立保留。

针对性回归入口：

```powershell
cargo test -p editor-app --bin editor-app unified_messages_ -- --nocapture
cargo test -p editor-app --bin editor-app host_messages -- --nocapture
cargo test -p plugin-runtime logs::tests -- --nocapture
cargo test -p plugin-runtime --lib unified_messages_ -- --nocapture
cargo test -p plugin-protocol -p plugin-runtime --lib publish_message_ -- --nocapture
```

原生回归覆盖真实扩展观察器、统一列表、浮窗位置与关闭、深浅主题、缩放、中文输入及焦点保持；另验证日志读取导致的 generation 变化不会重复导入，清空后新消息仍可到达。被忽略的实际包检查单独显式执行，不将 workspace 跳过项计为通过。

首轮结果：新增宿主回归先在旧实现执行得到 0 通过 / 2 失败（缺少警告卡和插件历史条目）；初版实现的 `unified_messages_` 得到 4 通过 / 0 失败，覆盖上述位置、关闭、主题、输入、隔离、限量及清空行为。此轮尚未包含随后新增的 `PublishMessage` 和长卡片用例，不代表新 API 已全部验收。

已完成的后续定向检查：协议序列化/文本边界 1/1；运行时 1.1 协商门禁 1/1；工作区来源、停放生产者及回灌边界 3/3；日志完整回归 13/13。初版 UI 测试产物的 `host_messages` 18/18、资源树外部定位提示 1/1；文件传输回归并行运行时曾有菜单标签查找失败（11/12），随后串行 `--test-threads=1` 为 12/12。最终新增长卡片、新 API 实际包和取消回归必须在最新产物重跑，不能用这轮旧产物结果替代。

最终审查补充的无归属管理器回归先在修复前执行，得到 0 通过 / 1 失败，断言为“ownerless manager failure must reach unified history”；修复后包含此用例的最新 `host_messages` 为 20/20，管理界面回归为 10/10。

最终其他相关回归：文件传输串行 12/12、外部定位提示 1/1；`native_plugin_input_confirms_unicode_and_restores_editor_focus` 使用真实 capability-example 0.19.0 包显式执行 ignored 测试，通过旧 Notify 单浮窗、取消、输入与焦点行为。最终 Git 0.1.51 包的 3 项原生测试覆盖 PublishMessage 的 Warning 归属、终态及零残留交互请求。宿主格式、workspace 测试（不含 editor-app，873 通过、446 ignored）和 workspace check 全部通过。

最终宿主与插件组合另经真实窗口操作验收：受控 Git 失败在统一消息历史与右下角各显示一次，浮窗长正文可滚动，关闭不删除历史，清空不回放；Git 表单未再绘制内联错误。隔离开发会话使用显式权限和专用配置，未改变用户安装。截图与最终包身份见 [Git 验收记录](../../plugins/verification/git-events-messages-2026-10-29.md)。

本机外部证据目录：`C:/Users/Tmiracle/.codex/task-artifacts/`；关键记录为 `git-events-messages-20261029/host-messages-red.log`、`git-events-messages-host-green.log`、`publish-message-tests.log`、`host-messages-regression.log`、`host-message-transfer-tests.log`、`host-message-transfer-serial.log`、`host-message-reveal-tests.log`。路径仅标记本次本机验收来源，不作为项目运行依赖。

返回[宿主消息规格](../specs/host-messages.md)与[原生 UI 目录](../README.md)。
