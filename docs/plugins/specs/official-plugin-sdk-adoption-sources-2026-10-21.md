# 官方插件 SDK 助手接入：源码调研依据

日期：2026-10-21。状态：只读源码调研完成；以下改造建议尚未实施。

本笔记区分市场已发布包、组织源码主干和本地候选。兼容静态检查由本批主任务执行；本笔记不运行 WASM、LSP、原生服务，不把源码阅读等同于功能验收。方案须继续满足[平台规格](plugin-api-platform.md)中的热安装、公开通用能力、实例资源归属及失败恢复要求；本次助手的边界见[开发工具方案](plugin-developer-tooling.md)。

## 1. 官方范围及固定输入

市场仓库主干固定为 `cd83b2f94814f6463f4104e5eaa6890f1132098a`，提交时间为 `2026-10-14T16:03:41Z`。其 [registry](https://github.com/Nanobug-Editor/Plugin-Marketplace/tree/cd83b2f94814f6463f4104e5eaa6890f1132098a/registry) 包含七项；同日读取的 [Pages catalog](https://nanobug-editor.github.io/Plugin-Marketplace/catalog.json) 也是七项、八个版本（Markdown 两版）。该 catalog 的字节摘要由主任务保存在验证记录中；动态 URL 不代表不可变的未来输入。官方维护方式和不随宿主附带发行的范围见[固定 README](https://github.com/Nanobug-Editor/Plugin-Marketplace/blob/cd83b2f94814f6463f4104e5eaa6890f1132098a/README.md)。

源码阅读以市场版本的完整 commit 为基线，并以 `git ls-remote` 得到的当前主干 commit 复核。除 Markdown 外，下列六项的已发布版本到主干 `src/` 内容相同；HTML、JavaScript、TOML 没有访客 Rust `src/` 目录。该比较限于所列源文件，不表示整个仓库或 ZIP 字节相同。

| 插件 | 市场已发布版本 / 固定源码 | 调研时主干 / 清单版本 |
| --- | --- | --- |
| HTML | [0.3.1 / 68b8c7baabd0790cba884aa19e4f2523b2b2e7ce](https://github.com/Nanobug-Editor/HTML/tree/68b8c7baabd0790cba884aa19e4f2523b2b2e7ce) | [d114a44f4bc9677e0bfaf0f06d99aacf1a0f7ced / 0.3.1](https://github.com/Nanobug-Editor/HTML/tree/d114a44f4bc9677e0bfaf0f06d99aacf1a0f7ced) |
| JavaScript | [0.3.1 / 18dc8818787857b75cb1ae1b4ffd88edfa35a631](https://github.com/Nanobug-Editor/JavaScript/tree/18dc8818787857b75cb1ae1b4ffd88edfa35a631) | [f8a309bb7b855b11baa60a4ce9aaaef38703b4d1 / 0.3.1](https://github.com/Nanobug-Editor/JavaScript/tree/f8a309bb7b855b11baa60a4ce9aaaef38703b4d1) |
| Markdown | [0.17.2 / 8480eb38e059d575cb5dafd740ed14f035d20936](https://github.com/Nanobug-Editor/Markdown/tree/8480eb38e059d575cb5dafd740ed14f035d20936)；[0.17.1 / 31516790679c2ab591105d11e6fd15e974b72505](https://github.com/Nanobug-Editor/Markdown/tree/31516790679c2ab591105d11e6fd15e974b72505) | [057d8bfe11b5caa9f93c6ed0c3be029594dee427 / 0.18.2](https://github.com/Nanobug-Editor/Markdown/tree/057d8bfe11b5caa9f93c6ed0c3be029594dee427) |
| Rust | [0.6.1 / 942a905e4d4dfe02a9b9b162c35e3c2788691d5f](https://github.com/Nanobug-Editor/Rust/tree/942a905e4d4dfe02a9b9b162c35e3c2788691d5f) | [2a8059c02011238ceac56f7d3669c2decd326c9d / 0.6.1](https://github.com/Nanobug-Editor/Rust/tree/2a8059c02011238ceac56f7d3669c2decd326c9d) |
| Image（安装 ID `svg`） | [0.6.1 / cac09895b6bc9cd227a6e318edb8c868da41084e](https://github.com/Nanobug-Editor/Image/tree/cac09895b6bc9cd227a6e318edb8c868da41084e) | [dd37662339e9eef9fc2aebb36a80e31b9cde62bf / 0.6.1](https://github.com/Nanobug-Editor/Image/tree/dd37662339e9eef9fc2aebb36a80e31b9cde62bf) |
| TOML | [0.2.3 / a1a4677118403ac01e6a0fd2f667cfa788aa9bd2](https://github.com/Nanobug-Editor/TOML/tree/a1a4677118403ac01e6a0fd2f667cfa788aa9bd2) | [08b2a5a864b160cb574309b3931e6384e86d9088 / 0.2.3](https://github.com/Nanobug-Editor/TOML/tree/08b2a5a864b160cb574309b3931e6384e86d9088) |
| XML | [0.2.2 / 69749e0d37ca28571e39101678f77a051a939ca8](https://github.com/Nanobug-Editor/XML/tree/69749e0d37ca28571e39101678f77a051a939ca8) | [98c912b891f5a6c496d1081b01c80164861d6cf6 / 0.2.2](https://github.com/Nanobug-Editor/XML/tree/98c912b891f5a6c496d1081b01c80164861d6cf6) |

补充检查以下维护对象，但不将它们算成当前市场已发布七项：

| 对象 | 身份及限制 |
| --- | --- |
| TypeScript 0.1.15 | 组织仓库主干和清洁本地 HEAD 均为 [2bc5be0301fc485d3b684641104e4dd596d3456e](https://github.com/Nanobug-Editor/TypeScript/tree/2bc5be0301fc485d3b684641104e4dd596d3456e)。调研时 [Releases](https://github.com/Nanobug-Editor/TypeScript/releases) 为空，registry 没有该项；[README](https://github.com/Nanobug-Editor/TypeScript/blob/2bc5be0301fc485d3b684641104e4dd596d3456e/README.md) 也明确功能支持不代表 Release 或市场发布。 |
| Vue 0.1.12 | 组织仓库主干和清洁本地 HEAD 均为 [173c265e4c19298ca2fdb86b849ae90b6e3e4043](https://github.com/Nanobug-Editor/Vue/tree/173c265e4c19298ca2fdb86b849ae90b6e3e4043)。调研时 [Releases](https://github.com/Nanobug-Editor/Vue/releases) 为空，registry 没有该项。 |
| 本地 Git 0.1.23 | [本地 README](../../../../Git-for-Nanobug/README.md) 明确为未上架的开发候选；此目录当前没有 `.git`，无法确认 HEAD、remote、主干或脏改动。只能按实际源码文件和候选 ZIP 摘要记录，不能给它编造组织仓库或已发布源码 commit。 |
| 本地 Markdown 0.18.2 候选 | 本地 HEAD 为 `985ea81ed762ea7a62f371ec4dfdf273cc9f0fe4`，有清单、源码及文档等未提交修改；它与组织主干、市场版本和本地生成 ZIP 是不同输入。改造前必须重新固定选用的候选源码及包摘要。 |

源码归档仅读取固定 commit 的 GitHub codeload 内容，缓存于 `target/official-plugin-sdk-research-20261021/`，没有执行归档内代码。协议测试 fixtures 和已退役终端插件不计入官方插件清单；官方/本地对象的归档规则见[仓库插件入口](../README.md)。

## 2. 三个助手的实际边界

- [ResourceGroup](../../../crates/plugin-protocol/src/api/guest/resources.rs) 只管理通过自身 `timer()`、`documents()` 创建的独立定时器和文档订阅；没有接管任意句柄的入口。`observe()` 只处理所属资源的终结通知；`close_all()` 返回清理结果，失败项仍保留。不能拿它包 LSP、原生进程、服务引用、树、虚拟文档或 `PreferenceBinding` 的订阅。
- [EditorIntent](../../../crates/plugin-protocol/src/api/guest/intent.rs) 表示一个可被替换的当前编辑意图。`replace()` 撤销旧结果接收资格，尝试取消并清理旧请求后才启动新请求。它不替代文档/revision 校验、领域状态机或副作用账本；取消和关闭不会回滚已执行的编辑、打开文档或外部操作。
- [HttpTask](../../../crates/plugin-protocol/src/http/task.rs) 管理显式 HTTP 流及其轮询 timer，需要 `network.http`、`lifecycle.timers` 协商。它保留原始 chunk 和非 2xx 状态，清理失败和传输结果分开；不是 LSP/Node 网络访问、浏览器链接打开或宿主资源图片加载的替代接口。

三个助手的 `Drop` 均不调用宿主；失败的清理 owner 不能因为字段重置而丢弃。当前 [Input](../../../crates/plugin-protocol/src/api.rs) 是 Prepare/Activate/Event/Snapshot，没有供本方案直接调用的 Deactivate 消息；停用/移除的最终兜底仍是宿主实例撤销。不能在改造方案里虚构一个现有停用回调。

## 3. 逐插件接入判断

| 插件 | 源码事实及判断 | 本批建议 |
| --- | --- | --- |
| Markdown | [formatting.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/formatting.rs)、[navigation.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/navigation.rs) 各自保存一个 `EditorTask`，手写取消/替换/通知匹配；[scrolling.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/scrolling.rs) 是一个在途请求加一个 latest 位置。 | 优先接入 EditorIntent 到格式/任务编辑和显式导航；滚动单列后续小步，保留合并策略。 |
| 本地 Git | [lib.rs:269](../../../../Git-for-Nanobug/src/lib.rs#L269) 创建重复 timer，随后订阅文档事件，两者返回句柄均未保存。事件处理见 [events.rs](../../../../Git-for-Nanobug/src/events.rs)。 | ResourceGroup 最直接的候选。保存句柄供精确匹配，明确部分启动失败和状态重置时的清理。 |
| TypeScript | [lib.rs](https://github.com/Nanobug-Editor/TypeScript/blob/2bc5be0301fc485d3b684641104e4dd596d3456e/src/lib.rs) 按每个 service invocation 保存 pending；[selection.rs](https://github.com/Nanobug-Editor/TypeScript/blob/2bc5be0301fc485d3b684641104e4dd596d3456e/src/selection.rs) 最多保留 32 个独立项目选择/准备请求；[contributions.rs](https://github.com/Nanobug-Editor/TypeScript/blob/2bc5be0301fc485d3b684641104e4dd596d3456e/src/contributions.rs) 同时涉及 EditorTask、服务 Task 和组合阶段。 | 先处理本次发现的清单/宿主契约不匹配；暂不批量换 EditorIntent，更不能用一个全局 intent 取消其他调用者。 |
| Vue | [lib.rs](https://github.com/Nanobug-Editor/Vue/blob/173c265e4c19298ca2fdb86b849ae90b6e3e4043/src/lib.rs) 的 Pending 按调用者保存服务 Task、引用和多阶段组合；取消同时释放服务引用。 | 保留现有 broker 生命周期；当前助手不覆盖这些资源。单包静态通过不能证明缺失/不兼容 TypeScript 时业务可用。 |
| Image | [display.rs](https://github.com/Nanobug-Editor/Image/blob/cac09895b6bc9cd227a6e318edb8c868da41084e/src/display.rs) 使用 PreferenceBinding；[lib.rs](https://github.com/Nanobug-Editor/Image/blob/cac09895b6bc9cd227a6e318edb8c868da41084e/src/lib.rs) 接收宿主托管预览及偏好事件。 | 保留现有专用绑定；没有明确的 ResourceGroup/EditorIntent/HttpTask 迁移收益。 |
| Rust | [lib.rs](https://github.com/Nanobug-Editor/Rust/blob/942a905e4d4dfe02a9b9b162c35e3c2788691d5f/src/lib.rs)、[cargo/targets.rs](https://github.com/Nanobug-Editor/Rust/blob/942a905e4d4dfe02a9b9b162c35e3c2788691d5f/src/cargo/targets.rs)、[debugger/session.rs](https://github.com/Nanobug-Editor/Rust/blob/942a905e4d4dfe02a9b9b162c35e3c2788691d5f/src/debugger/session.rs) 主要管理 Cargo/调试进程、延迟服务回复和会话，而非上述独立 timer/订阅或单当前编辑请求。 | 本批不改。进程与调试停止的领域语义不能委托 ResourceGroup。 |
| XML | [lib.rs](https://github.com/Nanobug-Editor/XML/blob/69749e0d37ca28571e39101678f77a051a939ca8/src/lib.rs) 直接生成配置、补全、结构结果；LemMinX 传输由宿主管理。 | 本批不改，不为同步钩子新增异步 owner。 |
| HTML / JavaScript / TOML | 各自固定版本 [HTML manifest](https://github.com/Nanobug-Editor/HTML/blob/68b8c7baabd0790cba884aa19e4f2523b2b2e7ce/manifest.json)、[JavaScript manifest](https://github.com/Nanobug-Editor/JavaScript/blob/18dc8818787857b75cb1ae1b4ffd88edfa35a631/manifest.json)、[TOML manifest](https://github.com/Nanobug-Editor/TOML/blob/a1a4677118403ac01e6a0fd2f667cfa788aa9bd2/manifest.json) 都未声明访客 component，服务是声明式贡献。 | 保留资源包；不为调用助手附加空 WASM。 |

截至所读源码，七项市场插件及 TS/Vue 两项主干中未发现显式 SDK HTTP stream 创建/拉取，也未发现可直接迁移的独立文档订阅/timer owner。补充本地 Git 是后两者的例外。因此本批不安排任何 HttpTask 迁移，也不新增网络权限。Markdown 的外部链接导航及图片呈现不构成改用 HttpTask 的理由；结论来自上述固定源码、清单与 SDK 能力边界，不代表所有原生依赖都不访问网络。

## 4. 前置兼容阻断：TypeScript 候选

本批静态检查的两个 TypeScript 0.1.15 候选均在 `manifest.json:480:21` 被拒绝；具体摘要、命令和宿主身份见主任务生成的兼容验证记录。源码 [manifest.json](https://github.com/Nanobug-Editor/TypeScript/blob/2bc5be0301fc485d3b684641104e4dd596d3456e/manifest.json#L474) 为 `language_servers[0].delegate` 声明了 `interactive: true`，而本次宿主公开 [language::Delegate](../../../crates/plugin-protocol/src/language.rs) 只有 `contract`、`method`，并使用 `deny_unknown_fields`。

这是候选包与当前宿主契约不匹配，不能归因于未接入助手，也不能以旧验收记录替代本次失败。候选 [README](https://github.com/Nanobug-Editor/TypeScript/blob/2bc5be0301fc485d3b684641104e4dd596d3456e/README.md) 把交互式项目选择列为现有功能，故不能直接删除 `interactive` 或放宽反序列化来制造“通过”。应先追溯该功能的配套公开契约和目标宿主：完成通用能力的规格/版本/实现与准入测试，或选择确实匹配的受测宿主/候选组合；保留项目歧义选择、取消不绑定、权限和非交互降级语义，再重跑两包与 TS/Vue 组合验证。此处仅提出阻断及处理顺序，不自动授权宿主协议变更。

## 5. 可以直接进入实施工单的改造切片

### A. 本地 Git：只接管定时器和文档订阅

目标入口：[State / Activate](../../../../Git-for-Nanobug/src/lib.rs)、[State::event](../../../../Git-for-Nanobug/src/events.rs)。

1. 在 State 保留一个 ResourceGroup，以及 refresh timer / document subscription 的匹配身份。由 group 创建两项资源，不重建或猜测资源句柄；已有清单已声明 `lifecycle.timers` 和 `editor.documents`，无需扩大权限。
2. 文档订阅创建失败时清理已创建 timer；保留原始业务错误及清理失败项。重置旧 State 前显式检查 `close_all()`，失败 owner 保留以便重试，避免 `*state = Default::default()` 丢弃它。宿主实例撤销仍承担异常退出及停用的最终回收。
3. 每个 Event 先 `observe()`，再按保存的准确句柄消费 Timer/DocumentEvent；匹配 SubscriptionFailed 时清除对应本地身份并报告，不无限自动重订阅。
4. 保留 30 秒刷新间隔、用户活动退让、显式保存/刷新优先、只在可见面板触发后台业务的策略。隐藏面板不等同于停用：不能清掉仍需用于已打开比较视图的文档关闭观察。
5. 验证 timer 成功/订阅失败、部分清理失败后重试、重复启动不累积资源、无关 timer/订阅事件无效、文档关闭正确回收比较引用；沿用 [scene_receipt_tests.rs](../../../../Git-for-Nanobug/src/scene_receipt_tests.rs) 及 [workbench_quiet_tests.rs](../../../../Git-for-Nanobug/src/repository/workbench_quiet_tests.rs) 的可观察行为。真实包停用/更新后检查宿主资源归零，不只检查 group.len()。

保留 [processes.rs](../../../../Git-for-Nanobug/src/processes.rs) 的进程所有权、[comparisons/lifecycle.rs](../../../../Git-for-Nanobug/src/comparisons/lifecycle.rs) 的临时源到原生视图的所有权转移，以及 Git 写操作的预览/确认。这些不能放进 ResourceGroup；本次也不把通用 `State::editor()` 的多请求路由替换为一个全局 EditorIntent。

### B. Markdown：格式编辑和显式导航各一个 EditorIntent

目标入口：`Formatting::{start_intent,request,complete,cancel_pending}`、`Navigation::{begin,request,retire_task}`；固定来源见第 3 节。调研时组织主干 0.18.2 的这两个文件及 scrolling/imports 与市场 0.17.2 相同，可以将同一切片应用到确认后的维护基线；这不表示本地脏工作区也与主干完全一致。

1. Formatting 与 Navigation 各自保留 EditorIntent，把 Pending 中的 document/kind/phase 等保留为领域元数据；两个 owner 独立，不能互相隐式取消。由 `replace()` 启动/替换，`update()` 筛选当前回执；保存原有 30 秒 deadline 及 TryTerminate 选择。
2. 读选区 → 计划 → 一次 ReplaceDocumentRange 的两阶段流程不变。继续检查 document/revision、UTF-8 选区、expected_selection 和最终选择；源码只来自宿主 Preview，仍只有一次宿主文档事务及一组 Undo。
3. 导航保留 Waiting、Opening、fragment、viewport driver 的领域状态，继续支持回执先于目标 Preview 的顺序。外部浏览器/文件已经打开不能声称被撤销；旧回执只失去对当前界面的推进资格。
4. source_changed/reset/prepare 等已有边界显式取消并 `close()`；检查 IntentFailure.cleanup，清理未完成时保留 helper、阻止新请求，不能只丢弃 owner。通知分发继续让各 owner 独立匹配，不能用短路表达式使另一个 owner 丢失回执。
5. 测试快速 A→B、A 完成但通知尚未交付、切文档/revision、读阶段成功后选区变化、写操作已提交后取消、清理失败后重试、导航等待 Preview 期间替换及中文路径。业务断言包括一次编辑/Undo、旧意图不覆盖当前反馈，以及原有标题定位/重排不回跳。

### C. Markdown 滚动：在 B 之后独立评估

`Scrolling::queue/request/cancel_pending` 可以让 EditorIntent 承担唯一在途 LocateViewport 请求的所有权，但必须保留 `pending + latest` 合并、origin、document/ui_revision、driver/takeover 和 2 秒 deadline。普通滚轮通知仍只更新 latest，不能每帧 replace/cancel 发出新请求；取消模式仍为 StopWaiting，已应用的原生 offset 不回滚。复用滚动领域测试，并对同一输入序列比较请求数和布局结果；助手接入本身不构成性能提升的证据。[固定 scrolling.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/scrolling.rs)

### D. 明确保留的边界

- Markdown [imports.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/imports.rs) 的 `active + observing` 是有意的文件副作用账本。被替换或切换文档的 SaveImageInput 仍可能创建文件，必须保留 EditorTask 接收文件名和结果；不能以 EditorIntent 的“旧结果丢弃”或 StopWaiting 掩盖已保存文件。输入释放、至多八张/32 MiB 约束、文件名冲突处理和最终引用插入均保留。
- TS/Vue 按调用者并发、服务引用/模块引用的归属，以及宿主托管的语言进程均保留。先完成兼容阻断处理；若将来只迁移某个明确“新选择替换旧选择”的 UI 分支，需单独证明它不取消其他文档/调用者，也不把交互取消解释成撤销已发布绑定。
- 暂不改 Image/XML/Rust/HTML/JavaScript/TOML，不引入空组件或无用途的权限。未来确有显式 HTTP 流需求时，才在对应功能中接入 HttpTask 并单独验证 chunk 边界、非 2xx、超时、取消和部分启动清理；本次没有这样的迁移工单。

## 6. 交付约束与未覆盖部分

改造实施时使用包含本次助手的宿主 `--plugin-cargo`，固定实际宿主/SDK 摘要；不依赖主仓库源码路径，也不把新助手 API 可编译等同于最低宿主能力声明正确。每个修改的插件提升实际清单版本；包版本、协议版本和能力版本分别处理。仅改 SDK 使用方式不自动要求提升 wire 协议或新增权限。[仓库约定](../../../AGENTS.md)、[开发工具方案](plugin-developer-tooling.md)

每个切片完成后先运行现有插件测试，再用新包执行 `--plugin-check` 与隔离 `--plugin-dev --report`，完成实际业务操作后检查停止/清理。用当前已通过静态准入的旧包作兼容对照；热更新保持同一宿主进程和未保存文档，验证失败候选保留旧版本、成功后旧 owner 回收。发布/市场提交不是本次调研授权的一部分。

本笔记未证明任何新助手改造已完成，没有运行原生服务或验证网络/调试业务；没有对候选 ZIP 与本地脏源码做可复现构建对应证明。静态包通过仅说明被测宿主的包准入和能力声明通过，不能代替 WASM 实例化、语言服务启动、TS/Vue 组合可用性或长期运行清理验收。
