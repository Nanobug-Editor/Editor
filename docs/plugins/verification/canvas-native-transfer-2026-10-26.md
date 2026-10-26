# Canvas 03：原生混排与数据交换验收记录

状态：实施中，尚未交付或关闭 [#130](https://github.com/Nanobug-Editor/Editor/issues/130)。依据为 [工单 03](../tickets/canvas-protocol-extension/03-native-transfer.md) 及 [Canvas 扩展方案](../specs/canvas-protocol-extension.md)。

## 执行位置与恢复检查点

- 工作区：`C:/Users/Tmiracle/.codex/worktrees/canvas-prerequisite/Editor`。
- 分支：`codex/canvas-protocol-extension`；本轮恢复时本地与 origin 均为 `823f089c7321e07104c2dc6db616f8a31035d6b7`。
- 前置 #128、#129 已交付；本单与 #131 完成并验收之前不合并 main。审查固定点继续为 `561aa5b2006c38668a083482156646a16cd02b3e`。
- 测试接缝沿用公开协议/管理器、实际 WASM 包、原生 GPUI。全部变更留在本工作区，保留其他工作区及会话的改动。

## 原生叠放第一切片

1. 公开协议红测：`cargo test -p plugin-protocol canvas_overlay_keeps_native_children_in_the_public_tree --offline --locked --jobs 2`。`target/canvas-c-overlay-red.log` 为 0 通过、1 失败，失败原因为未知 `overlays` 字段。
2. 同一入口绿测：`target/canvas-c-overlay-green.log` 为 1 通过，验证原生子控件参与普通 UI 身份校验、事件路由和能力需求 `ui.canvas 1.9`。此处仅报告类型需求，尚未登记宿主能力可用。
3. 原生红测：按 `target/canvas-c-native-overlay-red-build.log` 的实际测试可执行文件运行 `native_canvas_scene_paints_transformed_shapes --ignored --nocapture --test-threads=1`，设置 `NANOBUG_CANVAS_SCENE_CASE=custom` 和 `NANOBUG_CANVAS_SCENE_INPUT` 指向 `scene_test_data/native-overlay.json`。`target/canvas-c-native-overlay-red.log` 为 0 通过、1 失败，失败点为原生输入框尚未实际绘制。
4. `target/canvas-c-native-overlay-green.log`：同一原生入口 1 通过。真实点击聚焦、组移动后保持同一输入实体及中文预编辑、一次普通 `Change("中文")`、显示回执 `[1,2]` 及鼠标按下不穿透均通过。初次运行曾暴露测试对输入框内边距和两次显示回执的错误假设，已按既有本地 Input 与实际场景版本修正后重跑。
5. `native-overlay-epoch.json`：红测 `target/canvas-c-overlay-epoch-red.log` 在目标 epoch 更换后仍复用旧原生输入实体；绿测 `target/canvas-c-overlay-epoch-green.log` 为 1 通过。重建从新目标声明的值开始，保留的旧端点不能向新目标投递文本。
6. `native-overlay-return.json`：红测 `target/canvas-c-overlay-return-red.log` 为缺少 `focus_return` 契约；绿测 `target/canvas-c-overlay-return-green.log` 为 1 通过。宿主确认移除的原生子树确实持有焦点，返回策略等待下一次成功原生显示，且只允许同 Canvas 内仍可见、可聚焦的活目标；原生方向键确认返回路由。
7. `native-overlay-scale.json`：深色主题、125% DPI、2 倍场景组与 0.75 倍 VisualViewport。红测 `target/canvas-c-overlay-scale-red.log` 显示原生边框/留白未随场景缩放；修正沿用输入框原有半 rem 留白并缩放边框后，`target/canvas-c-overlay-scale-green.log` 为 1 通过。实际内框几何、移动、预编辑及提交均通过。
8. `native-overlay-tab.json`：`target/canvas-c-overlay-tab-red.log` 捕获原生输入框 Shift-Tab 不能进入场景目标。直接 `Window::focus_prev` 对照同样失败，确认不是仅缺按键绑定。本地 Input 语义框与 Base 编辑根重复登记同一焦点 ID；简单关闭其中一个句柄会通过共享句柄元数据一起移除编辑根的 Tab 停靠点，`target/canvas-c-overlay-tab-probe-next.log` 记录了这一结果。改用公开 `View`、`Render` 和类型擦除元素适配，将语义放回实际编辑根并保留单一焦点登记后，`target/canvas-c-overlay-tab-body.log` 为 1 通过，正反向原生键盘遍历均通过。正在清除临时诊断并复跑相关原生回归。

## 待完成验收

本地输入框适配回归：`target/canvas-c-native-find-green.log` 与迁回控件自身 `scale` 接口后的 `target/canvas-c-input-scale-find.log` 均为 1 通过，覆盖普通源编辑器查找输入与焦点返回。第一次适配遗漏测试观察器在实际编辑根上的焦点登记，已修正为同一元素上的单次登记。

阶段检查点（后续锚点及传输改动尚未包含）：`target/canvas-c-native-workspace.log` 为 549 通过、387 ignored；`target/canvas-c-native-check.log` 编译通过；`target/canvas-c-native-app-plugin.log` 为 77 通过、32 ignored。上述 ignored 不算通过。

原生追加回归：`target/canvas-c-overlay-hover.log` 为 1 通过；多按钮捕获的红测 `target/canvas-c-overlay-capture-red.log` 显示右键按下会被前景输入框截走并取消旧 Canvas 捕获，补齐同一捕获的窗口捕获阶段 Down 路由后，`target/canvas-c-overlay-capture-green.log` 为 1 通过。完全裁出视口的输入框仍进入 Tab 的红测为 `target/canvas-c-overlay-clip-red.log`，按可见矩形决定原生元素挂载后 `target/canvas-c-overlay-clip-green.log` 为 1 通过，输入实体仍保留。

冻结 B 包回归：`target/canvas-c-wasm-table-probe.log` 为 1 通过（46.43 秒），`target/canvas-c-repeat2-wasm-flow-editable.log` 为 1 通过（37.08 秒），均覆盖 OS 中文输入与 Windows UIA 设值/读回。首次表格等待超时原因未定；流程图一次被前台窗口保护拒绝，核对该外来窗口为 Edge，未放松发送输入的 HWND 校验。这里仅证明不可变 B 包回归，不能替代尚未生成的 C 包。

提示/菜单锚点协议红绿日志为 `target/canvas-c-anchor-protocol-{red,green}.log`；新增锚点保留既有菜单与纯文本 tooltip，且校验坐标预算。原生菜单红测 `target/canvas-c-anchor-native-red.log` 观察到旧面板坐标，`target/canvas-c-anchor-native-green.log` 为 1 通过。`target/canvas-c-green-native-tooltip.log` 为 1 通过（7.07 秒）：先确认实际绘制，再对新锚点悬停，验证移动后关闭旧提示、新位置显示、边缘夹持及 epoch 撤销。最初用 Base 观察器查询未注册的 Tooltip 根，测试无法观察实际内容，现用原有本地 Tooltip 内容的标准观察器校验其真实可见布局。

剪贴板协议红绿日志为 `target/canvas-c-clipboard-wire-{red,green}.log`、`target/canvas-c-clipboard-bounds-{red,green}.log`，两项测试通过。`target/canvas-c-green-native-clipboard.log` 为 1 通过：四种格式精确往返，HTML 脚本/外链只保留字节，自定义尾零不受 Win32 内存填充影响，普通 GPUI 文本入口读取中文 TSV，剪贴板替换后旧序号读取拒绝。原生红测发现命名空间检查错误地把 MIME 前缀中的斜杠纳入后缀字符检查，修复后通过。格式枚举不获取数据句柄；IO/PNG 校验在后台运行。运行时登记 ui.clipboard 1.1、ui.canvas 1.9，资源限定原实例/作用域及 30 秒期限，显式关闭和退休撤销原生读者。

实际 C 流程图包已提升源码版本至 1.2.0，B 冻结 ZIP 保持不变。首次公开 `--plugin-cargo` 构建 `target/canvas-c-flow-build.log` 捕获 SDK 显式清单漏掉 `transfer.rs` 与 `ui/canvas/overlay.rs`；已补齐源码及其 cfg(test) 模块。`--plugin-cargo` 只接收 build/check/test；独立 Cargo 元数据通过本次导出的 SDK 配置核对工作区和独立输出目录，再经宿主入口构建，不使用仓库 path 依赖。新版实际包验收仍在进行，未冻结登记产物。

补充切片：

- `native-overlay-hide.json` 的红测 `target/canvas-c-overlay-hide-red.log` 捕获没有新帧的隐藏/恢复仍复活旧编辑端点；`target/canvas-c-overlay-hide-green.log` 为 1 通过，已确认插件值保留，旧预编辑与旧端点撤销。原生撤销先断开编辑订阅，再结束原窗口焦点，避免结束组合时投到替换控件。
- 公开子树增量红/绿：`target/canvas-c-overlay-incremental-{red,green}.log`；已有 `ui.incremental` 现在可独立复用叠放原生子树，场景 revision 变化不要求把子控件送入绘制协议。
- 原生布局预算红/绿：`target/canvas-c-overlay-budget-{red,green}.log`；原先 1024 倍组缩放会进入控件布局。现在 `Document.validate` 限定组与 VisualViewport 组合后的原生缩放为 `1/64..8`（OS DPI 独立），并校验内容/显示投影矩形的百万逻辑像素预算。旋转、倾斜、反射及非均匀缩放仍明确拒绝；图形自身的缩放范围不因此改变。

实际 WASM 管理器切片：`target/canvas-c-transfer-manager-green.log` 为 2 通过（209.08 秒），包括剪贴板元数据优先级/外实例隔离，以及原生拖放进入/移动不读取、放下后只加载所选 HTML。外部数据生产者只在公开 `TransferReader` 原生边界计数，未模拟内部管理器。`target/canvas-c-import-wire-{red,green}.log` 记录保留文件列表 MIME 的协议红绿：格式选择允许文件列表，任意字节项不能伪造路径，文件读取携带 offer 及条目、偏移和有限长度。

`target/canvas-c-import-manager-green.log` 为 1 通过（144.59 秒），实际 1.2.0 候选包经公开管理器获得只读导入句柄、文件名和大小，再按偏移读取；外实例和显式关闭后的读取拒绝。原始路径只存在于原生生产者，未进入客体 JSON。候选 ZIP 为 757954 字节、SHA-256 `7df7ed11961884dba611ab2cffa7ce1d724cb891152916b1bdadb78667c81cbf`，SDK 内容摘要 `5e0d4467d4e697e1ee4d4b3c066ace801e35d3ad7ac72b0d8c772d168b84b63e`；仍未冻结到账本。阻塞生产者取消、超额数据及 OS 原生拖入/拖出尚在补验。

阻塞生产者取消红测 `target/canvas-c-import-cancel-red.log` 在 `CloseResource` 超过 1 秒时失败。原生文件生产者/文件 IO 不再占用授权注册表锁后，`target/canvas-c-import-cancel-green.log` 为 1 通过（71.26 秒），关闭及时完成，迟到文件列表不能恢复导入授权。

原生 OLE 切片：`target/canvas-c-ole-native-green.log` 为 1 通过，真实 IDataObject 格式枚举不调用 GetData，后台通过 COM GIT 读取单个惰性 HTML，撤销后不再调用生产者。新增直接依赖仅为已锁定的 `windows-core 0.62.2`，Cargo.lock 只新增该依赖边，没有升级任何包。

真实系统拖动最初两次超时，保留 `target/canvas-c-ole-lifecycle-green.log`（名称源于原预期，实际结果失败）和 `target/canvas-c-ole-diagnostic.log`。临时最小 Win32 窗口将问题缩小到独立源 STA 未连接所属窗口输入队列；连接后进入/移动/放下恢复。修复使用本进程 HWND 校验及 RAII AttachThreadInput/断开，窗口目标明确为非 agile COM 对象，IDataObject 仅包含可跨线程的只读数据生产者。`target/canvas-c-ole-fixed.log` 为 1 通过（1.95 秒），回到原 GPUI 窗口验证真实系统拖动及选定格式读取。

`target/canvas-c-native-drop-node-green.log` 为 1 通过（2.12 秒），实际系统拖动进入普通 Node 声明的区域，输出正确节点局部坐标和进入/移动/放下顺序，全程不生成载荷。窗口共享 OLE 注册保留至 HWND 关闭；视图的来源、overlay 代际和可见性单独撤销。非通用目标的文件拖放回到既有 GPUI 文件/图片入口，专门回归仍待完成。

Canvas 命中红/绿为 `target/canvas-c-native-drop-canvas-{red,green}.log`。绿测 1 通过（2.30 秒），真实系统拖动命中带组变换的路径，入口携带显示场景 62/revision 1、receiver/epoch 4 和 Canvas 内容坐标；使用同一成功原生输入帧的路径、裁剪与逆投影。

实际源管理器：`target/canvas-c-native-source-manager-green.log` 为 1 通过（70.74 秒），实际流程图声明的 TSV/HTML/本包命名空间数据只经原生入口导出，客体不能读取私有源 offer，取消通知只出现一次。对应候选 SDK 摘要为 `7473e1f2eda9203df18850377ecf739dbb5cc3829d42fcf1369c43ae7d938bf7`。完成后的延迟读取红/绿日志为 `target/canvas-c-native-source-delay-{red,green}.log`，绿测 1 通过（68.81 秒）：完成通知先封口，成功复制仅保留原 30 秒数据租期，取消立即撤销，显式关闭仍能结束成功后的数据租期。

`target/canvas-c-outgoing-native-{red,green}.log` 验证同一实际包的原生 Source 按钮。红测源包未收到 OS 完成；添加实际左键按下及 4 像素移动阈值、原实例管理器签发和后台 STA 后，绿测为 1 通过（79.65 秒）。第二个受控 HWND 接收真实系统拖出，并在源包收到一次完成通知后再读取原 HTML，未把内部消息注入视为 OS 拖动。

独立目标目录 `target/canvas-c-protocol-check` 的 `target/canvas-c-protocol-suite.log` 为 103 通过、0 ignored（包含 Source 完成类型加入前的协议快照，后续门禁仍需重跑）。构建前元数据已核对本工作区及该独立输出目录。

`target/canvas-c-outgoing-cancel-red.log` 复现延迟授权被后续物理点击重新激活：旧手势已释放，700 毫秒后才到达的源租约向第二次点击导出了旧数据。给待启动请求增加独立手势存活标记，左键释放、视图撤销与新源按下即撤销，成功后的数据租约仍独立保留。`target/canvas-c-outgoing-cancel-green.log` 为 1 通过（76.03 秒）。

表格消费者源码提升至 1.1.0，原 B 的 1.0.0 冻结 ZIP 未变。`target/canvas-c-table-copy-{red,green}.log` 的绿测为 1 通过（57.14 秒），验证实际表格单元格生成一致 TSV/HTML，经普通原生 Clipboard 请求写入后收到成功状态。`target/canvas-c-table-paste-{red,green}.log` 的绿测为 1 通过（77.02 秒），先枚举、优先选择 HTML、导入 `A &amp; B` 与中文为普通客体单元格值，并关闭原 offer；HTML 从不进入渲染器。构建通过宿主 --plugin-cargo，独立目录 `target/canvas-c-table` 的元数据已核对。

此处是当时的阶段检查点，后续切片和审查结果见下文。AC1–AC6 仍待最终 SDK/包冻结、冻结字节复验和提交后的树一致性检查，工单尚未关闭。

## 目标、显示帧与消费者追加切片

- `target/canvas-c-source-frame-manager-green.log` 为 1 通过（70.48 秒）：公开管理器允许使用最近实际显示的 Canvas 源帧，拒绝借该帧导出已替换的源表示。原生 Canvas 拖出红测 `target/canvas-c-canvas-source-red.log` 未进入源手势；接入同一命中路径、原 UI revision 与目标 epoch 后，`target/canvas-c-canvas-source-probe-live.log` 为 1 通过（73.93 秒），OS 接收窗口在完成后读取原 HTML。诊断记录准入约 18 毫秒、资源/手势有效和 OS Dropped；临时日志随后删除。
- 原生拖出首次复测 `target/canvas-c-canvas-source-green.log` 实际失败，接收方没有数据；其后一次 `target/canvas-c-source-final-native-transfer-canvas-outgoing.log` 触及 90 秒子进程上限。测试现在等待真实 OLE Enter 后再释放成功手势，并为阻塞式冷组件编译及原生部分设置 180 秒总上限，保留原 700 毫秒延迟下提前释放的取消用例。`target/canvas-c-source-composed-native-transfer-canvas-outgoing.log` 为 1 通过（94.42 秒），`target/canvas-c-source-composed-native-transfer-outgoing-cancel.log` 为 1 通过（87.82 秒）。这些记录不能替代后续最终快照回归。
- `target/canvas-c-current-drop-frame-{red-build,green}.log` 的绿测为 1 通过（85.74 秒）：同一 offer 在目标移动和新帧显示后，Drop 使用新显示版本/内容坐标，仍绑定原目标 incarnation。
- 表格粘贴一次撤销红/绿为 `target/canvas-c-table-paste-undo-{red,green}.log`，绿测 1 通过（57.24 秒），整批单元格属于一次客体 undo。`target/canvas-c-table-roundtrip-{red,green}.log` 先捕获混合 th/td 跳过表头，绿测 1 通过（58.26 秒），HTML 原顺序以及 TSV 引号内制表符、引号和换行保留。新增 C 测试使用 1.1.0 默认包，B 默认 1.0.0 不变。
- `target/canvas-c-table-drag-{red,green}.log` 的绿测为 1 通过（58.71 秒）：实际表格声明行/列命中形状和 TSV/HTML 源表示，拖入一行经公开选定读取写入原行，再一次 undo 恢复。PNG 运输节点已加入候选源码，完整实际消费/原生验证尚未完成。
- `target/canvas-c-nested-owner-{red,green}.log` 的绿测为 1 通过（88.08 秒）。原先声明查找遗漏嵌套 Canvas，并把未变的内层 scene/target 当成同一原生所有者。现在完整 overlay 祖先链约束帧、编辑 draft/focus 和传输租约；外层 epoch 替换立即撤销内层源，旧输入拒绝，旧清理只封口旧凭据，不能清掉新焦点。
- `target/canvas-c-anchor-hidden-group-{red,green}.log`：世界坐标锚点原先能绕过目标组透明/隐藏状态；公开协议修正后 1 通过，坐标空间只决定位置，所有者组仍约束可见性和活动原生子树。
- `target/canvas-c-native-leave-{red,green}.log` 的绿测为 1 通过（71.62 秒）：OS 在 actor 处理 Leave 前撤销 IDataObject 时，终止路径仍封口并通知一次，不能借终止处理恢复载荷读取。
- `target/canvas-c-drop-occlusion-red.log` 复现前景原生输入框下的 Canvas 接收拖放；按实际普通子树及 overlay 的前景顺序保留惰性阻挡矩形后，`target/canvas-c-drop-order-native-transfer-canvas-occlusion.log` 为 1 通过（2.06 秒）。

文件拖入组合最初因普通 Text 没有本地控件观察器而未定位接收区，尚未开始 OS 拖动，不能计通过；候选接收区改用普通本地 Button，保留相同通用声明。旧 GPUI 文件消费者的对照发现接收器被前景 Canvas 遮挡；接收器按真实前景布局设置后，原入口和新 OLE 注册入口分别通过（`target/canvas-c-latest-native-transfer-legacy-control.log`、`target/canvas-c-stage-native-transfer-legacy.log`；早期失败记录保留）。新适配按 GPUI 的下一帧回调顺序发送 Enter 和 Submit，恢复实际文件/图片入口。

## C 源码冻结前的完整检查与审查修复

- `target/canvas-c-manager-suite.log`：实际候选流程图包的公开管理器测试 10 通过，897.99 秒；覆盖惰性读取、实例隔离、文件导入与关闭、阻塞生产者取消、显示帧、嵌套所有者和源完成后的有限租期。`target/canvas-c-table-final-suite.log`：实际表格交换测试 5 通过，292.48 秒，包含整批粘贴一次撤销、TSV/HTML 边界、行列拖放与 PNG 字节往返。
- `target/canvas-c-stage-workspace.log`：559 通过、402 ignored；`target/canvas-c-stage-check.log` 编译通过；`target/canvas-c-stage-app-plugin.log`：77 通过、32 ignored。ignored 均不计通过；后续审查修复后另跑最终门禁。
- 实际 C 包中文 IME 和 Windows UIA：`target/canvas-c-composition-wasm-c-flow-125-dark.log`、`target/canvas-c-composition-wasm-c-table-200.log` 各 1 通过，覆盖 125% 深色流程图及 200% 表格。原生菜单、锚点、Tab、缩放、拖放遮挡和原文件入口的六项阶段回归均通过。
- `target/canvas-c-table-native-source-red.log` 捕获实际表格遗漏 OS 完成通知；处理通用 `DragFinished` 后，`target/canvas-c-final-native-transfer-table-outgoing.log` 为 1 通过（61.39 秒），真实行拖出后接收窗口读到原 TSV/HTML。
- Windows 源 PNG 使用既有输入格式/尺寸校验；`target/canvas-c-binary-native-transfer-outgoing-png-limit.log` 为 1 通过（81.68 秒），正常源 `target/canvas-c-binary-native-transfer-outgoing.log` 为 1 通过（81.44 秒）。超额 PNG 不进入原生数据对象消费者。

固定点双轴初审使用不可变 Git tree `643a7ec73cc8a8742c4c6192f168c1d7e3b5bdd9`，固定点仍为 `561aa5b2`，C 差异另与 `823f089c` 比较。发现并修复三项 P2：

1. 点号包名可冒用其他包的自定义 MIME 前缀。格式名限定为最后一个点号段，包名允许点号；公开协议红绿测试验证 `foo` 不可写入 `foo.bar` 的格式。
2. 枚举剪贴板失败留下客体未收到的 offer。非成功枚举终态先释放资源再通知客体，已有 offer 的读取失败保留原授权；实际管理器连续 20 次失败后仍可枚举，日志 `target/canvas-c-review-failed_clipboard_enumeration_releases_each_undelivered_offer.log` 为 1 通过（69.74 秒）。
3. 原生叠放没有应用祖先组裁剪。现在求所有轴对齐矩形裁剪的交集，同时约束像素和点击；圆角、路径或非轴对齐裁剪在公开文档校验时明确拒绝。`target/canvas-c-review-native-overlay-group-clip.log` 为 1 通过（1.72 秒），同快照缩放和 Tab 各 1 通过；不支持裁剪的协议红绿测试通过。

复测同时捕获普通拖入目标被无关 UI revision 推进误判失效。新增公开 `begin_transfer_snapshot` 校验实际显示的声明，不能借旧显示回执接收已替换声明；实际管理器测试为 1 通过（70.04 秒）。`target/canvas-c-review-native-transfer-file-import.log` 为 1 通过（72.64 秒）：真实 OS 文件进入实际 WASM 包，关闭 offer 后释放只读文件句柄，临时目录可删除。初审后所有改动须补审；最终 SDK 与 ZIP 尚待冻结，不能把候选包记录当作已交付账本。
