# Plugin SDK

`ui.canvas 1.6` 提供[原生列宽声明](src/ui/canvas/columns.rs)：透明边界、系统调整光标及绑定文本区域由原生帧更新；完成后发送带身份校验的最终宽度，提供者自行保存。移动不依赖插件回包，也不扩大权限。

公开契约的用途、权限、错误、限额、兼容与释放义务维护在 [Rust 源码注释](src/lib.rs)，供宿主和独立访客使用。多来源诊断见 [diagnostics](src/diagnostics.rs)，实验直接语言结果见 [direct](src/direct.rs)，标准 LSP 的实际能力边界见 [language](src/language.rs)，预览修复与只读源前提见 [workspace](src/workspace.rs)。

SDK 只导出源码、Cargo、WIT 与 JSON 构建输入，不包含此 Markdown 或其他仓库读者资料。

`ui.native 1.3` 的 `Node::split_proportions(weights)` 为可拖动 Row/Column 声明恢复比例；空数组保留初始尺寸并启用松手回执，非空数组必须与子项数量相同、正数且总和为 1。只有用户松手产生 `Action::SplitResized`，普通窗口缩放和恢复不会产生持久化意图；插件通过 `storage.private` 保存自己的工作区偏好。详见[原生 UI](src/ui.rs)。

`ui.native 1.2` 的 `Node::textarea_rows(min, max)` 为所有插件提供按原生可视行数自动增长的多行输入框（`1 <= min <= max <= 64`）。超过最大行数在原输入框内滚动，保留草稿 revision、IME、选择与 Undo；不得与固定像素高度并用。普通／主操作按钮复用本地公共 `Button`，主操作与插件管理安装按钮使用相同的 primary outline 样式，显式颜色和字体角色覆盖仍有效。详见[原生 UI](src/ui.rs)。

`workspace.files 1.4` 的 `Plan.conditions` 为预览和提交附加只读文档版本条件；它不授予读取权限，也不将辅助输入加入分组撤销。跨文件重命名与选中时解析的补全使用既有原生文档会话；主补全和附加导入共同校验、一次应用。契约和限额见 [workspace](src/workspace.rs) 与 [language](src/language.rs)。

`ui.canvas 1.2` 提供内容 epoch 与 `CanvasEvent::ScrollInput`：同一滚动内容的增量／绝对位置可跨越纯绘制 revision，替换内容必须更换非零 epoch。旧点击、键盘、普通滚轮和源绑定动作继续使用精确场景校验；公开使用义务见[画布契约](src/ui/canvas.rs)和[事件校验](src/ui/events.rs)。宿主可合并连续同方向增量，反向输入保留边界裁剪顺序。

`ui.canvas 1.3` 新增可选 `scroll_buffer`，插件声明已完整绘制的内容区间与上下缓冲。宿主在原生帧中平移现有画面，回包前也可滚动，并把点击坐标映射回对应绘图。缓冲耗尽时等待下一次绘图；不改变文档与滚动内容的权威来源，不适用于 grid/IME 画布或 `VisualViewport` 变换。所有插件均可协商使用。

`ui.canvas 1.4` 增加原生单行 `Paint::TextBox`，`1.5` 增加有界文字样本测量，`1.6` 增加原生列宽拖动与 TextBox 帧内预览；它们不新增权限，使用相应字段必须协商对应版本。接口义务和限制见[TextBox 定义](src/lib.rs)、[测量契约](src/ui/canvas/text_measure.rs)及[列宽契约](src/ui/canvas/columns.rs)。

`ui.canvas 1.4` 新增通用 `Paint::TextBox`：单行 UTF-8 文本在指定逻辑矩形内按实际原生字形左对齐、居中或右对齐，超宽时由宿主省略并裁剪。继承画布字体、可覆盖字体及粗体，沿用字体／文本／绘图预算并支持现有视口投影；使用该绘图项须声明 1.4。点定位的 `Paint::Text` 语义保留，所有插件使用同一接口。

`ui.canvas 1.5` 新增 `Canvas.measure_text`：提交带唯一 ID、字体、字号和粗细的单行样本组，宿主通过 `CanvasEvent::TextMeasured` 返回每组最宽样本的原生字宽，单位为变换前的逻辑像素。相同请求及字体在原生重绘间复用；结果仍受节点、模态和精确场景 revision 校验，不能沿滚动 epoch 越过旧场景。每画布最多 16 组、总计 256 个样本、64 KiB 文本，每样本最多 4096 字节；样本文本同时计入整份 UI 的文本预算。插件可据此安排紧凑列宽，主动覆盖的列宽仍由插件保留；宿主不解释样本的业务含义。

`ui.canvas 1.7` 增加互斥于旧 paint 的结构化场景，提供路径、RGBA/渐变、层级变换/裁剪、受控图片引用和实际显示回执。来源、配额及显示身份义务见[场景契约](src/ui/canvas/scene.rs)与[Canvas 事件](src/ui/canvas.rs)。

`ui.canvas 1.8` 增加宿主原生多行 Paragraph、原文位置查询、稳定目标输入/预编辑和有界语义树。原文偏移始终使用 UTF-8 扩展字素边界；OS 的 UTF-16 转换、原生临时草稿与累积确认不引入编辑器文档或领域 Undo。目标换代、完整确认、原生编辑键的显式启用和回执义务见[文字会话](src/ui/canvas/text_input.rs)、[目标输入](src/ui/canvas/targets.rs)、[位置查询](src/ui/canvas/text_layout.rs)与[语义树](src/ui/canvas/semantics.rs)。真实编辑器文本继续使用既有文档会话。

`ui.canvas 1.9` 增加场景锚定的普通原生子树、现有提示/菜单锚点及目标级拖放声明。原生控件保留普通 UI 身份、IME、键盘与语义；几何移动保留编辑会话，目标 epoch、来源或任一 overlay 祖先换代撤销旧端点。原生层只接受轴对齐等比缩放及矩形祖先裁剪，曲线、路径或非轴对齐的原生裁剪会明确拒绝；组与 VisualViewport 的组合倍率限于 `1/64..8`，OS DPI 独立。用途与失败条件见[原生叠放及锚点](src/ui/canvas/overlay.rs)。

`ui.clipboard 1.1` 与 `ui.transfer 1.0` 提供有界多格式交换：先枚举元数据，再按消费者优先级读取选定的纯文本、惰性 HTML、PNG 或包命名空间数据；已有纯文本剪贴板入口保留。剪贴板沿用 `clipboard` 权限，拖放单独要求 `ui.transfer`，原生真实手势才可发放 offer 或开始拖出。文件列表只由 OS 原生边界提供，客体获得只读导入条目和有界分块读取，不能从数据借用工作区、写入或其他插件权限。offer 的实例/目标/来源/期限和显式关闭共同约束所有读取，HTML 不执行或加载外部内容；接口义务见[传输契约](src/transfer.rs)。

HTTP、密钥、私有状态、计时器、通用树与编辑器装饰分别见 [http](src/http.rs)、[secrets](src/secrets.rs)、[storage](src/storage.rs)、[timers](src/timers.rs)、[tree](src/tree.rs) 与 [decorations](src/decorations.rs)。工作区文件名颜色和顶部入口分别协商 `explorer.decorations 1.0`、`ui.workspace 1.0`，见 [presentation](src/presentation.rs)；共享层次菜单见 [menus](src/ui/menus.rs)。

通用树的 `ui.tree 1.2` 提供 `tree::update`，原子更新缓存节点的显示和禁用状态，保留已加载页、展开、epoch 及未完成请求；结构变化使用 `refresh`，或另行协商下述 1.4 增量能力。清单须明确声明 1.2 最低版本，完整预算与回调约束见 [树契约](src/tree.rs)。

`ui.tree 1.3` 增加 `tree::initialize(tree, path)` 和 `Node.tone`：前者沿最多 64 个唯一原生节点 ID 懒加载祖先与后续页，首次定位并选中目标；不发出业务选择事件、不执行命令、不抢焦点，用户输入或完成后永久结束该默认定位。刷新和显隐不重新应用。后者提供正向绿色、负向红色和信息蓝色文字语义，原生主题负责绘制。清单 required 范围必须排除全部 1.0／1.1／1.2 补丁版本。

`ui.tree 1.4` 的 `tree::apply_patch(tree, Patch { nodes, branches })` 原子更新变化的节点元数据和已缓存目录前缀；其他已加载页、展开、选择与滚动继续保留，不先清空树。目录使用稳定节点 ID、有序 `children` 和当前 `next` 游标；未引用的旧子树被移除，新目录保持懒加载。现有 ID 不得改变目录类型或命令意图。变化补丁撤销旧回调与未完成请求，无变化补丁不推进 epoch。清单 required 必须排除全部旧 minor 补丁版本；归属、权限、图标与最终树预算仍完整检查，失败保留原树。`refresh` 继续提供明确的全量失效，旧调用语义不变。

`ui.tree 1.5` 增加 `tree::create_with_options(Options { selection: SelectionGesture::DoubleClick, ..Default::default() })`。第一次按下只聚焦，第二次原生按下才选中并发出选择事件；键盘选择、目录展开及类型化激活保留原有行为。配置在资源创建时固定，刷新和显隐不能改写。原 `tree::create()` 继续使用单击。清单 required 必须明确排除全部旧 minor 补丁版本；权限和资源配额与普通树相同。悬停色来自原生主题，菜单打开期间保留目标行的悬停背景，不改动已选项。

`ui.tree 1.6` 增加 `Node.action_menu` 和 `Options.single_click_expand`：分层菜单支持稳定动作 ID、检查标记、可折叠目录、懒准备的级联子菜单；目录可以保留单击展开，同时叶子使用双击选择。创建选项结构体时使用 `..Default::default()` 保留新增字段默认值。详情及最低版本声明义务见 [tree](src/tree.rs)。

`ui.tree 1.7` 增加可选的 `Node.checked`（未勾选／已勾选／混合）、次要文本 `Node.suffix` 和 `tree::set_expanded`。勾选只发出携带原显示状态与目标布尔值的 `Event::Checked`，提供者完成业务处理后再更新节点；宿主不猜测父子关联、不修改业务数据。点击混合态请求勾选，点击复选框不触发所在行的选择或展开。宿主检查资源、epoch、节点可用性及原状态，拒绝过期事件。批量展开只处理已缓存目录，不改变选择或焦点；折叠撤销后代的未完成请求。清单 required 范围必须排除所有旧 minor 补丁版本，权限仍为通用树原有权限。各方法的边界、上限与错误见 [tree](src/tree.rs)。

`ui.tree 1.9` 增加 `tree::reveal(tree, path)`：显式定位最多 64 个节点 ID 组成的已缓存根到目标路径，展开祖先、选中并滚动到目标，不抢焦点、不执行命令、不回发 `Selected`。每个节点必须已加载、可用且父子相连，验证失败不改变任何展开状态；可在用户先前选择之后重复调用。新原生输入、结构补丁、刷新或资源撤销取消未呈现的旧定位。清单 required 须明确排除所有旧 minor；归属和权限继续复用通用树校验。

原生树未指定 `Node.icon` 时，目录与文件显示名称使用资源管理器的图标及深浅主题优先级；显示名称不授予文件访问权。显式包内 artwork 继续遵循 `ui.icons`／`assets.read` 校验。未声明 `Command.menu` 的插件不显示空标题栏菜单按钮。

主编辑区的持续比较标签见 [文档与比较契约](src/api/documents.rs)：`editor.diff 1.1` 提供双源标签，`1.2` 允许 `OpenDocumentComparison.third` 加入第三个只读来源；第三源采用相同的归属、精确版本及撤销边界。`1.3` 的可选 `navigation` 为前后项箭头提供所属实例声明的类型化命令，参数固定于标签，并在发布时校验结构、执行时校验原始实例生命周期；详情见 [比较导航契约](src/api/comparisons.rs)。

`editor.documents 1.2` 的 `OpenDocumentSource` 可在不显示普通文件标签的情况下准备工作区原生比较源，`ReleaseDocumentSource` 释放调用方的准备。原有可见标签、未保存文本和 Undo 始终复用原会话；视图保留、32 源上限、60 秒未挂载宽限及实例撤销的清理规则见同一文档契约。

`editor.diff 1.4` 的可选 `folding` 提供双／三源无差异区段折叠。插件声明包内的折叠／展开深浅主题 SVG 和 1–20 行上下文，运行时复用 `ui.icons`／`assets.read` 的归属、64 KiB 与 SVG 校验。宿主仅折叠完全一致的显示行，保留源文本、原行号和 Undo；支持全部折叠、全部展开、单段展开与隐藏目标揭示。

`editor.diff 1.5` 增加可选 `navigation.previous_boundary`／`next_boundary` 类型化命令与 `initial_position`。用户在首／末差异处先收到 2 秒提示，再次同向操作才执行边界命令；插件用 `start`／`end` 指定相邻文件的内容顶部／底部，默认 `first_difference` 保持首次差异居中。边界命令使用普通导航相同的实例归属、命令声明和参数校验，未提供动作时不循环跳转。

默认按命令、语言、视图或服务需求启动，后台用途须明确声明，见 [activation](src/activation.rs)。结构化配置、变化通知、适用的热应用与密钥引用见 [settings](src/settings.rs)；宿主版本下限、稳定同 major 演进、实验能力及迁移义务见 [lib](src/lib.rs)。包版本、传输协议、能力版本和宿主版本分别处理。

独立 SDK 只导出本 crate 的源码、Cargo 清单、WIT 与 JSON 协议资源，不内嵌或导出 Markdown。

按文档发现和共享项目上下文使用 [language.lsp 1.5](src/language.rs)；[language.control 1.0](src/language_control.rs) 在同一受管连接上传输有界控制消息，并以宿主封印的原生文档身份及 revision 关联普通源同步。受管原生适配闭包使用 `language.modules 1.0` 与独立 `native.modules` 授权，模块不加载进宿主，也不具有 WASM 级别的相互隔离。只读项目成员分页形状见 [language_context](src/language_context.rs)；显式多提供者集合及其撤销语义见 [plugin.services 1.2](src/service.rs)。

[language.lsp 1.6](src/language_context/registry.rs) 增加按拥有者、工作区和配置代次封印的上下文引用、显式源选择与交互准备。交互声明必须使用 [固定准备契约](src/language_control.rs)，仅宿主直接准备的首个提供者可访问自己的偏好；嵌套调用继续收窄权限，取消不发布绑定。[workspace.files 1.4](src/workspace.rs) 的只读文档条件在预览和提交前校验，不增加可写文本或 Undo 栈。仅提高宿主应用版本号不能替代这些能力的完整实现和协商。

原生导航只借出一次有效来源引用，明确刷新校验原文档 revision，内部 provider 仅从界面候选隐藏；能力、权限与清理要求不变。

关联快照同时要求提供者和原始调用链具有 `workspace.read`，方法的较弱权限声明不能代替该授权。原生控制失败只有在匹配 JSON-RPC 响应携带严格的 `error.data={"version":1,"imports":"released"}` 回执时，才释放本次新增模块使用；已有成功使用及其他进行中的使用仍有效。回执表示没有仍存活的受管 worker 保留本次新增导入，不承诺代码从未执行或外部副作用已经回滚。普通错误、取消和超时无法证明释放，宿主继续保守保留可能已加载代码的授权。

`language.lsp 1.7` 允许准备钩子显式设置 `Proposal.expand_dependencies`，在依赖准备完成后解析初始化选项和配置中的完整依赖路径字符串。仅处理 `${dependency:id}/relative/path`，路径必须存在且位于本方案持有租约的依赖中；普通字符串和键保持字面含义。清单 required 必须排除 1.6 及更早版本，详见 [Proposal](src/language.rs)。

`language.lsp 1.8` 的 [register_borrowed](src/language_context/registry.rs) 注册调用方作用域的项目上下文。宿主保留提供者和原始服务调用链的实例生命周期；调用完成仍可分析，任一参与方停用、更新或撤销信任即撤销注册，旧引用和语言绑定不能继续使用。调用方应使用独立逻辑 key；同一活跃 key 不能更换到另一条调用链。普通 `register` 仍由提供者独立拥有。两者沿用既有读取权限，不增加进程或文件授权。

`language.lsp 1.9` 的 [invalidate](src/language_context/registry.rs) 在领域输入变化时撤销本实例的语言发现缓存、注册上下文及经过本实例借出的上下文。下一次发现使用新的代次，即使文档和用户设置未改变也不会复用旧计划。失效不执行领域工具，不修改文本或设置。借用元数据跟随实例、工作区和集合授权；原生准备请求的临时进程根单独清理，成功完成准备不会撤销持久元数据。

原生宿主可通过 `Manager::language_discovery_revisions` 读取当前作用域的语言实例代次，并据此替换尚未完成或失败的原文档意图。插件在成功发布新模型时也应失效一次，使等待导入期间的暂时错误不会成为新模型的缓存结果。借用方退休会同步撤销已交给原生消费者的上下文标记。

同版的 `check` 仅校验已拥有元数据 token 的作用域和存活状态，既不返回描述符也不进行文件读取。仅获进程权限的服务可据此拒绝已失效的本地缓存；它不会继承提供者的文件或编辑器读取权限。实际使用项目路径时仍经过对应能力的路径与授权检查。

1.9 同时允许单个描述符携带最多 32 KiB 的惰性元数据；较早协商版本仍限制为 4 KiB。整批注册仍限 64 KiB、128 项，并保留每实例与宿主总量限制。较大数据不会增加读取权限，也不能作为原生可执行文件或文件授权。

`tools.inspect 1.0` 提供只读工具分发元数据，详见 [process](src/process.rs)。调用需执行授权，声明服务还需对应服务授权；可读取可执行文件目录或上一级目录内的有界 UTF-8 文件，不执行版本探测、不下载、不创建文件句柄。受管依赖通过原服务声明取得租约；其他插件私有目录和越界路径均拒绝。

`language.content 1.0` 允许语言提供者声明非文件 URI 的只读内容请求，详见 [language_content](src/language_content.rs)。同时要求 `editor.virtual` 与 `editor.read`。宿主通过同一受管 LSP 取得最多 1 MiB UTF-8 内容，保留原 URI 和严格位置映射，交由现有原生只读文档呈现；不把 URI 交给文件系统或网络。结果仍校验源文档 revision 和提供者，持久视图按原始服务实例撤销。

`language.editing 1.2` 增加提供者的 `workspace_edits` 声明，详见 [language](src/language.rs)。原生手动重命名可同步同一传输中的精确原生源码快照，再把涉及其他文档的标准文本编辑提交到普通预览、权限、版本和整组 Undo 流程。每轮最多 64 份源码、合计 8 MiB，每份最多 1 MiB；不支持的资源操作和注解不会部分应用。声明必须排除 1.1 及更早版本，持有该能力不授予额外文件权限。

`language.editing 1.3` 在同一 opt-in 声明下增加标准文件重命名提案，要求独立的工作区读写权限和 `editor.documents 1.1`。原生消费者在分析前捕获磁盘版本，并把文本、文件名、标签路径及恢复交给普通文件事务；来源失效或未授权时不声明资源操作支持。仅支持已声明的 `rename`，不支持的创建、删除、覆盖及注解整项拒绝。

`run.targets 1.1` 的准备结果增加可选字面参数前缀，配置参数在其后追加；1.0 的程序名结果仍使用原方法签名。准备仍须完成构建，结果才交给后续启动；详见 [运行目标准备契约](src/targets.rs)。

`dependencies 1.4` 的 `service_preparation_hook` 在已授权方法准备前选择其已声明依赖计划的唯一子集。该有界钩子只能进行获准的只读观察，不能下载、启动进程、发布 UI 或修改持久数据；异步准备继续继承原调用权限、取消与租约。清单 required 范围必须排除 1.3 及以前版本，详见 [依赖契约](src/dependencies.rs) 和 [只读选择输入](src/service.rs)。

`process 1.9` 的 `NativeContext` 为已声明原生服务返回所属实例的包资源和私有数据路径，需 `process.exec` 及对应服务授权；原始服务调用链仍须具有执行授权。路径仅用于配置受控原生进程，不授予 WASI 文件访问，不创建进程或安装工具。未发布、退休或未协商的实例明确拒绝，详见 [process](src/process.rs)。

`debug.session 1.2` 保留全部 1.1 方法，增加可选的 `threads`、`thread_frames` 和 `variable_children`。线程、帧及变量引用绑定原始会话和暂停代次；零引用读取根变量，其他引用只可用于它被签发的线程与帧。分页最多 100 项，调用方按实际返回项数推进，`more` 表示还有数据；继续和重新暂停撤销旧引用。规范及 SDK 类型见 [debug](src/debug.rs) 和 [扩展形状](src/debug-inspection.json)。

- [SDK 源码与使用注释](src/lib.rs)
- [组件接口](wit/plugin.wit)
