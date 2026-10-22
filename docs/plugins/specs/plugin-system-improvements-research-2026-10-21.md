# Nanobug 插件系统改进调研

日期：2026-10-21。

状态：调研建议，未批准实施。本文不是已确认规格、实施工单或验收记录，不改变现行公开契约。外部事实来自本次实际读取的一手来源；“迁移建议”和“取舍”是基于这些事实的设计判断。

范围：比较 VS Code、IntelliJ Platform、Chrome Extensions、Neovim、Obsidian、Zed 六类插件系统。目的在于改进 Nanobug 已有能力的可用性、可验证性和维护成本，不能把其他系统拥有某机制直接推断为 Nanobug 缺少该机制。

后续：用户已选择 SDK 生命周期助手、运行诊断和兼容验证工具，具体源码接缝、交付顺序与验收条件见[改进方案草案](plugin-developer-tooling.md)。该草案不代表已授权实施。

## 外部系统证据与取舍

### 1. VS Code：实验入口、开发测试和问题定位形成闭环

**核验事实。** VS Code 将不稳定接口放入 Proposed API，允许在 Insiders 的本地开发中试用，稳定后再供正式扩展使用；文档明确不应把使用 Proposed API 的扩展发布到 Marketplace。这是在接口被长期依赖之前收集反馈的制度。[Using Proposed API](https://code.visualstudio.com/api/advanced-topics/using-proposed-api)

官方测试工具可在独立 Extension Development Host 中执行集成测试，并选择宿主版本、工作区和开发插件目录；开发者不必把测试接入宿主源码仓库。[Testing Extensions](https://code.visualstudio.com/api/working-with-extensions/testing-extension)

运行期工具包含 Running Extensions、CPU profiling，以及通过禁用部分扩展逐步定位故障的 Extension Bisect。[Performance Issues](https://github.com/microsoft/vscode/wiki/Performance-Issues)

**迁移建议。** 在 Nanobug 现有 stable/experimental 和隔离开发入口上，重点考虑：插件作者可以直接运行的公开测试入口、精确宿主版本矩阵、清晰的 API 稳定化条件，以及按插件和请求展示耗时、排队、超时与资源占用的诊断视图。先让真实插件作者完成一次“开发—验证—故障复现”，再判断是否需要更多接口。

**取舍。** 不需要为了学习这些机制引入 Node、Electron、Webview 或另建一套宿主内部测试 API。诊断采集也应有采样和容量边界。插件二分需要处理依赖关系，并且只是排查工具，不能保证识别所有多插件交互问题。

### 2. IntelliJ Platform：把兼容检查交给插件作者和持续集成

**核验事实。** Plugin Verifier 检查插件与多个 IDE 构建之间的二进制兼容性，可通过 Gradle、CI、Marketplace 或独立工具运行，报告可定位到插件源码。[Verifying Plugin Compatibility](https://plugins.jetbrains.com/docs/intellij/verifying-plugin-compatibility.html)

动态插件要求使用支持动态变更的扩展点、正确释放资源，并通过专门检查；卸载失败会要求重启。文档还提供动态加载日志及泄漏诊断方法。[Dynamic Plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html)

**迁移建议。** 复用 Nanobug 已有静态包校验、能力协商和固定旧包测试，形成面向独立插件仓库的统一公开验证入口，提供机器可读诊断与稳定退出码：输出不支持的能力版本、缺少的权限、非法贡献、失败阶段和可定位字段；持续运行“历史插件原始 ZIP/WASM × 受支持宿主”矩阵。静态市场准入保持不执行 WASM；需要运行插件的动态契约验证放在显式开发模式，报告停用后的任务、进程、订阅和面板残留。

**取舍。** JVM 二进制验证不能原样用于 WIT/WASM；静态兼容通过也不能证明语义、数据迁移和原生 UI 行为正确。应保留实际运行与错误注入测试。无需照搬 IntelliJ 内部 PSI、类加载器或允许插件覆盖宿主服务。

### 3. Chrome Extensions：权限与用户正在启用的功能对应

**核验事实。** Chrome 区分必需权限、可选权限以及按来源声明的 host 权限。可选权限支持在运行期授予，官方建议在功能允许时减少必需权限。[Declare permissions](https://developer.chrome.com/docs/extensions/develop/concepts/declare-permissions)

权限请求需要来自用户操作；插件可以检测当前授权，权限被撤销时有 `onRemoved` 事件。因此插件需能处理某项可选功能失去授权的情况。[Permissions API](https://developer.chrome.com/docs/extensions/reference/api/permissions)

**迁移建议。** Nanobug 已有精确 origin 和安装授权，规格也要求区分必需/可选权限；改进重点应是权限审查体验：说明哪项功能需要哪个 origin、文件范围或执行能力，展示更新增加的权限，并评估查看与撤销可选功能授权的完整流程。撤销后的 SDK 事件、受影响请求终止、UI 降级应形成一个可测试流程，不能把规格要求直接当作当前界面已实现的证明。

**取舍。** 运行期申请是需另行评审的产品选择，不是本次批准项；须保持 Nanobug“正常调用不反复弹窗”的约定，避免照搬浏览器频繁的访问请求。浏览器站点来源模型不能替代本地规范化路径、实例归属和原生进程边界校验。

### 4. Neovim：把调用时机作为 API 契约的一部分

**核验事实。** Neovim 区分进入主循环的 deferred API 与立即执行的 fast API，fast 上下文通常不能直接修改编辑器状态；需要经 `vim.schedule()` 等入口调度。[Nvim API](https://neovim.io/doc/user/api/#api-fast)

Lua API 明确规定 `vim.uv` 回调不能直接调用多数编辑器 API，同时提供 `vim.schedule_wrap()` 便捷转发；RPC notification 立即返回，request 会等待响应。[Lua engine](https://neovim.io/doc/user/lua/#lua-loop-callbacks)

**迁移建议。** 在 Nanobug 已有异步请求和 revision 校验上，统一公开“可在哪种回调发起调用、是否允许重入、何时完成、取消后还可能收到什么”的规则；SDK 尽量代管关联请求、取消和事件分发。开发模式可以为慢回调、阻塞等待或过量事件提供归属明确的诊断，而不是要求作者从宿主源码推断调度约束。

**取舍。** 学习的是明确调度契约与便捷 API，不能把 Neovim 的同步 RPC、输入队列或 Lua 可直接调用编辑器的模型搬进 GPUI/WASM 宿主。合并事件、丢弃中间结果和背压策略需分别定义，不能为了流畅而静默丢失有业务意义的事件。

### 5. Obsidian：让资源生命周期自然跟随组件

**核验事实。** Obsidian 的公开 `Component` API 支持子组件随父组件加载和卸载；`register` 注册卸载清理函数，`registerEvent`、`registerDomEvent` 和 `registerInterval` 把事件与定时器清理绑定到组件生命周期。[官方 API 类型定义](https://raw.githubusercontent.com/obsidianmd/obsidian-api/master/obsidian.d.ts)

**迁移建议。** Nanobug 已有实例资源归属与卸载清理，值得进一步评估 SDK 的“资源组/作用域”便利接口：面板关闭时统一取消关联请求、释放订阅，插件卸载时再由宿主兜底。目标是减少作者重复维护句柄集合及遗漏清理的机会，而不是另造一层转发包装。

**取舍。** 不引入 DOM 或无限制宿主对象访问；Rust 自动析构也不能取代宿主的强制撤销，因为 WASM trap、预算耗尽或实例销毁未必执行访客清理代码。便利接口应建立在现有句柄所有权上，定义显式关闭与异常退出的区别。

### 6. Zed：授权策略可解释到具体操作

**核验事实。** Zed 的 `granted_extension_capabilities` 可限制扩展执行能力。公开配置区分进程命令与参数、下载 host 与路径、npm 包名；缺少授权时相应 API 返回错误。[Extension Capabilities](https://zed.dev/docs/extensions/capabilities)

**迁移建议。** 对 Nanobug 来说，可借鉴的是权限错误与授权审查的可解释性：开发者能知道被拒绝的是哪项操作、哪个作用域或哪条授权规则；用户能区分“启动已声明服务”与“任意执行”。现有精确 origin 授权不必退回通配域名模型。

**取舍。** 命令及参数匹配并不是操作系统沙箱，也不保证受允许运行的程序没有更广泛副作用。不应为了规则灵活性默认授予通配执行权限，或把原生服务说成受到 WASM 隔离。规则复杂度应由真实需求推动，避免形成另一套难以审查的策略语言。

## 证据适用范围

- 本次读取的是上述官方文档及公开 API 源码的在线状态，不是对这些产品的实际安装测试；链接内容后续可能变化。
- 外部系统的事实与迁移建议分别标注。任何新授权流程、SDK 接口和公开工具均需对照 Nanobug 当前实现进一步收敛，本文不授权实施。
- 比较聚焦插件开发与维护机制，不据此宣称某平台整体更安全、更快或更成熟。

## Nanobug 当前依据与建议

### 核验范围

本次读取的是 2026-10-21 的当前工作区，HEAD 为 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b`，同时存在大量先于本次调研的未提交变更。下列“已有”只表示当前源码及对应历史记录有依据，不意味着所有工作区改动已发布，也不替代本次没有运行的行为测试。调研不修改运行时代码，不创建或执行工单。

- [社区基础验收入口](../verification/plugin-api-community-foundation/README.md)记录文档、事务、交互、诊断、树与装饰、HTTP、密钥和按需激活的七单交付。当前[能力目录](../../../crates/plugin-runtime/src/capabilities.rs)与[协议源码](../../../crates/plugin-protocol/src/lib.rs)有对应入口；不能继续沿用 2026-10-09“这些接口仍缺失”的判断。
- [公开 SDK 总注释](../../../crates/plugin-protocol/src/lib.rs)已经规定稳定主版本、实验能力及弃用要求；[07 验收](../verification/plugin-api-community-foundation/07-activation-and-compatibility.md)包含未重建旧 SDK ZIP 的兼容验证。建议是持续化并对作者开放工具，不是重新设计版本机制。
- 已有[隔离开发 CLI](../../../crates/editor-app/src/plugin_development/cli.rs)、[静态包校验器](../../../crates/plugin-runtime/examples/inspect_package.rs)和市场校验流程。市场现行[README](https://github.com/Nanobug-Editor/Plugin-Marketplace/blob/main/README.md)与[规格](https://github.com/Nanobug-Editor/Plugin-Marketplace/blob/main/docs/specs/plugin-marketplace.md)明确审核、不可变包摘要、撤回和归属转移；不把这些机制当作空白。
- [HTTP](../../../crates/plugin-protocol/src/http.rs)已经按精确 origin 授权并检查重定向；[定时器](../../../crates/plugin-protocol/src/timers.rs)与[激活契约](../../../crates/plugin-protocol/src/activation.rs)已定义预算和回收。原生服务的实际 OS 权限仍不能由 WASM 隔离推导。

### 建议优先级

以下是设计推断和待讨论建议，均未批准实施。P1/P2 表示建议投入顺序，不是已确认缺陷等级。

| 优先级 | 建议 | 已有基础与增量 | 评价改进是否有效的证据 |
| --- | --- | --- | --- |
| P1 | 在现有 SDK 内补生命周期便利接口 | 已有 `EditorTask`、`PreferenceBinding` 和显式释放；增加可组合的资源组、HTTP 任务驱动和“新意图替换旧请求”辅助模式，减少业务插件手动关联通知、取消与关闭资源 | 一个查询视图关闭或替换时，不留流、定时器或订阅；迟到结果不覆盖新意图，作者不必读宿主内部代码 |
| P1 | 对作者提供统一验证入口与持续兼容矩阵 | 复用 `inspect_package`、独立开发和真实包测试；输出声明字段、所需能力下限、权限、错误码及包/SDK 身份。旧包原字节运行与用新 SDK 重建分别验证 | 稳定 minor 更新可重复证明旧包行为；新功能漏写能力下限可定位；动态测试须显式在开发/CI 隔离环境运行，不把执行第三方 WASM 混入市场静态准入 |
| P1 | 增强按插件的运行诊断 | 已有按插件限量日志与预算异常；增加激活原因、排队/访客执行/宿主或原生 IO 等待阶段、请求关联、资源数和 UI 发布量的有界统计 | 慢请求能区分插件计算、队列阻塞、外部服务和宿主呈现；能导出脱敏诊断。fuel、调用墙钟耗时和原生进程资源不得混称 CPU 占用 |
| P2 | 把增量 UI 与交互状态保留做成常用 SDK 模式 | 已有原生节点、`ui.incremental`、树补丁和画布 epoch；让稳定 ID、局部发布、保留已呈现数据及显式空态易于正确使用 | 快速切换和刷新不闪空白、不丢焦点/选择/滚动；迟到事件按内容身份拒绝；至少第二个无关消费者验证，不能用 Git 领域分支实现 |
| P2 | 完善权限的功能解释与撤销体验 | 保留精确 origin、选择文件授权、安装/新增权限确认；区分必需与可选功能，并让用户理解禁用某项权限的影响 | 用户能判断“查询服务”和“任意执行程序”的不同；撤销后相关在途工作终止。若支持用户新配置的服务地址，需另行明确主动授权流程，正常调用不反复弹窗 |
| P2 | 统一可检验的能力元数据 | 操作类型、能力目录、UI 最低版本推导和运行时校验已经存在；补足或归一稳定级别、最低 minor、静态权限说明、取消语义与释放义务等元数据，用于诊断和一致性检查 | 新增操作不会遗漏清单校验、SDK 注释或能力下限；动态资源归属和路径授权仍由运行时检查，元数据不能冒充权限证明 |

SDK 便利层应留在现有协议/guest 边界，封装真实的关联、取消与回收复杂度，不创建仅转发调用的新 crate。资源组必须区分不同操作的完成、取消、关闭语义；不能因为 HTTP 观察到终态就假定句柄已释放，也不能依赖 WASM trap 后仍执行访客析构。宿主退役回收继续兜底。

当前 [HTTP 源码注释](../../../crates/plugin-protocol/src/http.rs)明确要求插件保留定时器轮询流，并在终态后关闭流和定时器；[guest 助手](../../../crates/plugin-protocol/src/api/guest.rs)已有请求关联封装，[PreferenceBinding](../../../crates/plugin-protocol/src/api/preference_binding.rs)已有关闭旧观察的先例。因此优先扩展这些真实接缝，比另起通用异步框架更合适。

当前 [日志记录结构](../../../crates/plugin-runtime/src/logs.rs)提供插件、时间、级别、来源和文本；[访客调用](../../../crates/plugin-runtime/src/instance.rs)识别预算异常。这支持“完善结构化性能证据”的建议，但本次检索不足以宣称仓库所有位置都没有局部计时。

[Git 增量刷新记录](../verification/git-incremental-refresh-2026-10-20.md)和[画布连续滚动记录](../verification/canvas-scroll-2026-10-21.md)已经包含修复与候选证据。引用它们是识别值得统一处理的交互模式，不是宣称这些具体缺陷仍未修复，也不把其未发布候选算作正式版本。

### 近期不建议扩大

- 不为追赶 API 数量移植 IDEA 的整套内部语义模型，或引入 Node/DOM/WebView；继续尊重原生 UI 和宿主通用机制的定位。
- 不仅因为 WIT 当前传送 JSON 就全面重写传输。先测量序列化、跨边界调用和 UI 发布成本，再决定是否对热点增加批量或二进制能力。
- 不把更多语言 SDK、复杂市场后台或原生进程 OS 沙箱作为这批建议的默认工作；这些都需要独立需求、维护成本与平台验证。
- 不恢复随附官方插件或已删除的示例源码目录。新手样例如有需求，可使用独立示例仓库或 SDK 源码中的可编译示例，继续通过宿主构建入口，避免复制 SDK。

建议首轮只选 SDK 生命周期便利接口、运行诊断和兼容验证工具，使用一个真实独立插件衡量开发代码量、排障耗时及旧包兼容结果，再决定后续范围。
