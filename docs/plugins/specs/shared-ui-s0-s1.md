# 共享 UI：S0／S1 目标、交互与授权契约

日期：2026-10-25。稳定标识：`shared-ui-s0-s1`。

状态：本地规格草案／待实施。用户本轮仅授权完善文档和拆分本地工单；不修改代码、依赖，不运行技术探针，不发布远程议题，不提交或推送。

来源：[共享 UI 路线图](shared-ui-roadmap.md)。实施分解：[本地工单与依赖](../tickets/shared-ui-foundation/README.md)。本规格细化 S0／S1，其余阶段继续按路线图管理。

## Problem Statement

编辑器、普通输入框和只读弹窗已有各自的原生状态及操作，但插件缺少统一、受控的目标引用。焦点可能在打开菜单、等待异步结果或切换视图时变化，不能用“收到结果时的当前控件”决定读取或替换位置。现有剪贴板和文件选择能力也不能自动承担目标绑定、拖放来源验证和跨实例授权。

还需要确定当前宿主和插件是否实际使用同一个本地实现，并在既有依赖不升级、不 fork 的条件下验证 WebView 路线。完成调查不代表 WebView 已可交付，更不能让网页分支的未知项阻塞原生交互。

## Solution

S0 交付可审查的共享行为基线与独立的 WebView 可行性结论。S1 在现有原生状态和请求完成机制上增加受用户动作约束的目标引用，分别打通命令、文本转换、复制粘贴和文本／文件拖放。宿主直接使用本地模块，插件通过公开描述、请求与事件适配器使用同一实现。

所有新增名称与版本均为本规格选定的**待实现契约**，不表示当前 SDK 已有这些调用。实施时同时更新协议、能力库存、包准入、授权界面、运行时、SDK 和消费者；不建立旧协议运行兼容分支。

## User Stories

1. US01: As a maintainer, I want evidence of shared host and plugin input behaviour, so that reuse is verified rather than assumed.
2. US02: As a maintainer, I want a separate WebView feasibility verdict, so that native work can proceed independently.
3. US03: As a user, I want commands to retain their original target, so that changing focus cannot redirect an asynchronous operation.
4. US04: As a user, I want menus to restore focus safely, so that closing a menu does not steal focus from my later choice.
5. US05: As a plugin author, I want opaque target references, so that I do not depend on native objects or UI paths.
6. US06: As a user, I want translation of selected editor text, so that I can replace only the text I chose.
7. US07: As a user, I want the same operation in ordinary inputs, so that tools work beyond the document editor.
8. US08: As a user, I want translation of readonly popup text, so that I can inspect results without modifying the popup.
9. US09: As a user, I want an empty selection to remain empty, so that a tool cannot silently read the whole field.
10. US10: As a user, I want password and sensitive fields excluded, so that UI tools cannot export secrets.
11. US11: As a user, I want composition and native Undo preserved, so that plugin tools do not interrupt Chinese input or corrupt editing history.
12. US12: As a plugin author, I want deterministic stale-target errors, so that delayed proposals cannot overwrite newer work.
13. US13: As a user, I want copying and pasting to use the intended target, so that delayed clipboard reads cannot paste elsewhere.
14. US14: As a user, I want text drag-and-drop to create one native edit, so that its result can be undone normally.
15. US15: As a plugin author, I want dropped files as scoped resources, so that a path string never substitutes for access permission.
16. US16: As a user, I want cancellation and timeout to report their actual effect, so that stopped waiting is not described as rollback.
17. US17: As a user, I want closing a target or disabling a plugin to revoke pending work, so that late replies cannot revive it.
18. US18: As a plugin author, I want causal versions in results and events, so that delivery order across streams does not corrupt state.
19. US19: As a maintainer, I want bounded resources and explicit overflow, so that slow consumers cannot retain unbounded UI state.
20. US20: As a maintainer, I want the same acceptance scenarios through host and plugin entry points, so that one shared implementation remains observable.

## Implementation Decisions

### D01. 范围与共享接缝

- S1 首版覆盖：已挂载的原生文档编辑器、普通文本输入，以及具有原生选择映射的只读文本区域。只读定义弹窗作为后者的验收场景。
- 不具备原生选择映射的绘制文本先不伪造选区。选择“整个字段”是单独的用户动作，不能替代“读取当前选区”。
- 使用既有公开包 → Manager → 宿主原生界面接缝；宿主与插件适配器最终进入同一个本地目标模块和原生状态，不新增测试专用宿主入口。
- 命令路由只支持明确的操作集合：目标描述、全选、原生 Undo／Redo、安全聚焦，以及后续切片接入的复制／粘贴。保存沿用既有文档接口，不为输入框或只读文本创造保存含义。
- 公共目标适配器在首个命令切片内提供原生状态检查、受控读取和原子变更的内部接缝，供后续三条切片复用；可向插件导出文本的公开操作在文本切片实现。不存在“先完成翻译业务才可实现剪贴板／拖放”的前置关系。
- 工作区实例仍遵守信任规则；S1 不扩展应用级插件实例的权限。宿主直接用户操作继续遵循原生策略，不能因新增插件契约为普通键入或既有粘贴追加安装权限。
- 插件自有 `Input`／`Textarea` 的原生输入、`Change` 回显及声明式 `value/value_revision` 更新继续沿用原准入，不要求新 `ui.text.read/write`；新权限只约束主动调用新增通用目标读写。不得用自有字段更新接缝访问其他控件或宿主文档。

### D02. S0 状态与出口

| 调查对象 | 状态转换 | 结论与证据 |
| --- | --- | --- |
| 共享行为基线 | 未开始 → 调查中 → 已记录／证据不足 | 固定源码快照、既有依赖版本、调用链及验证入口；同一输入在宿主和插件中的状态、绘制、主题、事件、销毁逐项对照 |
| WebView 门禁 | 未运行 → 探查中 → 可行／有条件／不可行 | 分别记录嵌入、叠放与裁剪、DPI、焦点／IME、消息、销毁、运行时缺失、依赖及许可证证据 |

WebView 探查与对照基线可独立进行，各自记录源码与输出目录；没有证据或环境缺失只能给“有条件／未验证”，不能给“可行”。“调查完成”与“技术门禁通过”分别记录。

可行要求已证明不 fork、不升级既有依赖的承载路径及原生行为；有条件结论列出尚未满足的具体条件。后续 WebView 实施必须满足这些条件，原生 S1 不受该结论阻塞。新增固定版本绑定与升级既有依赖分别审查，不通过复制整套第三方绑定绕开限制，不静默安装运行时。

### D03. 目标、动作租约与守卫

| 概念 | 约束 |
| --- | --- |
| 原生目标身份 | 由宿主生成，绑定工作区会话、源拥有者 incarnation、原生目标代际和源资源；路径、节点 ID、当前活动标签不能代替身份 |
| 动作租约 | 由宿主在用户直接选择目标和接收插件时签发；绑定接收实例、目标、范围、操作集合、用户动作及截止时间。插件只能回传不透明句柄 |
| 编辑守卫 | 宿主封装内容 revision、选区 epoch、输入 epoch；插件不得自行拼装或扩大。受理和原子变更前均检查 |
| 内容 revision | 内容变更、Undo／Redo、显式重置时推进；恢复为相同字节也不复用旧版本 |
| 选区 epoch | 逻辑选区变化时推进；移开后再选回相同范围仍使旧提案过期 |
| 输入 epoch | IME 开始、结束、取消及输入事务切换时推进；提交还检查当前是否组合中 |
| UI revision | 验证用户动作的来源场景；单纯主题、尺寸、滚动更新不使已捕获的文本守卫过期。更换源对象必须推进目标代际 |

用户动作来源由宿主密封，不能由访客字段、插件 ID 或字符串伪造。后台服务、定时器和跨插件调用不能自行捕获目标或继承动作租约。对其他插件的普通可见文本，仅用户明确选择该目标和工具时签发定向租约，不转让对方私有资源或文档句柄。

动作目的同时受限。翻译／替换选区租约不能执行 Undo／Redo，整字段读取租约也不自动授予历史操作。Undo／Redo 必须由用户明确发起对应的目标级历史动作并具备写入权限；历史项若超出普通范围租约允许的修改范围，返回 `PermissionDenied`，不能借历史操作扩大授权。

### D04. 目标与租约状态

| 当前状态 | 触发 | 后续与效果 |
| --- | --- | --- |
| 未公开目标 | 原生对象已建立并接入状态 | 可挂载；未向插件签发租约 |
| 可用目标 | 失焦，但仍挂载可见 | 保持原身份与租约；异步操作仍指向原目标，不抢回焦点 |
| 可用目标 | 模态阻断、禁用或进行中的 IME | 暂不可执行；相关操作返回明确错误，不自动排队到未来输入状态 |
| 暂不可执行 | 原生状态解除 | 恢复可用；涉及输入或内容变化的旧守卫仍可能过期，不能自动刷新守卫 |
| 可用／暂不可执行 | 隐藏、卸载、更换源、关闭窗口、源或接收实例退役、信任撤销 | 撤销呈现租约和关联订阅；重新显示或重新打开必须重新签发，旧句柄不能复活 |

“隐藏撤销”仅限新增 UI 呈现租约；现有显式文档接口对后台文档的读写语义保持有效，不被新规则收窄。普通外观重绘及稳定节点复用不等同于卸载，更换来源或节点种类则撤销旧代际。

租约状态为“有效 → 已消费／已撤销／已到期”，终态不可恢复。读取只导出捕获时获准的范围和版本；成功变更消费该次变更授权，重复请求不得再次编辑。过期后不得轮询获取后续用户输入；扩大范围或更新守卫需要新的明确用户动作。

焦点恢复还需窗口有效、原焦点仍可交互、恢复者仍拥有该交互，以及用户没有作出更新的焦点选择。租约有效不等于允许后台抢焦点。

### D05. 能力与权限矩阵

以下新能力均拟采用独立 `^1` 契约；当前未实现。新权限必须同时进入包准入白名单、安装／更新授权说明和运行时检查，不能只更新声明式 schema。

新增 `ui.text`、`ui.focus` 和 `ui.drag_drop` 的目标操作同时要求 `ui.targets ^1`；它们不会替代对方的能力或权限。既有裸剪贴板、原生文件选择和插件自有字段交互不因此追加新能力要求。

| 操作 | 能力 | 安装权限及附加条件 |
| --- | --- | --- |
| 捕获／描述／释放目标，按支持集合路由命令 | 新 `ui.targets ^1` | 捕获本身只返回元数据；要求宿主签发的用户动作，不新增全局枚举权限。具体命令继续检查其资源权限 |
| 插件请求聚焦获准目标 | 新 `ui.focus ^1` | 新 `ui.focus`；前台用户动作及焦点所有权检查。宿主内部安全恢复不要求该插件权限 |
| 读取普通字段／只读文本 | 新 `ui.text ^1` | 新 `ui.text.read`、有效定向租约、获准范围；不包含敏感区域 |
| 替换普通字段 | 新 `ui.text ^1` | 新 `ui.text.write`、可编辑目标、原守卫与替换范围 |
| 经通用文本目标读取文档内容 | 新 `ui.text ^1`，复用 `editor.edit` 或 `editor.documents` 对应操作 | 既有 `editor.read` 与定向租约；范围／整字段选择仍分开 |
| 经通用目标修改／Undo／Redo 文档 | 新 `ui.targets`／`ui.text` 对应操作，复用 `editor.edit` | 既有 `editor.write` 与变更守卫；多编辑使用现有 `editor.edit >=1.1`；不因此获得磁盘写入权 |
| 普通字段原生 Undo／Redo | 新 `ui.targets ^1` | 新 `ui.text.write` 与动作租约；只操作该字段自己的原生历史 |
| 读取／写入系统剪贴板 | 既有 `ui.clipboard ^1` | 既有 `clipboard`。保留当前裸读写的准入含义，不伪称它们已具备目标校验 |
| 插件发起的定向复制／粘贴 | 新 `ui.targets ^1` + 既有 `ui.clipboard ^1` | 既有 `clipboard`；复制另检查源文本读取权限，粘贴另检查目标写入权限；不依赖公开翻译操作已经实现 |
| 接收宿主确认的拖放 | 新 `ui.drag_drop ^1` | 类型化有效投递。主动取得文本载荷要求对应读取授权；文件要求既有 `files.select` 及 `files.selection`，不新增 `files.drop` 权限 |
| 实际读写所选文件／工作区文件 | 既有选择资源／工作区接口 | 继续检查原有句柄、访问模式及 `workspace.read/write` 等各自适用授权；拖入不自动授予写权限 |
| 显示翻译结果、交互或保存 | 既有面板／交互／文档接口 | 分别检查 `ui.panels`、`ui.interaction`、`editor.write` 等原有规则；目标租约不隐含这些能力 |

文档能力的当前已知下限：`ReadDocument` 为 `editor.documents >=1.1`，`ApplyEdits` 为 `editor.edit >=1.1`；普通选区读取和单次范围替换保持现有 `editor.edit` 语义。`ui.clipboard 1.0` 对应单一安装权限 `clipboard`，不新造 `clipboard.read/write`。文件选择库存为 `files.selection 1.1`，显式写入同意仍按其既有契约处理。

密码、凭据和宿主标记敏感区域不签发文本租约；默认拒绝内容导出，日志也不记录载荷。只读目标可读与可呈现结果，写入返回不支持操作。安装权限、协商成功和用户动作三者不能互相替代。

### D06. 范围、IME 与原生编辑

- `Selection` 与 `WholeField` 是两种显式授权范围。空选区返回空文本和空范围，不能回退为整个字段；只读无选区区域可由用户明确选择“整个文本区域”。
- 读取一次至多 256 KiB；替换至多 1 MiB；完整请求继续遵守既有 2 MiB 上限。范围为捕获版本上的半开 UTF-8 字节范围，边界不合法不自动夹紧。
- 原生输入处于 IME 组合时，不读取未提交组合串，不强制结束组合。捕获文本快照或提交替换返回 `InvalidState`；组合结束后需重新捕获／显式重试。
- 后来失焦不会改写目标；内容、选区或输入 epoch 变化后，旧提案返回 `StaleRevision`。只改主题、滚动或字号不触发该错误。
- 文档写入通过唯一原生文档会话；字段写入通过其唯一输入状态。一次成功替换形成该目标的一次原生 Undo 单元，不新增跨目标撤销历史，不自行保存到磁盘。
- 新文本由宿主校验和提交；插件只提供结果。并行提案串行穿过同一原子门，第一个变更后其余旧守卫失败。

### D07. 请求状态、提交与取消

复用既有 EditorRequest、Completion、副作用门和 `RequestUpdate`，不增加第二套取消源。受理不等于成功；元数据同步返回无需人为制造进度事件。

| 状态／事件 | 结果 |
| --- | --- |
| 校验失败，尚未受理 | 直接返回 `Failure`，不创建可执行请求或副作用 |
| 已受理 → 准备中 | 允许读取获准快照和准备原生操作；尚未改变目标 |
| 门前取消／超时赢得竞争 | 终态为 `Cancelled`，原因分别为 `Cancelled`／`TimedOut`，effect 为 `NotExecuted`；不得发生编辑或授权交付 |
| 原子副作用门通过 | 重查生命周期、权限、范围、目标状态、守卫、期限与取消；通过后执行唯一原生事务 |
| 编辑已执行 | 捕获本次精确版本、选区和因果序号，等待原生 Change 汇入会话后形成成功回执；不在 deferred 回调时改读“当前最新版本” |
| 门后停止等待 | 若首终态尚未确定，返回 `Cancelled` 且 effect 为 `WaitingStopped`；操作可能已执行，不能报告回滚或 `NotExecuted` |
| 已终态 | `Completed`／`Cancelled` 不再改变；迟到回调不产生第二个终态或第二次编辑 |

三道检查分别发生于受理、原子提交前、结果交付／展示前。最后一道检查可以阻止死实例收到数据或抢焦点，但不能否认已经发生的原生编辑或外部副作用。丢失等待结果时，存活且仍获授权的调用者通过原资源接口重读状态，不能盲目重放写入。

本次成功变更已消费的租约不阻止交付其已冻结回执；交付门不要求旧编辑守卫等于提交后的最新版本，也不重新执行变更。目标随后关闭或接收方退役可以抑制展示，不能把已执行编辑改报为门前拒绝或未执行。

### D08. 事件顺序与溢出

逻辑顺序固定为：宿主捕获用户动作与目标 → 校验及受理 → 取得快照或准备操作 → 原子门重查 → 原生状态变更 → 记录本次版本与目标变化序号 → 发布变化和请求结果。

- 同一目标代际的 `target_sequence` 单调推进；提交回执携带此次请求关联、目标代际、该序号、精确提交版本及选区。内容、选区、可执行性及撤销变更有类型化元数据事件，不包含文本载荷。
- `target_sequence`、既有文档事件 sequence、请求更新序号和 UI revision 属于不同域，不允许直接比较或构造全局总序。
- 同一订阅内按目标序号保持顺序；不同通知流与请求结果不保证送达先后。调用者以回执中的因果关联去重和同步，不能靠回调先后猜测提交版本。
- 动作租约和请求期限均不得超过 300,000 ms；保留的租约、拖放会话及订阅计入现有实例资源预算，不能开旁路。S1 每实例至多八个目标订阅，每个队列最多 128 条；既有更严格总额度同时适用。
- 首次订阅和溢出后恢复均在同一原生状态事务内“建立订阅并返回元数据快照”，快照含目标代际及 `target_sequence` 水位。只消费大于水位的后续事件；快照与订阅建立之间不能留出未覆盖的变更窗口。若交付失败，清理尚未交付的订阅。
- 目标事件不静默丢弃。溢出时终止该订阅并以 `SubscriptionFailed(LimitExceeded)` 对应语义通知；只终止订阅，不假装撤销原生文本。恢复时按上一条原子订阅流程重新取快照，禁止沿缺失事件继续增量应用。恢复快照仅含元数据，不扩大旧动作范围或刷新旧编辑守卫。
- 拥有者撤销先关闭授权和回调门，再清理订阅与载荷。终止通知能否送达取决于接收实例是否存活；清理不能依赖其消费通知。

### D09. 剪贴板与拖放首版

**定向剪贴板**：先固定源／目标与守卫，再调用既有剪贴板请求。复制检查读取权限及敏感性；粘贴在异步读取完成后重新检查原目标和守卫。原目标失效不能粘到新焦点。S1 只处理文本；图片粘贴保留原有专门接口。

操作系统剪贴板已写入后不能承诺取消会恢复旧内容。读取失败与空剪贴板分别处理；空文本可以合法返回，但不能作为权限、超时或取消的替代成功结果。

**拖放状态**：未开始 → 拖动中 → 候选目标 → 宿主确认投递 → 准备载荷 → 交付／取消／失败。候选高亮不授予读取权限；确认时固定目的目标、接收实例和类型，交付前再次校验。已完成投递不能重放。

- 首版只支持文本复制和只读文件资源投递，不增加任意结构化二进制通道，不自动执行移动、删除、复制文件事务。WebView 拖放留给后续承载阶段。
- 文本至多 1 MiB；文件批次至多 32 个，并遵守既有实例资源额度。批次全部通过路径、类型、归属和权限检查后才签发，不允许半批授权后以失败结束。
- 文件投递使用新的宿主认证拖放来源，复用 `files.selection` 的受控对象访问规则；不得伪造 picker 完成或放宽所有服务／委派调用。
- 接收实例获得重新签发的只读 `SelectedResource`。路径字符串、源句柄及源实例权限均不转授；即便是同一插件的另一个实例，也不能直接转发句柄。
- 读取仍检查规范化路径及符号链接边界；文件实际变更继续进入已有工作区／所选资源写入流程。投递只授予被选择对象的读意图。
- 准备后取消、关闭目标或实例退役，释放尚未交付的资源；成功交付的资源按其正常寿命管理，不随拖动浮层销毁而错误释放。

### D10. 错误契约

统一使用既有 `Failure { code, message }`，调用方不解析 message。错误解释可本地化，但不能包含被拒绝目标的秘密、完整载荷或私有路径。

下表“无副作用”等表述仅适用于原子提交前的拒绝。门后关闭、版本推进或实例退役按 D07 的已冻结提交事实、`WaitingStopped` 或抑制交付语义处理；不能把已执行操作改报为门前 `InvalidHandle`／`StaleRevision` 并暗示未执行。

| 条件 | ErrorCode | 副作用／重试语义 |
| --- | --- | --- |
| 请求结构、范围顺序、UTF-8 边界、期限参数非法 | `InvalidRequest` | 无；修正参数 |
| 编码／载荷／资源／队列超过额度 | `LimitExceeded` | 不部分授予；缩小请求或重新订阅 |
| 能力未实现、未协商或版本不足 | `CapabilityUnavailable` | 无；按可选能力降级或更新声明 |
| 缺安装权限、缺用户定向授权、敏感区域或超出范围 | `PermissionDenied` | 无；不能通过重试或换句柄扩大权限 |
| 句柄伪造、跨实例／作用域、关闭、隐藏、到期或旧代际 | `InvalidHandle` | 无；新用户动作重新捕获，不能复活旧句柄 |
| 目标仍有效但内容、选区或输入守卫过期 | `StaleRevision` | 无；重新取得用户确认及快照，不自动改投当前焦点 |
| 目标天然不支持写入／聚焦，或平台不支持操作 | `UnsupportedOperation` | 无；按目标能力降级 |
| IME 组合中、模态阻断、临时禁用或投递状态已结束 | `InvalidState` | 不强制结束原生交互；状态恢复后显式重新发起 |
| 原生互斥事务占用，且尚未发生本次变更 | `Conflict` | 无；结束冲突后以有效守卫重试 |
| 获准文件对象已不存在 | `NotFound` | 不改为新路径；由用户重新选择 |
| 文件路径／设备路径／备用流等不合法 | `InvalidPath` | 无；不能退回未经校验的绝对路径 |
| 用户取消或期限届满 | `Cancelled`／`TimedOut` | 受理后的终态携带 `NotExecuted` 或 `WaitingStopped` |
| 合法原生操作执行失败 | `OperationFailed` | 说明已知效果；不能一律声称未执行 |

校验顺序为：结构和硬额度 → 能力 → 实例／信任及适用安装权限 → 句柄归属与租约来源／范围 → 支持操作 → 临时状态与守卫 → 原子门重查。类型相关权限可在解析受控句柄元数据后确定，但读取内容前必须通过；跨实例句柄按 `InvalidHandle` 处理，不能误报版本过期。正在 IME 时优先报告 `InvalidState`，组合已结束而守卫变化则为 `StaleRevision`。

## Testing Decisions

### 共同验证接缝

沿用宿主直接调用与实际插件包经公开 Manager 接入两条路径，共用同一原生模块。协议测试验证描述和错误，模块测试验证目标状态及原子门，宿主集成验证唯一会话和回执，Windows 原生输入验证焦点、键盘、IME、剪贴板及拖放。用确定性文本转换验证机制，不引入在线翻译或特定插件宿主接口。

S0 调查只读已有实现与记录；技术探针在其工单获实施授权后才执行，单独使用输出目录并核对 workspace／target，不能污染当前未提交工作。测试不靠添加测试专用公开操作来驱动取消或过期。

### 验收编号与主责

| 编号 | 可观察要求 | 工单与验证方式 |
| --- | --- | --- |
| F01 | 宿主和插件输入的实际状态、绘制、事件、主题及销毁可对照，已有／在途／缺失可区分 | 01；只读调用链和既有记录，不新增测试代码 |
| F02 | 锁定依赖、无 fork、无组件运行时回流，已有验证与未验证部分明确 | 01；清单／锁文件及既有依赖证据 |
| F03 | WebView 嵌入、叠放、DPI、IME、消息、销毁及缺运行时结果可复现 | 02；获实施授权后原生探针 |
| F04 | 可行／有条件／不可行与调查完成分开，负面结论不阻塞 S1 | 02；结果清单与依赖图检查 |
| F05 | 三类原生目标的命令不会因后来失焦改投其他目标 | 03；宿主＋公开插件原生集成 |
| F06 | 模态关闭不抢回更新的焦点，IME 不被远程命令打断 | 03；真实键盘／IME 与模态交互 |
| F07 | 同 ID 重建、同路径重开、选区 ABA、外观刷新遵守代际与守卫区别 | 03；可控生命周期测试和原生观察 |
| F08 | 元数据不等于内容授权；敏感、只读及跨实例限制成立 | 03／04；包准入、权限负例及原生结果 |
| F09 | 选区／整字段分开，空选区不泄露全字段，三类区域均可定向读取 | 04；确定性转换插件与边界输入 |
| F10 | 一次替换仅影响原目标、形成一次原生 Undo，回执是本次提交版本 | 04；原生文本／选区／历史及 deferred 竞态 |
| F11 | 取消、副作用门、迟到交付及跨流乱序结果一致 | 04；现有请求接缝与延迟消费者 |
| F12 | 定向复制／粘贴复用已有剪贴板接口，延迟读不误贴新目标 | 05；宿主＋插件＋Windows 剪贴板 |
| F13 | 空值、超限、缺权限、IME、取消与已写 OS 副作用可区分 | 05；自动边界和原生回归 |
| F14 | 文本投递为一次编辑；文件投递重新签发接收方只读资源 | 06；真实拖放与公开资源访问 |
| F15 | 假来源、跨实例、非法路径、半批授权、取消和重放均被拒绝 | 06；受控文件夹具与原生取消 |
| F16 | 翻译等待时发生粘贴／拖放／焦点切换，旧结果不覆盖新状态 | 07；三切片联合场景 |
| F17 | 撤销信任／退役／隐藏重建后无迟到写入、抢焦点、授权或订阅残留 | 07；原生联合生命周期 |
| F18 | 配额、溢出和重订阅有界且不静默丢事件 | 03 基础、07 联合；慢消费者与恢复 |
| F19 | 两种调用方共用实现，独立目标正常工作，原生文档后台接口不回退 | 07；跨入口、跨目标、现有文档回归 |
| F20 | 能力缺失与权限缺失分开，更新新增权限可见，SDK／消费者契约一致 | 各功能单基础、07 联合；包与公开管理器检查 |

每个实施工单运行受影响测试及仓库规定的格式、workspace 测试和编译检查；宿主改动追加相关测试和原生交互，实际 WASM 夹具按当前直接构建入口准备并显式运行相关 ignored 测试。各单记录结果，不将本轮文档检查、既有记录或跳过项视为实现通过。

## Out of Scope

- 本轮实现代码、运行探针、升级依赖、fork、远程议题、提交、推送或关闭工单。
- S1 的全界面文本扫描、密码导出、后台全局焦点控制、全局跨控件 Undo、任意跨插件句柄共享。
- 新 DataGrid、可编辑临时文档、网页文件编辑器、富文本编辑、图片剪贴板及 WebView 拖放的实现；沿路线图后续阶段交付。
- 重新实现已有树、文件选择、系统剪贴板、文档保存和工作区事务系统。

## Further Notes

本地工单按端到端垂直切片组织。S0 对照调查与 WebView 调查独立；S1 的文本、剪贴板和拖放共享目标基础，联合出口只承担跨切片竞态和资源撤销，不重复所有控件测试。

当前所有新能力均未实施。接口使用说明最终应与公开协议源码注释共置，本地规格用于实现依据，不作为随 SDK 导出的文档。
