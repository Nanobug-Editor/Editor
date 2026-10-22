# 官方插件 SDK 生命周期助手接入方案

日期：2026-10-21。状态：**方案已获确认，进入三会话并行实施**。用户已明确要求分别处理 TypeScript 兼容问题、Git 与 Markdown 的 SDK 助手改造；具体会话和验证状态见[实施调度记录](../verification/official-plugin-sdk-adoption-execution-2026-10-21.md)。实施授权不等于各项已完成验收。

依据：[已实现的三个助手](plugin-developer-tooling.md)、[官方源码调研](official-plugin-sdk-adoption-sources-2026-10-21.md)、[本批兼容核验](../verification/official-plugin-compatibility-2026-10-21.md)及[平台规格](plugin-api-platform.md)。本文的切片编号不是 GitHub issue；当前用户指令已授权实施及三会话并行，不自动授权提交、推送或发布。

## 1. 检查结论与适用范围

当前[官方市场目录](https://nanobug-editor.github.io/Plugin-Marketplace/catalog.json)收录 HTML、JavaScript、Markdown、Rust、Image（安装 ID svg）、TOML、XML 七项。八个已收录版本全部通过本次固定宿主的生产静态准入，归档 SHA-256 全部与目录一致。

补充检查未上架的维护对象：Git 0.1.23、Markdown 0.18.2、Vue 0.1.12 两个候选均通过；TypeScript 0.1.15 的两个候选均在 manifest.json 第 480 行第 21 列被拒绝。其 delegate.interactive 不在本次宿主的 Delegate 契约内。全部原始身份和结果见[账本](../verification/official-plugin-compatibility-2026-10-21.json)。

这批只执行静态检查，所有报告 executed=false；未安装、执行插件或验证 LSP/调试/原生模块业务。Vue 单包通过也不证明其与被拒绝的 TypeScript 候选组合可用。以下建议基于真实源码中的资源拥有关系，不以“所有插件使用相同 SDK”作为全面重写理由。

| 对象 | 接入判断 | 排期 |
| --- | --- | --- |
| 本地 Git 0.1.23 | ResourceGroup 直接替代裸 timer + documents subscription 的管理 | 首批，切片 A1 |
| Markdown 0.17.2 / 0.18.2 维护线 | Formatting、Navigation 各用一个 EditorIntent；保留各自领域状态 | 首批，切片 A2 |
| Markdown 滚动同步 | EditorIntent 可管理唯一在途请求，但必须保留 latest 合并和滚动驱动策略 | 第二批，切片 A3，依赖 A2 |
| TypeScript 0.1.15 | 先消除配套宿主不匹配；并发项目/文档调用不适合一个全局 EditorIntent | 兼容切片 A0；暂不改助手 |
| Vue 0.1.12 | 服务调用、服务引用和分阶段 broker 不在当前三个助手的拥有范围 | 保持；A0 后验证 TS/Vue 组合 |
| Rust / Image / XML | 当前主要使用进程会话、PreferenceBinding、同步钩子或宿主托管能力 | 暂不改 |
| HTML / JavaScript / TOML | 声明式包，无需增加访客组件 | 暂不改 |
| 全部上述对象的 HTTP | 未找到需要迁移的访客 HTTP stream；宿主图片加载、依赖下载和外部打开不属于 HttpTask | 本轮无 HttpTask 工单 |

逐项源码证据和固定 commit 见[调研第 1、3 节](official-plugin-sdk-adoption-sources-2026-10-21.md)。

## 2. 改造接缝与共同约束

接缝放在已有功能状态的拥有者：Git State、Markdown Formatting/Navigation/Scrolling。助手作为深模块隐藏通用的关联、取消和资源释放；文档 revision、读写阶段、导航等待、滚动合并等领域状态仍留在插件。直接替换已有手写管理，不叠加只转发调用的新 crate 或管理层。

- ResourceGroup 仅创建并拥有独立 timer、documents subscription；不接管任意旧句柄、树、进程、服务引用、虚拟文档或偏好 watch。
- EditorIntent 表示一个可以替代的当前编辑意图；Formatting 与 Navigation 分别持有，不用一个全局实例串行化所有功能。
- HttpTask 内部拥有自己的 stream/timer；不得将其内部资源重复登记给 ResourceGroup。
- 所有清理都检查 CleanupReport。部分失败保留助手，允许有界显式重试；不能重置字段后把失败 owner 丢掉，也不能无限重试。
- Drop 不调用宿主。现有 Input 没有 Deactivate；只在实际已有的功能结束、源文档失效和重置路径显式清理，异常、停用、更新和移除的最终回收由宿主实例退役兜底，不虚构停用通知。
- 本批助手接入继续使用协议 7、已声明能力及现有权限。宿主提供通用接口，不引入插件 ID 特例，不修改宿主安装流程或强制重启。
- 不迁移活句柄进 Snapshot；快照仍只保存原有逻辑状态。

## 3. A0：固定 TypeScript 的目标宿主配套

**触发依据：** [两份实际失败报告](../verification/official-plugin-compatibility-2026-10-21.md)均止于清单解析。这是候选契约与固定目标不匹配，不能归因于没有使用 SDK 助手。A0 阻断 TypeScript 发布和 TS/Vue 组合验收，不阻断独立的 Git/Markdown 改造。

实施步骤：

1. 固定 TypeScript 0.1.15 两个 ZIP 的完整摘要和构建来源，追溯 interactive 项目选择功能对应的宿主源码、SDK 摘要及已批准语言规格；检查该宿主是否已经具备要求的公开契约。
2. 选择实际匹配的目标组合并重新导出/核对 SDK。若当前正式目标确实缺少通用交互能力，应另行确认并实现其契约、能力版本和宿主行为，不能当作助手改造的隐含内容。
3. 用未改写的原 ZIP 先验证正确配套；需要重建时，通过确认的宿主 --plugin-cargo/--plugin-package 直接构建，并使用新插件包版本，保留旧失败归档作为对照。
4. 先做清单/能力准入，再做真实组件启动、项目歧义选择、取消后不发布绑定、非交互降级及 TS/Vue 联合场景。

验收：两个候选按明确目标分别检查，报告身份可复现；匹配目标通过，不匹配目标明确拒绝。不能删除 interactive、放宽 deny_unknown_fields 或沿用历史通过记录来掩盖问题。清单解析通过后的其它能力及业务仍需验证。

来源：[TypeScript 固定源码](https://github.com/Nanobug-Editor/TypeScript/tree/2bc5be0301fc485d3b684641104e4dd596d3456e)、[Delegate](../../../crates/plugin-protocol/src/language.rs)、[来源笔记第 4 节](official-plugin-sdk-adoption-sources-2026-10-21.md)。

## 4. A1：Git 的独立资源归组

**修改位置：** [State / Activate](../../../../Git-for-Nanobug/src/lib.rs)、[State::event](../../../../Git-for-Nanobug/src/events.rs)。当前 Activate 启动 30 秒重复 timer 并订阅文档事件，返回句柄没有保存在 State；Timer 分支匹配任意 timer。

建议变更：

1. State 新增 ResourceGroup，保存 refresh timer 与 document subscription 的匹配身份。身份只用于识别事件，清理归 group 唯一负责。
2. Activate 用 group.timer(30_000, true) 和 group.documents() 创建资源。文档订阅失败时关闭已创建的 timer，同时保留主错误及清理失败 owner。重复启动不得再增加一组资源。
3. 每个 Event 先 group.observe，再仅处理本功能的匹配通知。SubscriptionFailed 更新本地身份及失败状态，不无界重订阅。已有合成事件测试同步改为实际匹配身份，不取消行为断言。
4. 在已存在的正常状态重置路径，先明确结束旧资源；清理失败不得通过覆盖 State 丢失。宿主停用/更新时继续走实例退役。
5. 保持 30 秒间隔、用户活动退让、显式保存/刷新优先和可见性策略。隐藏面板仍可能需要文档关闭通知来释放比较源，不能简单把隐藏等同于整个 group 关闭。

**保留：** Git 进程、只读查询抢占、写操作队列、预览确认、树句柄及比较临时文档归属，不放入 ResourceGroup；不把多请求的 State::editor 路由改成全局 EditorIntent。

验收场景：

- 正常激活恰好一项 timer、一项 documents subscription；重复初始化不累积。
- timer 成功、订阅失败，以及关闭部分失败后重试；失败 owner 保留。
- 无关 timer/订阅通知不会触发刷新；文档关闭仍正确释放比较引用。
- 30 秒后台检查不打断编辑/审查，隐藏重开保留当前行为。
- 新包经公开 Manager 启用、停用、成功更新和失败候选恢复，检查实际宿主资源与未保存文档；不能只断言 group.len()。

测试接缝沿用 [scene_receipt_tests](../../../../Git-for-Nanobug/src/scene_receipt_tests.rs)、[quiet tests](../../../../Git-for-Nanobug/src/repository/workbench_quiet_tests.rs)和真实包集成，不新增测试专用宿主接口。

## 5. A2：Markdown 的格式编辑和显式导航

**修改位置：** Formatting::{start_intent,request,complete,cancel_pending}、Navigation::{begin,request,retire_task}。市场 0.17.2 与所核对组织主干 0.18.2 的相关文件内容相同；开始实施时选定一条维护基线，不覆盖本地其它改动。

建议分两步，各自可验证：

1. Formatting 保留一个 EditorIntent，Pending 只记录 document、kind、phase 等领域元数据。用 replace 启动请求，用 update 取得当前回执；保留 30 秒 deadline 和 TryTerminate 策略。
2. 保留读选区 → 计划 → 一次 ReplaceDocumentRange 的阶段，继续检查 document/revision、UTF-8 选区、expected_selection 和最终选择。阶段衔接不另写文本缓冲，也不新增撤销栈。
3. Navigation 独立持有另一个 EditorIntent；保留 Opening、Waiting、fragment 和 viewport driver，仍处理“打开回执先于 Preview”和“Preview 先于回执”两种顺序。
4. source_changed/reset/prepare 等真实已有路径显式取消/close；处理 IntentFailure.cleanup，清理不完整时阻止替代请求，保留 owner。各功能独立观察通知，不短路分发。
5. 外部链接或文件已经打开、编辑已经提交时，取消只按真实效果反馈；旧意图失效不等于操作回滚。

验收场景：

- 快速 A→B、A 完成但尚未交付通知、切换/关闭源文档、revision 变化。
- 读选区后发生用户编辑，旧写入不得应用；成功格式/任务编辑只有一次事务和一次 Undo。
- 旧意图清理失败不会启动 B，重试后可恢复；迟到结果不推进新界面。
- 中文相对路径、外部链接、标题定位和导航等待 Preview；重排不回跳。
- 真实插件 UI 操作后停止开发会话，生命周期报告有实际激活及最终清理证据。

**明确不迁移 Imports：** [imports.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/imports.rs) 用 active + observing 保存被替换 SaveImageInput 的文件副作用回执。它必须接收已保存文件名和结果，不适合 EditorIntent 丢弃旧结果的语义。现有图片输入释放、8 张/32 MiB 限额和最终引用插入继续保留。

来源：[固定 formatting.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/formatting.rs)、[固定 navigation.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/navigation.rs)。

## 6. A3：Markdown 滚动请求，后续独立切片

在 A2 通过后评估 Scrolling::queue/request/cancel_pending。EditorIntent 只承担唯一在途 LocateViewport 请求；保留 pending + latest 合并、document/ui_revision、origin、driver/takeover、2 秒 deadline 与 StopWaiting。

普通滚轮事件继续只更新 latest；不能每帧 replace/cancel 而增加请求或改变原生偏移。已经应用的 offset 不因取消回滚。对同一输入序列比较迁移前后的实际请求数、最终位置、双向驱动、导航抢占和 stale receipt 行为；不把接入助手本身宣称为性能优化。

若替换后接口没有减少复杂性或破坏合并局部性，保留现有实现并记录结论，不为覆盖类型而强行接入。来源：[固定 scrolling.rs](https://github.com/Nanobug-Editor/Markdown/blob/8480eb38e059d575cb5dafd740ed14f035d20936/src/scrolling.rs)。

## 7. 不改动对象及原因

- TypeScript/Vue 按 invocation/document 并发维护服务 Task、引用及多阶段 broker，三个助手不提供这种资源管理。一个全局 EditorIntent 会误取消其它调用者；若以后发现真正可替代的局部 UI 意图，再单列证明。
- Rust 的 Cargo/调试进程和延迟服务回复不是 EditorTask；进程停止及恢复语义仍由原模块负责。
- Image 和 Markdown 已有 PreferenceBinding，ResourceGroup 没有接管其 watch 的能力。
- XML 的同步配置/补全/结构钩子，以及 HTML/JavaScript/TOML 声明式包不新增异步层或空 WASM。
- HttpTask 暂无迁移对象，不新增 network.http 权限。将来有真实访客 HTTP 流时再接入并验证 chunk 边界、非 2xx、取消和部分启动失败。

上述判定的逐文件依据见[源码调研](official-plugin-sdk-adoption-sources-2026-10-21.md)，不是以后禁止扩展的清单。

## 8. 实施顺序和交付门禁

用户已明确授权 A0、A1、A2 分别在三个独立会话并行实施，Markdown 会话在 A2 通过后决定 A3。A0 仅阻断 TS 路径，Git/Markdown 不依赖 TypeScript。各会话使用独立构建输出和验证记录；Git/Markdown 固定已包含助手的 SDK 宿主，避免 TypeScript 的宿主契约变化污染其输入。

每个真正改动的插件：

1. 选定干净源码或记录完整源码摘要，保留当前通过的旧 ZIP 及本批拒绝输入。只提交本切片，保持既有配置和私有数据。
2. 通过包含助手的宿主 --plugin-cargo 使用受管 SDK，记录 EXE/SDK/源码/包 SHA-256；不添加指向 Editor/crates 的依赖，不复制 SDK 源码。当前宿主内容摘要以验收记录为参照，开工时重新固定。
3. 先跑对应模块行为测试和现有插件测试，再直接 --plugin-package 生成新版本；包、协议和能力版本分别处理，不覆盖同名旧归档。依赖和权限按实际使用校对。
4. 新包必须 --plugin-check 通过；隔离 --plugin-dev --report 会话中完成真实业务操作，再发送 stop，不能把 not_exercised 或 inconclusive 当通过。
5. 用运行诊断对照真实资源及阶段耗时。资源基线可能含合法的树/视图/进程，按归属判断；捕获可能截断，CPU/绘制未测量，不累加嵌套墙钟声称总耗时。
6. 原生热更新保留宿主 PID、已打开文档、未保存文本和 Undo/Redo；失败候选保留旧版本及受管数据，成功切换后旧实例资源退役。额外验证停用/移除与 trap 的兜底路径。
7. 同步源码注释、插件变更说明和本批验证记录。宿主若只使用已有通用接口，无需改代码；若发现缺少新能力则另行设计，不塞入插件特例。

本方案完成条件是 A0 有明确配套处理结果、A1/A2 行为与生命周期验收通过、A3 有独立接入或保留结论。发布、市场收录、提交、推送和关闭 issue 不属于当前授权。调研结论及固定核验记录保留原状态；改造交付另记实际实现与验收结果，未通过前不标记完成。
