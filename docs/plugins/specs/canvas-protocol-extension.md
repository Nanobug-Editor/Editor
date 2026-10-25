# 通用 Canvas 协议扩展方案

日期：2026-10-24，2026-10-25 续作。状态：已确认，实施中；阶段 A 的 1.7 已通过验收与双轴审查，在独立分支交付，B/C/D 尚未实施。用户已确认四张垂直切片工单、公开协议/实际 WASM/原生 GPUI 测试接缝和审查基点。阶段执行与真实议题见[实施工单](../tickets/canvas-protocol-extension/README.md)，后续版本仍为拟议目标，不能据此宣称整套能力已支持。

本文细化[插件平台通用接口方案](plugin-api-platform.md)的可组合 UI 原则。普通插件应能用同一公开契约实现可编辑表格、带文字编辑的流程图和大数据时间轴。插件描述内容和业务状态；宿主用 GPUI 绘制、处理原生输入并管理资源。GPUI 对象不跨 WASM 接口，宿主不按插件 ID 或领域类型分支。终端等已有消费者继续使用现行契约，不是新接口的特权使用者。

用户于 2026-10-25 要求四单仅在独立 worktree 的 `codex/canvas-protocol-extension` 分支执行，全部完成后统一合回 main；不再逐单同步或快进主工作区。各单仍按验证、审查、普通提交、推送该分支、核对关闭对应 issue 交付。

## 目标、场景与范围

| 场景 | 用户可见行为 | 所需公共能力 |
| --- | --- | --- |
| 可编辑表格 | 大量单元格滚动、列宽拖动、键盘定位、中文输入、复制多单元格与屏幕阅读器逐格读取 | 原生文字度量、稳定焦点和输入会话、语义行列、保留场景与格式化剪贴板；沿用已有列宽拖动 |
| 流程图 | 曲线连线、透明形状与渐变、缩放平移、拖动节点、框内改字、操作菜单和原生属性控件 | 结构化路径、变换/裁剪/图层、指针捕获与取消、文本位置映射、坐标锚点和原生控件叠放 |
| 大数据时间轴 | 只绘制视口附近事件，连续横纵滚动和缩放，播放指示线按显示帧更新，隐藏后停止刷新 | 双轴范围与保留区间、局部场景更新、资源复用、按需帧调度和有界事件队列 |

首批目标是 Windows 原生交互；macOS/Linux 保持现有可编译约束，平台级辅助功能、拖放和输入法要在对应平台实测后才宣称验收。方案不新增自由停靠、多窗口、WebView、内建业务布局引擎或任意插件原生代码。图表坐标轴、表格公式、流程图约束、时间轴数据索引、文本协同编辑等领域逻辑由插件实现；SDK 可以提供这些组合的可选辅助类型。

### 当前基线与需要补齐的契约

下表以 2026-10-24 工作区源码为依据，区分“已有”和“待新增”。源码和验收记录在并发变化，实施前应重新核对。

| 能力 | 已有接缝或行为 | 本方案新增部分 |
| --- | --- | --- |
| 画布绘制 | [`Paint`](../../../crates/plugin-protocol/src/lib.rs) 有 Fill、Text、单行 TextBox 和 Svg；[`Canvas`](../../../crates/plugin-protocol/src/ui/canvas.rs) 有字体、尺寸、焦点和 IME 锚点；[`CanvasView`](../../../crates/editor-app/src/ui/plugin/canvas.rs) 已在 GPUI 绘制管线内 | 结构化路径/曲线/描边/圆角、RGBA、渐变、受控图片引用、场景组和稳定绘制身份 |
| 视口与滚动 | `ui.viewport 1.0` 可缩放平移图片和非网格 Canvas；`ui.canvas 1.2/1.3` 有滚动 epoch、原生连续滚动和纵向保留区间；`1.6` 有[原生列宽拖动](canvas-native-columns.md) | 统一场景坐标映射，双轴范围与局部更新；不把已有能力重新定义为缺失功能 |
| 文字 | `1.4` 的 TextBox 负责单行裁剪、对齐和省略；`1.5` 的样本测量返回真实字宽；GPUI 原生塑形已用于绘制 | 多行换行/双向文本布局，字节位置与屏幕位置的双向映射，稳定的目标级原生输入会话 |
| 输入 | 基础键、文字、指针、滚轮和 Canvas 级焦点已有；宿主已在拖动开始后捕获鼠标并在失焦时清除合成 | 完整按键上/下、修饰键、重复、指针进入/离开/取消、多个指针身份、目标级捕获/焦点/光标与取消回执 |
| 原生 UI | [`ui::Node`](../../../crates/plugin-protocol/src/ui.rs) 可把 Canvas 与按钮、输入框等排在同一树；`Node.tooltip`、`Document.menu` 已有；图片节点和 `ui.images` 已有 | 在画布坐标内定位原生控件、为绘制目标定位已有菜单/提示、画布图片资源复用 |
| 生命周期 | `lifecycle.timers`、`ui.incremental` 的 UI 子树复用、实例资源句柄和文本 `ui.clipboard` 已有 | 对 Canvas 绘制项的局部补丁、显示帧请求、持久资源和多格式传输；定时器不冒充显示刷新同步 |

锁定依赖是 `gpui-kit = 0.8.0`（关闭默认功能，仅 `assets`）、`gpui-base = 0.8.0`（仓库补丁）和 `gpui-pre = 0.3.9`，见根目录 [`Cargo.toml`](../../../Cargo.toml)。已核对该版本的 `PathBuilder`（填充、描边、二次/三次曲线）、`Window::paint_path`、`Window::with_content_mask`、`TextSystem::shape_text/shape_line`、`Window::request_animation_frame`、原生拖放及 AccessKit 语义子节点接缝。GPUI 的 `linear_gradient` 原生只容纳两个色标；非矩形裁剪、更多色标和需要隔离的组透明度须有有界离屏/栅格适配。当前 Svg 先由 [`images.rs`](../../../crates/editor-app/src/ui/plugin/images.rs) 的 `resvg` 在后台栅格化，再交 GPUI 绘制；不能把所有场景计算称作 GPU 执行。

## 模块接缝与职责

公开接缝是 `plugin-protocol` 的 Canvas 声明、输入通知和资源操作；它只表达插件需要知道的语义。`plugin-runtime` 在发布前验证能力、权限、身份、配额和补丁顺序；`editor-app` 将通过验证的场景交给 GPUI，并把真实布局/输入映回同一场景；`editor-ui/src/<组件名>/mod.rs` 承载可复用的原生控件行为和绘制。现有 [`editor-ui` 文字适配](../../../crates/editor-ui/src/text/canvas.rs)、滚动与列宽控件继续复用。不得通过 `gpui_kit::component`、重导出或包装层引入 `gpui-component`，不 fork GPUI，也不增设仅转发调用的 crate。

| 宿主负责 | 插件及 SDK 负责 |
| --- | --- |
| 语法和配额验证、路径与文字的 GPUI 适配、裁剪和资源缓存 | 绘制项与语义项的内容、稳定 ID、领域命中含义和业务布局 |
| 精确的像素/DPI 投影、目标路由、原生焦点/IME、访问性桥接和可见性 | 文本内容、选择意图、命令处理、领域 Undo、快捷键业务含义 |
| 授权资源加载、撤销、队列背压、可见帧请求和错误归属 | 准备视口附近数据、响应帧与输入、释放不再使用的显式资源 |
| 现有 `DocumentSession` 的文本、选择、IME 和 Undo/Redo 真相 | 对真实编辑器文档只提交带版本的公开编辑请求，不复制一份可变 Rope/撤销栈 |

宿主只把通用绘制、测量、事件和原生行为藏在上述深模块后；表格、图形或时间轴的业务策略均不进入宿主。删除某个业务插件不会留下专属宿主接口。

## 拟议公开契约

以下是语义固定的接口形状，字段名是实施时的工作名称。全部新增字段可选且有默认值；旧 `Canvas.paint` 与旧 `CanvasEvent` 保持原意义。新场景与旧 `paint` 对同一个 Canvas 节点互斥，以免叠绘顺序和事件身份含糊。新版本仍通过当前协议 7 的独立能力协商，不以包版本代替能力版本。

```text
Canvas {
  paint: Vec<Paint>,                 // 原有绘制，旧包保持原样
  scene: Option<CanvasScene>,        // ui.canvas >= 1.7；与非空 paint 互斥
  native_overlays: Vec<CanvasOverlay>, // ui.canvas >= 1.9
  ...既有 font/scroll/scroll_epoch/scroll_buffer/measure_text/focusable/grid/caret
}
CanvasScene {
  identity: NonZeroU64,              // 同一内容的身份；A→B→A 也换代
  revision: u64,                     // 每次已接纳的完整场景/补丁递增
  roots: Vec<SceneNode>,             // 顺序即绘制顺序，稳定 id；组形成层级
  targets: Vec<HitTarget>,           // 独立于画笔，声明命中/焦点/光标/捕获
  semantics: Vec<SemanticNode>,      // ui.canvas >= 1.8，覆盖可操作目标
}
SceneNode = Group { id, transform, clip, opacity, children }
          | Shape { id, geometry, fill, stroke }
          | Paragraph { id, text, style, box, layout_key } // ui.canvas >= 1.8
          | Image { id, source, box, sampling }
CanvasScenePatch { panel, canvas_node, identity, base_revision, next_revision,
                   edits: Vec<Insert | Replace | Reorder | Remove>, damage: Option<Rect> }
CanvasInput { presented_revision, target_id, target_epoch, sequence,
              kind: Key | Text | Composition | Pointer | Wheel | Focus | A11yAction }
ui::Action::CanvasInput(CanvasInput)       // 外层仍是现有 UiEvent { revision, node, action }
api::Output::scene_patches: Vec<CanvasScenePatch> // 与同次 View 发布按版本原子接纳
api::Operation::Canvas(CreateImage | RequestFrame | QueryTextLayout)
api::Notification::CanvasFrame { panel, canvas_node, identity, opportunity }
```

`SceneNode.id` 只标识绘制项，`HitTarget.id` 标识可交互对象，`SemanticNode.id` 标识辅助功能对象；三者可以显式关联，不因数组索引变化而换身份。`identity` 变更即撤销旧场景的焦点、捕获、文本会话、菜单锚点、待发帧和补丁基础。重复 ID、环、悬空引用、非有限坐标和越界资源在发布前整批拒绝，旧已显示场景继续可用。组只接受普通 source-over 合成；复杂混合模式先不纳入核心契约。

### 绘制、坐标和资源

- 几何包括矩形/圆角矩形、线段、闭合/开放路径、二次与三次 Bézier；填充和描边分离，描边含宽度、端点、连接及有界虚线。颜色是明确的 sRGB RGBA（直通 alpha），旧 `u32` RGB 绘制不改线格式；渐变使用方向和有序色标（拟定 2～8 个）。两色标线性渐变优先用 GPUI 背景；多色标、复杂裁剪和隔离透明组在后台生成有界纹理并缓存，不在 UI 线程反复解码。离屏失败返回局部资源错误，不能静默丢成纯色。
- 组变换使用有限的 2×3 仿射矩阵；组合次序固定为子项本地 → 祖先组 → Canvas 内容 → 可选 `VisualViewport` → 面板逻辑坐标 → 宿主 DPI。路径点可交给 GPUI `PathBuilder`；非轴对齐图片、旋转/倾斜文字及路径裁剪若当前原语不能直接表达，必须走受预算约束的离屏适配，不能悄悄忽略变换。矩形裁剪优先用 `with_content_mask`；组 clip、opacity、hit test 和语义边界使用同一投影/交集。图层 z 顺序由场景顺序决定，组透明度是隔离后再合成，而非逐子项相乘。
- 对外坐标统一为 Canvas 本地逻辑像素，`ui.viewport` 的缩放与 OS DPI 分开。插件按内容坐标布局；宿主只在显示边缘做物理像素舍入，并返回投影与逆投影后的坐标。不可逆矩阵、溢出和过大缩放直接拒绝。原有 `VisualViewport` 与 `Canvas.scroll_buffer` 互斥约束不被暗中放宽；新双轴滚动在后续阶段单独定义其保留矩形与坐标规则。
- `Image.source` 复用现行受控图片加载和 `package.assets`、`workspace.read`、`network.images` 授权语义，绑定精确文档/文件版本；不从 SVG、HTML 或场景文本追随外部链接。生成的位图经有界资源操作取得现有形式的 `ResourceHandle`，场景只能引用同实例/作用域/面板的已接纳资源。解码缓存按来源、版本、尺寸/缩放及内容摘要标识；图形缓存按场景 ID 和内容键标识。显式 `CloseResource` 关闭访客持有的生成资源引用，禁止后续新场景再引用；已接纳场景的原生引用延续到该场景移除，避免悬空绘制。移除场景、关闭面板、失信任、停用、更新回滚与实例退休都撤销其引用和原生缓存。不能把悬空或外实例句柄交给新场景。

### 多行文字、位置映射与原生输入

- `Paragraph` 声明 UTF-8 原文、字体/主题角色、样式段、行高、宽度、换行、水平/垂直对齐及溢出策略。宿主调用 GPUI 文字系统塑形和换行，返回与 `scene.revision`、字体环境、缩放及布局键绑定的 `TextLayout`：可见行区间、各行矩形、文本 UTF-8 字节边界/字素簇到屏幕位置映射、点到最近合法边界和选区矩形。旧 `TextBox` 仍是轻量单行接口。映射按原文偏移，不以省略后的字符串替代原文；双向文本、连字、组合字符、emoji 和软换行必须用塑形结果验证。
- `HitTarget` 声明稳定 ID、非零语义 epoch、命中形状、指针策略、原生光标、Tab 次序、是否按下即捕获/聚焦及可选 `TextInputSession`。宿主同步决定捕获/焦点，不能等待 WASM 回包才开始拖动。输入通知含物理码与逻辑键、按下/释放/重复、Ctrl/Alt/Shift/Meta、文本提交、指针身份/按钮/点击数/压力可用性、进入/离开/取消、精确滚轮；没有的平台字段显式缺省。每个指针最多一个目标捕获，释放、失焦、目标撤销、弹窗接管、实例退休均发送取消并清理；退出画布仍完成原目标的拖动。
- 文本目标的原生会话以 `(实例, 面板, Canvas 节点, target id, target epoch)` 标识，保留焦点、选择、IME 预编辑和光标矩形。宿主用 GPUI 的 `EntityInputHandler`/既有本地输入行为接入 OS 输入法；插件拥有已提交文本及领域 Undo，宿主仅保存未确认的预编辑和必要的会话草稿。提交以 `text_revision` 和合法 UTF-8 范围回报，过期回执不能覆盖较新文本。GPUI 使用的 UTF-16 范围需在同一塑形结果中转换成 UTF-8 字节/字素边界。目标保持时普通场景重绘不清空中文 IME；目标删除、epoch 改变、模态接管或失信任时取消预编辑。真实编辑器文档必须继续挂接现有 `DocumentSession`，使用 `EditorState` 的唯一文本、选择、IME 与 Undo/Redo，不把段落会话当成第二份编辑器文档。
- 输入通知携带实际**已呈现**的场景 revision，而非仅已接纳但尚未绘制的 revision。宿主记录每帧的投影、裁剪和目标命中快照，并在发送时再检查实例、面板、节点、模态、来源、目标 epoch 和当前显示代。普通操作默认只在同一显示/目标代下生效；明确保留目标 epoch 的纯绘制补丁可让迟到的同目标事件通过，但不能改变源文档授权。已有 `ScrollInput` 和 `ColumnsResized` 继续使用各自的窄例外。下行事件带单调序号，插件按序处理同一手势；不能把旧目标坐标重新命中到新目标。

### 语义、原生控件与数据交换

- 语义树声明角色、双语名称/描述、状态、值、行列关系、父子关系、可用动作和关联绘制项或内容矩形。宿主用 GPUI/AccessKit 的合成子节点呈现稳定 ID，投影、裁剪、可见区与画笔一致；虚拟化内容只暴露当前可见项、必要的行列总量和可导航范围，不一次发布百万事件。辅助功能的聚焦、激活、设值、滚动等动作走同一实例/目标/revision 校验，不能借它绕过权限。原生控件保留自己的语义，画布语义不得重复广播其标签。
- `CanvasOverlay { id, anchor, rect, child: Box<ui::Node> }` 把已有本地原生控件固定在场景坐标，投影、裁剪并随焦点目标存续；它们在画布前景层绘制，控件之间有稳定 z 序，不承诺与任意矢量笔画交错。控件仅允许轴对齐定位/等比缩放；含旋转或倾斜的定位显式拒绝，插件可改用场景图形。原生输入、Textarea、按钮等仍沿用既有事件、主题和 i18n；控件移除后焦点返回策略由插件声明且经宿主确认。绘制项的提示和菜单只新增 `CanvasAnchor { target, scene_revision, rect }` 到现有 `Node.tooltip`/`Document.menu` 的定位适配；宿主负责投影、窗口边缘夹持、关闭与模态输入，不建立第二套菜单。
- `ui.clipboard 1.1` 在已有纯文本读写上增加按需枚举/读取/写入格式化项；独立 `ui.transfer 1.0` 为画布及普通原生节点提供统一拖放 offer、进入/移动/离开/放下/取消和受控读取。最小格式集为 `text/plain;charset=utf-8`、惰性 `text/html`、`image/png` 和包命名空间的自定义数据；文件拖入以宿主发放的有界导入句柄表示，不把原始路径直接变成工作区权限。HTML 当惰性数据处理，宿主不执行脚本、不自动加载外部资源；图像仍走现有格式/尺寸检查。系统剪贴板权限沿用 `ui.clipboard`；外部文件只能经该次用户拖入发放的受控句柄读取，写入工作区仍按现有文件权限逐次检查。offer 有格式、字节数、来源和有效期，只有用户实际请求才读取数据；拖出到其他应用走 OS 能支持的相同安全格式。跨插件数据不继承来源插件权限。

### 显示刷新、补丁与背压

- `ui.incremental` 的 `ViewPatch` 继续复用**完整 UI 子树**；它不会拆分 `Canvas.paint` 数组，因此新增 `CanvasScenePatch` 才能只更新局部绘制项。补丁以已接纳 `identity/base_revision` 为前提，整批原子应用；失败保持旧场景并返回 `Conflict`/`StaleRevision` 及可重发的当前版本。删除父组会撤销子项、目标、语义和关联资源。`damage` 只作优化提示，宿主计算的联合脏区必须覆盖实际变化；尺寸、主题、DPI 和字体变化可强制重建缓存。旧全量 `View` 与新场景补丁不能互相伪造基础版本。
- 插件按需请求 `CanvasFrame`，宿主在 GPUI 下一次可见帧机会发送一次 `FrameOpportunity`（含帧序、逻辑视口、实际间隔和可见性），同一 Canvas 至多保留一个待发请求。`Window::request_animation_frame` 提供调度接缝，不承诺物理 vblank、固定 16 ms 或每请求一帧。没有场景变化就不重绘；隐藏面板的装饰动画暂停，任务完成/取消照常送达。已有 `lifecycle.timers` 保留业务定时与防抖用途，不用 10 ms 定时器模拟显示刷新。
- 在途场景最多保留有限个已接纳/已显示版本。相邻目标移动和同帧绘制属性可合并，最新全量场景可替换尚未接纳的旧全量场景；结构补丁要么按 base 顺序提交，要么显式要求全量重同步，不能跳过中间补丁后谎报成功。指针移动可在同一捕获目标/修饰键/epoch 内保留最新位置；连续同向滚轮按已有规则合并；键、文本、IME、按下/释放/取消、拖放完成和辅助功能动作绝不静默丢弃。队列超限要拒绝生产者、暂停或取消手势并明确报告。场景输出需要独立的编码与展开预算，建议与现行宿主请求 `MAX_REQUEST_BYTES` 的 2 MiB 量级相当；不能把该常量误当成当前 `Output` 的既有限制，也不能让小补丁解引用成无界原生分配。
- 后续新增 `SceneScroll2D { content, offset, buffer_rect, epoch }`，沿用原生滚动求解器，将 X/Y 本地移动与同一个场景身份关联。插件保留视口周围的矩形 overscan；宿主只在已绘区域内逐帧平移，不生成不存在的事件。旧纵向 `ScrollRange/ScrollBuffer` 和 `VisualViewport` 不被暗中组合；新能力明确规定与缩放同时使用时先内容滚动、后视口缩放的顺序和输入逆映射。

## 能力协商、兼容和失败语义

| 拟议能力 | 增量契约 | 旧消费者行为 |
| --- | --- | --- |
| `ui.canvas 1.7` | 可选 `scene` 的图形组、路径、变换、渐变、图片、命中基础；显示身份 | `paint` 和 `CanvasEvent` 原样；使用新字段的包必须声明 >=1.7 |
| `ui.canvas 1.8` | 多行文字与位置映射、目标级完整输入/IME、语义树 | 未声明者仍用 Canvas 级焦点、单行 TextBox 和原键鼠事件 |
| `ui.canvas 1.9` | 场景坐标原生控件、提示/菜单锚点和拖放目标 | 普通 `Row/Column` 混排、既有菜单/tooltip 不变 |
| `ui.canvas 1.10` | `CanvasScenePatch`、帧请求、保留资源和双轴滚动 | 全量绘制、原纵向滚动/epoch 保持可用 |
| `ui.clipboard 1.1`、`ui.transfer 1.0` | 格式化剪贴板和通用拖放；独立权限、资源与上限 | 原纯文本剪贴板和已有文档图片输入不变 |

上述版本仅在对应阶段实际交付且通过验收后登记为可用。required 能力在激活前缺失则拒绝；optional 缺失时 SDK 只使用已协商的旧路径，不能发送新字段再赌宿主忽略。所有新类型使用有界 `serde` 结构并拒绝未知危险字段；破坏旧字段含义时应启用新主版本，而不是悄悄改变 `1.x`。旧包、旧已安装私有数据和旧场景标识不做清空迁移；面板更新沿用平台两阶段切换与失败恢复。新 SDK 只打包 `plugin-protocol` 允许的源码/WIT/JSON，接口用法最终写在定义旁注释，本文只存维护者方案。

初始预算建议：单场景最多 32,000 绘制节点、128,000 路径段、16 层 clip、32 层组变换、4,096 个**可见**语义节点；单次补丁不超过 2 MiB、单资源沿用现行 8 MiB 编码和 4096×4096 解码限制，同一实例的可驻留纹理设定可观测上限。数值须在第一阶段用三类夹具和现有 `Canvas.validate` 配额校准，再作为测试覆盖的策略常量固定；不能用“限制已定义”替代性能实测。失败区分 `CapabilityUnavailable`、`PermissionDenied`、`InvalidRequest`、`InvalidHandle`、`StaleRevision`、`Conflict`、`LimitExceeded` 和 `OperationFailed`。无效补丁不破坏旧画面；资源局部失败显示可访问的失败占位；反复超限沿现行实例故障策略处理。

## 实施阶段与验收门槛

每阶段跨协议、运行时、宿主、SDK 和真实插件消费者形成一条可使用的能力切片。共同前置：记录当前 Cargo 源码/锁文件与 GPUI 接缝，保存旧包和旧 `Canvas` 回归夹具；按[项目验收约定](../../../AGENTS.md)跑受影响定向测试、`cargo fmt --check`、`cargo test --workspace --exclude editor-app`、`cargo check --workspace`，修改 `editor-app` 再跑相应过滤测试与 Windows 原生验收。实际 WASM 包的 ignored 测试必须先直接构建夹具再显式运行。阶段完成证据记入 `docs/plugins/verification/`；文档提案不预先标记通过。

| 阶段与依赖 | 可交付的普通插件能力 | 主要修改位置与兼容策略 | 必过门槛 |
| --- | --- | --- | --- |
| A. 场景图形与空间操作（现行 1.6 基线） | 流程图能显示曲线、圆角、透明/渐变、图片和多层缩放画面，并在已有指针/视口入口下选中对象 | `plugin-protocol/ui/canvas`、`plugin-runtime` 验证/图片资源、`editor-app/ui/plugin/canvas` 和 `images`、必要的 `editor-ui` 图形适配；增加 `ui.canvas 1.7`，旧 `paint` 不变 | 三种图形/图片和深浅主题原生像素对照；裁剪、DPI、旋转/复杂渐变回退、失效图片/超额资源与旧包兼容测试；不新增插件专属分支 |
| B. 可编辑目标与语义（A） | 流程图内改字，表格可键盘逐格导航、中文 IME 输入和被屏幕阅读器识别；拖动目标失焦能取消 | `plugin-protocol/ui/canvas` 文本/输入/语义类型，`editor-app` 输入会话和 AccessKit 桥接，`editor-ui/text`；增加 `ui.canvas 1.8`，沿用旧 Canvas 级事件 | 中英/emoji/RTL/软换行点位往返、UTF-16↔UTF-8、候选窗位置、连续重绘不断 IME；键上/下、重复、捕获/取消、模态与卸载；Windows 原生辅助功能焦点/值/行列/动作实测 |
| C. 原生混排与交换（B） | 表格可复制粘贴 TSV/HTML、拖动行列和图片；流程图可在节点上弹出输入框、菜单和提示，跨应用安全拖放 | `ui::Node`/Canvas overlay 遍历与校验、`editor-ui` 原生控件、宿主菜单/图片/剪贴板和运行时资源；增加 `ui.canvas 1.9`、`ui.clipboard 1.1`、`ui.transfer 1.0` | 控件与画布坐标、滚动/缩放后锚点、焦点返回与遮挡；多格式优先级、取消/失效/越权文件导入、HTML 惰性、OS 拖入/拖出；旧纯文本及文档图片入口不回退 |
| D. 保留场景与显示帧（A～C） | 时间轴在大量数据下连续双轴滚动/缩放、局部刷新与播放；表格/流程图只更新变化区域，隐藏后静止 | `api::Output` 场景补丁与请求、`plugin-runtime` 原子应用/背压、`editor-app` 场景缓存和帧调度、`editor-ui` 双轴滚动；增加 `ui.canvas 1.10` | 下面的性能基准、乱序补丁/ABA/撤销/回滚、热更新资源清理、输入不丢、旧滚动和列宽拖动；三个真实插件都只调用公开接口，并完成 Windows 原生端到端验收 |

阶段 A 的配额/场景身份是 B～D 的接缝前提；B 的目标/语义身份是 C 的原生控件与拖放前提；D 可先在 A 后开发内部原型，但对外验收需覆盖 B/C 的同一事件和资源生命周期。每阶段独立版本协商，前一阶段未交付不得以新版本号占位。正式工单和 issue 映射在方案确认后按 `docs/plugins/tickets/` 另行建立；本文没有关闭或发布授权。

### 共用验收矩阵与性能证据

三个插件均通过实际安装包与公开管理器进入宿主，不新增测试专用宿主接口。可编辑表格检验中文 IME、键盘/Tab、行列语义、列宽与 TSV/HTML 剪贴板；流程图检验多形状、变换、目标捕获、文字位置、原生叠放、菜单/提示和拖放；时间轴检验 100,000 条逻辑事件但只发布可见区及 overscan、双轴连续滚动、帧请求、补丁与资源回收。每个场景还要测深浅主题、125%/200% DPI、缩放/滚动组合、隐藏/关闭/失信任/更新失败，以及既有终端、Git Canvas 和图片预览回归。

性能用固定 Windows 参考机、Release/profile、屏幕刷新率、DPI、插件包版本和数据种子记录基线；同机对比旧全量路径。建议门槛是 60 Hz 下交互滚动/拖动的宿主帧 p95 ≤16.7 ms、p99 ≤33.3 ms，输入到可见反馈 p95 ≤50 ms；长时间静止或隐藏场景每秒 0 个由自身请求的重绘帧。10 分钟往返滚动后记录场景/纹理驻留、WASM→宿主字节数、重新塑形/栅格次数和队列峰值，内存回到有界稳态且无持续增长；局部单项更新不得序列化整个 100,000 项数据集。若参考机达不到目标，需保存原始采样、瓶颈和具体取舍，不能用编译通过或模拟帧证明性能达标。

## 主要取舍与后续可选能力

| 取舍或风险 | 当前方案 |
| --- | --- |
| 直接暴露 GPUI 对象会锁定实现和 WASM ABI | 只暴露值类型场景与事件；宿主可替换 GPUI 适配，不向插件借用原生对象 |
| 复杂效果可能耗尽 UI 线程或纹理预算 | 两色标/矩形剪裁优先走原生路径，复杂效果后台有界栅格化、缓存与局部失败；性能证据决定具体预算 |
| 每次重绘替换目标会丢焦点/IME或误送旧事件 | 绘制、目标、语义分别用稳定 ID/epoch；输入绑定已显示版本和原目标，撤销先于复用 |
| 大型场景全量更新成本高 | UI 子树仍走现有 `ui.incremental`，Canvas 内增补原子补丁与可重同步机制；可见区虚拟化仍由插件决定 |
| 原生控件任意旋转和与笔画交错会破坏输入/辅助功能 | 前景、轴对齐原生叠放作为明确契约；旋转视觉内容留在场景绘制，输入控件使用正常原生矩形 |
| 任意剪贴板/拖放格式会扩大发送与文件权限 | 格式与大小白名单、惰性读取、来源句柄、独立协商和逐次授权；旧文本路径保留 |

首批不承诺 3D、视频、任意着色器、非普通混合模式、文本沿路径排版、富文本原生共同编辑或跨进程共享 GPUI 纹理。这些可按真实普通插件需求增加独立契约，不用终端/图表/白板专属入口。当前没有阻止阶段 A 的产品决策待用户补充；若将来要求**任意角度的原生输入控件**或**无白名单的系统数据格式**，须单独确认交互与安全范围。
