# Canvas 可编辑目标与语义实施记录

日期：2026-10-25。最新状态：阶段验收与双轴审查通过，已推送并读回 #129 closed/completed；四单完成后统一合回 main。来源：[02 工单](../tickets/canvas-protocol-extension/02-text-input-semantics.md)、[公共契约方案](../specs/canvas-protocol-extension.md)。

前置 #128 已通过双轴审查、实际 WASM 与原生验收，独立分支推送至 `d12c4d2139aaaf8dbb5ab5780d464944b5a22e13` 后读回为 closed/completed。用户要求四单在 `C:/Users/Tmiracle/.codex/worktrees/canvas-prerequisite/Editor` 的 `codex/canvas-protocol-extension` 分支完成，全部完成后统一合回 main；本单不修改或同步主工作区。

测试接缝沿用已确认的公开协议/管理器、实际 WASM 包及原生 GPUI。每轮只打通一个可观察行为；原生输入法、辅助功能和真实文档会话均须实际验证，不能用声明或编译结果代替。开发分支现已登记 Canvas 1.8 候选以构建独立消费包；整单仍未验收，不能据此宣称可交付。

## 当前循环

公开 `Document` 回归 `paragraph_declaration_negotiates_native_text_version` 已先因未知 paragraph 失败，再由有界段落、原文/字体检查及实际声明推导 1.8 后通过。新增文字计入场景和整份 UI 文本预算，旧 TextBox 不变。

原生 `paragraph.json` 先收到“需要 native glyph pixels”的失败回执，再通过 GPUI 原生多行塑形与裁剪显示中英文两行；像素及成功回执通过，1.48 s，截图已查看。段落声明阶段的协议全量 76 项通过。

`paragraph-query.json` 又先因缺失 `text_queries` 失败，再经同一原生布局输出点到 UTF-8 原文的回执，通过（1.47 s）：两行起点分别为 0 和 17。查询绑定场景、精确 UI revision、来源/文件、viewport、请求点及实际 native 字体/字号/DPI 指纹，每个 UI revision 重新发送观察，避免迟到观察被拒后永久丢失。16 个请求共享最多 256 个点；只按成功绘制的当前场景计算。

组合字符回归 `paragraph_query_rejects_positions_inside_a_grapheme` 已先复现把 e + combining acute 内部标量边界作为合法位置；改为锁定的 unicode-segmentation 1.13.3 检查扩展字素边界后通过。仅增加现有锁定依赖的边，不升级包版本。实际复杂 Unicode 原生定位仍需进一步验收。

原生 Unicode 查询已复现非法字素边界导致回执被公开事件校验拒绝，随后用同一原生位置选择相邻合法扩展字素边界，原生样本通过。一次误用了旧依赖图的测试二进制，`target/canvas-b-unicode-wrong-binary.log` 不作为证据；此后均从当前 Cargo 构建日志解析实际 `Executable unittests` 路径，不按文件名或时间猜测。

`paragraph-geometry.json` 的原生跨行光标/选区通过：24 逻辑像素行高，原文 0/17 两个行首，15..23 选区覆盖前行最后一个拉丁字形及后行两个汉字。协议全量 77 项通过。

锁定的 GPUI Windows 0.3.9 后端在 DirectWrite 回调中忽略 bidi origin/level。`paragraph-rtl.json` 先复现 Hebrew 原文 carets 从左到右；宿主在本地组件中保留原生字体、glyph ID、fallback、advance 与偏移，按锁定的 unicode-bidi 0.3.18 恢复视觉顺序，通过公开 native glyph API 绘制，不修改或 fork GPUI。新测试要求逻辑光标递减，并将实际像素与独立反向未连接字形参考比较，绿灯 1.46 s；截图已查看。此前四个段落样本再跑均通过，1.46–1.56 s。构建日志 `target/canvas-b-rtl-green-build.log`，实际二进制 `editor_app-85226ca5a025a9bd.exe`。

`paragraph-format.json` 已先因缺失 align 失败，再经同一原生 glyph/advance 模型验证样式段（红色粗体）、逐行水平居中、单行块垂直居中、最终可见行省略、原文范围 0..2/3..9 及隐藏后续原文光标，通过 1.50 s，截图已查看。原文未替换成省略字符串；查询仍绑定真实字体。此前五个段落原生样本回归通过 1.40–1.46 s。构建及运行记录 `target/canvas-b-format-green-build.log`、`target/canvas-b-format-green.log`；协议全量仍为 77 项通过。

公开 `canvas_target_input_rejects_a_replaced_epoch` 先因缺失 targets 失败，再通过当前 scene/target/epoch/source/projection 与完整逻辑键声明检查。运行时对新的目标输入同样要求当前成功的原生显示回执。

`target-keys.json` 原生脚本先复现缺失输入，随后在 Canvas 级 focusable 关闭时，按目标 epoch 保留独立原生焦点，验证 down/up/repeat、全部五项修饰键、物理码明确 None、Tab 次序及单调序号。Kit 的 Tab action 优先于 raw callback，因此在仅目标焦点启用的 CanvasTarget context 用公开 NoAction 覆盖 Tab/Shift-Tab，再由同一 native raw input 接缝处理；旧 Canvas 不启用该 context。最终通过 1.51 s。测试夹具早期曾持有 typed Root update 进行 native draw，已改为 AnyWindowHandle，排除测试重入故障。

`canvas_native_text_ranges_preserve_original_graphemes` 先缺失接口，再通过原文 UTF-16→UTF-8 字素转换及逆转换，覆盖 e + combining acute、joined emoji、CRLF、折叠光标与越界钳位。无 Rope 或 Undo 状态。

`target-pointer.json` 先因缺失 capture_on_down 失败，再用实际 native dispatch 验证三角形精确命中、进入/离开、画布外 move/up、失焦后一次 cancel 且不再投递旧拖动、精确小数 wheel（0.5/-2.25）及不重复旧 Canvas pointer/wheel，通过 1.65 s。原生 hit path 在现有场景 worker 上预先有界展开，场景最多 128,000 个平坦命中顶点，并计入跨场景共享 native vertex 预算；没有在鼠标回调中展开 Bézier。新增 lyon_algorithms 1.0.21 依赖边，版本已在锁文件中，无包升级。光标已接入原生 cursor policy，真实 OS cursor 形状仍待后续 OS 验收。

指针轮的快速原生构建显式使用 `--config 'profile.test.package.editor-app.debug=0'`（仅减少该测试包调试信息，保持 debug assertions），实际 Cargo 输出为 `editor_app-040b74d210253260.exe`；记录 `target/canvas-b-pointer-green-build.log`。此前七个原生键盘/文字样本回归通过 1.42–1.53 s；协议 79 个 unit tests 和 7 个 integration tests 通过（`target/canvas-b-pointer-protocol.log`）。最终阶段门禁仍执行仓库要求的普通入口。

`target-ime.json` 先因缺失 text_input 声明失败，随后在真实 Windows/D3D 窗口通过公开 EntityInputHandler 验证（2.06 s，`target/canvas-b-ime-green-build.log`、`target/canvas-b-ime-green.log`）：UTF-16 范围替换 joined emoji，保留预编辑跨普通重绘及 1.25 倍缩放/平移，光标候选矩形随同一投影移动，连续提交“中文/a/b”形成原文 UTF-8 前驱链，旧累积确认保留后续 a/b。换 epoch 后持有旧 native entity 再调用迟到提交，在新目标重新聚焦前后均不能改写新文本。每个目标使用独立可撤销原生输入端点，没有把旧 handler 重定向到当前焦点。此项验证实际 GPUI native text 接缝；真实 OS 中文输入法及系统候选窗口仍待验证。

公开协议 `canvas_current_target_accepts_an_older_display_frame` 先复现新绘图发布使仍显示的目标事件被提前判过期，再保留相同来源/目标 epoch 的 admission；实际 Manager 的精确原生显示回执与单调序号门禁仍待完成，协议准入不单独授予坐标权限。

`native_preedit_join_rejects_atomically` 从公开本地文字组件复现组合标记接入相邻已提交 grapheme 后产生非法 marked range，现于修改前拒绝并保留原文/选择；红绿日志 `target/canvas-b-grapheme-mark-{red,green}.log`，1 项通过。`paragraph_rejects_excessive_effective_font_before_native_shaping` 在纯协议入口安全复现“微小框 + 巨大组 scale”绕过字体预算；现独立限制有效字号并在已知 DPI 的 native shaping 前追加物理字号边界，红绿日志 `target/canvas-b-font-budget-{red,green}.log`。

共享组坐标的协议回归先因缺失 semantics 失败；现目标、非交互语义和画笔共用 group transform/clip/opacity。命中路径与祖先 clip 在现有 worker 有界展开，相同 group 的 clip 共享；`target-space.json` 实际 native capture/shape hit 通过 1.79 s。语义树限制 4096 个可见节点、32 层及 64 KiB 名称/值，逻辑行列总数不会展开为原生子节点。

实际独立 `canvas-flow 1.1.0` 候选已经通过宿主内嵌 SDK 与 `--plugin-package` 构建，保留阶段 A 的 1.0 ZIP/账本不覆盖。`canvas_editable_shown_frame_text_chain_ack_backpressure_and_aba` 显式 ignored 运行通过（33.68 s，`target/canvas-b-manager-chain.log`）：仅已呈现并存在的目标获得帧证据，后续相同 epoch 绘图/投影更新不提前撤销旧显示；输入序号去重，UTF-8 前驱链和完整 native snapshot 确认，非法字素范围在 WASM 前拒绝，32 条未确认队列返回 LimitExceeded，错误确认原子拒绝后同一实例可恢复，领域 Undo 留在 guest，目标退休后必须升 epoch。每个实例最多 256 个文本会话、64 MiB 草稿；目标 tombstone 最多 131072 个。此候选尚未固定源码 commit/最终 ZIP/SDK 摘要，不视为交付包。

Windows UIA 实测通过（`target/canvas-b-target-semantics-os.log` 1.86 s，生命周期回归 `target/canvas-b-target-semantics-lifetime.log` 1.83 s）：独立 Windows COM 客户端读取中文名称、ARIA 行列、100000 行总量、真实焦点及裁剪/投影后的物理边界，调用 SetFocus、可编辑角色的 SetValue 与单元格 Invoke。参考机 150% DPI 下单元格为 157×56 物理像素，与 70×25 内容框 × 1.5 组缩放 × 1.5 DPI 一致；隐藏行不进入原生树。锁定 AccessKit 0.34 Windows adapter 没有 GridPattern，类型化行列仍留在 AccessKit，Windows 客户端通过原生 rowindextext/colindextext 和本地化 FullDescription 读取总量；不宣称存在 GridPattern。旧 .NET UIAutomationClient 不认识 FullDescription，测试改用锁定版本 Windows COM 客户端；grid cell 的原生值为 readonly，可写文本使用真实 TextInput 角色。

真实 OS 中文 IME 通过（`target/canvas-b-os-ime-focus.log` 4.34 s，生命周期回归 4.21 s）：仅激活本测试 UI 线程已有的 HKL `0x08040804`，使用经自有 HWND 命中/前台/焦点校验的实际 SendInput，产生 UTF-16 3..12 的预编辑，最终由系统输入法独立提交“中文”，替换原 emoji 的 UTF-8 4..15。系统 CANDIDATEFORM 在 150% DPI 下为 (51,50)，1.25 倍缩放及 (20,10) 平移后为 (94,77)，与同一原生字形 marked line 起点一致。最初测试把候选位置误当光标末尾，核对 GPUI 公开策略后修正为 composing line 起点；没有修改输入法系统设置、安装或切换全局语言。原生测试先关闭自有 HWND 并排空消息；交互失败也走相同清理路径。

目标生命周期的原生红绿已闭环：`target-ime.json` 先复现换 epoch 没有取消回执，再通过原始 frame/source/target 的一次取消（2.09 s）。`target-pointer-epoch.json`、`target-pointer-modal.json` 分别验证换代与模态接管取消并停止旧拖动，通过 1.67/1.65 s；原失焦捕获回归 1.65 s。公开协议只为有界 Focus(false)/Composition(cancel)/Pointer(cancel) 提供清理准入，Manager 仍须匹配旧 native frame 并消费实际观察到的活动 lease；取消不能重新授权 Text/Key/A11y。实际 WASM 管理器扩展验证一次性取消及不可重放，通过 34.35 s（`target/canvas-b-manager-cleanup.log`）。原生输入序号由 App 共享，避免 widget 重建重用序号；单 Canvas 仍严格递增，间隙也可来自其他 Canvas 输入。

独立 `canvas-table 1.0.0` 候选也已通过宿主 SDK/打包入口构建；公开 Manager 显式 ignored 用例通过 44.23 s（`target/canvas-b-manager-table.log`），只发布六个可见单元格与 100000 逻辑行元数据，emoji 字素替换、辅助 SetValue、目标换代和 guest 领域 Undo 均可观察。两个包的原生完整组合仍待验收，最终固定提交/摘要尚未登记。

`paragraph-affine.json` 先复现旋转/路径裁剪/半透明组下的 Paragraph 明确失败回执，再以同一 GPUI native glyph ID/字体/advance 的有界像素适配通过（1.66 s，截图已查看）。宿主静态复杂组仍按原有惰性数值 SVG 合成；文本不作为 SVG/HTML 字符串解析。每张 native 字形图至多 2048²，原文、输出、PNG、采样均有预算。新发现锁定 Windows backend 的彩色字形栅格会使用共享 D3D immediate context，故彩色字形须由 GPUI UI executor 执行，worker 保留布局、灰度字形与合成；不能仅凭 Send+Sync trait 在后台调用该颜色分支。此线程接线和实际后台 emoji 回归正在验证。

后台彩色字形分支已通过 `prepare_worker:true` 的 `paragraph-affine-emoji.json`（1.57 s，`target/canvas-b-paragraph-affine-emoji-draft.log`）；实际彩色 emoji 截图已查看。灰度塑形与像素合成留在 worker，彩色 native glyph 通过有界 UI executor 请求使用同一 App 的 D3D 上下文，队列满/退休/超时明确失败。

`paragraph-affine-query.json` 先复现复杂投影不支持的位置查询，再通过同一 native glyph 基布局与完整 affine/viewport 逆映射（1.51 s）。查询矩形为保守轴对齐边界并与祖先 clip 的保守边界相交；精确形状命中继续使用 worker 的路径裁剪。没有把保守查询矩形声称为路径的精确可见片段。

`target-edit-keys.json` 先观察到 Backspace 不改变文字，再以显式 `text_input.editing_keys` 通过整 emoji、组合字符删除、选区移动、Ctrl+A、换行和 Ctrl+Z 不产生宿主 Undo（1.49 s，`target/canvas-b-target-edit-keys-draft.log`）。原始 down/up 保留，三次独立 Text 的原文范围为 4..15、0..3、0..1，前驱序号正确。默认不启用原生编辑策略，领域快捷键仍由 guest 决定。真实 OS 中文 IME 回归通过 4.24 s。

`target-affine-ime.json` 先复现旋转段落缺少 native candidate geometry，再通过 90°、路径裁剪、0.5 透明度下的原生预编辑、候选矩形、点位置逆映射及 D3D 像素变化（3.07 s，`target/canvas-b-affine-ime-worker.log`，截图已查看）。复杂场景仅替换该段落的私有 native presentation，公有 source/target/scene 不变；同 Canvas 最多一个后台任务和一个合并后的展示快照。私有草稿图片另受全进程 64 MiB 池约束，作业在分配前预留最多 16 MiB，完成后缩减到实际大小，旧纹理在替换/失焦/实体退休时显式逐出 GPUI atlas。未合并或丢弃任何输入回执。

两个实际包的原生完整组合已通过（`target/canvas-b-wasm-flow-editable-focus-guard.log` 37.07 s、`target/canvas-b-wasm-table-editable-focus-guard.log` 41.64 s）：真实 OS 中文 IME 预编辑不提交领域文字，最终提交经公开 Manager 送入实际 WASM，累积确认后保留正确选择；独立 Windows COM 客户端调用 SetFocus、观察真实焦点、SetValue“辅助 value”，再由 guest 提升目标 epoch 并显示新值。表格只暴露六个单元格和有界语义节点，保留 100000 行总量。

此组合曾复现 GPUI 逻辑焦点与实际 HWND 键盘焦点正确，但 AccessKit 窗口 activation 状态漏更新。锁定 Windows 后端在原生 activation 处理时使用可失败的 `try_borrow_mut`。本地通用窗口适配现对属于当前进程/UI 线程且实际前台、键盘聚焦的 HWND 排队一次观察；回调执行时重新检查真实 OS 焦点，再交给既有后端 activation handler。它不移动 OS 焦点，不合成键盘或鼠标，不改动上游源码；注册消息隔离且同 HWND 合并，完成或销毁后撤销。无相应真实焦点时不重放旧状态。UIA 客户端只在自有窗口仍在前台且元素进程仍匹配时读取全局焦点名称，避免读取其他应用内容。

两个进一步的原生回归已先红后绿：`target-keys-visible.json` 复现 Tab 进入透明组内没有原生元素的目标，现按已显示帧的实际可见目标几何过滤；`target-pointer-deactivate.json` 在第二个自有原生窗口获得激活后，复现不可聚焦目标仍持有拖动，现单独观察 window activation，取消全部捕获/预编辑并发送原作用域 Focus(false)。重新激活只恢复仍持有逻辑焦点的目标观察，不复活旧捕获。原生输入端点也检查窗口实际激活状态。日志分别为 `target/canvas-b-tab-visible-{red,green}.log`、`target/canvas-b-deactivate-red.log`、`target/canvas-b-final-target-pointer-deactivate.log`。

多按钮回归 `target-pointer-chords.json` 先复现 GPUI 只报告另一个按键时误取消最初捕获，再以实际观察到的独立 down/up 集合保留捕获，直到所有已观察按键释放或明确取消；公开 buttons 字段继续为 None，不伪造完整原生按键掩码。Back/Forward 保留独立编号 3/4。实际 Manager 的原始包链条再验证右键先释放后仍可一次清理左键捕获，通过 33.29 s（`target/canvas-b-manager-chords.log`）。

公开本地文字组件回归 `native_selection_after_commit_survives_its_late_acknowledgement` 先复现旧提交确认把后来的选择 0..1 重置为光标 3..3，再保留原生提交后的新选择；2 项集成测试通过（`target/canvas-b-selection-ack-green.log`）。公开协议回归也拒绝禁用语义祖先下的子动作，原生适配复用相同结构校验，先验证原场景/目标/动作后才改变 native focus。

视口外目标的追加回归也先复现 Tab 错误，再通过显示帧原始 basis 与 viewport 交集过滤（`target/canvas-b-tab-viewport-{red,green}.log`，绿灯 1.48 s）。透明组和视口外目标均保留声明身份，但不截走可见目标的键盘导航。

本次阶段门禁已通过：`cargo fmt --check`；`cargo test --workspace --exclude editor-app --offline --locked --jobs 2` 共 546 passed、387 ignored（112 组结果，ignored 未作为通过）；`cargo check --workspace --offline --locked --jobs 2`。实际 metadata 再确认 workspace 和 Cargo 输出都位于本 worktree。受影响 `ui::plugin::` 共 77 passed、21 ignored；公开文档会话 IME/相邻 typing 的原子 Undo/双范围 Undo/远距离 viewport 的文本选择焦点 Undo 四项分别显式运行通过；SDK 导出六项通过。原生十二项显式回归及追加 Tab 视口样本均通过。最终记录在 `target/canvas-b-final-*.log`；已有警告保留，无本次测试失败。

最终源码审查、SDK/包摘要固定仍未完成；本记录保持实施中，不能据此关闭整单。

## 第二单审查修复循环

固定点 `561aa5b2...769dbfe8` 的首次双轴审查：Standards 发现 1 项 P1（原生字形位置平方扫描），Spec 发现 3 项 P2（旧显示帧滚轮增量、路径孔洞可见性、圆角策略）。均保持未关闭，直到修复和复核。

`paragraph-budget.json` 在实际 Windows native font 接缝复现合法 64 KiB 单行布局 6.406 s，超过 2 s 响应性回归边界。现单次建立与锁定 GPUI `x_for_index` 完全相同的 first-match 原文字节位置表，按 source 索引查行内 glyph；软换行只复制自身的既有 UAX #9 embedding levels 再应用 L1，不重新解释跨行 embedding，也不重复扫描整条长行。相同原生用例通过 129.8 ms，原文查询仍正确；Unicode、RTL 和格式/省略样本复跑通过。

`target-wheel-projection.json` 经普通 UI 发布 scale=2、尚未绘制时派发滚轮，先复现旧投影配新 delta 比例；现位置与像素 delta 共用原显示帧的 basis，通过。`target-keys-visible.json` 再加入 even-odd clip 孔内目标，先复现 Tab 进入不可见目标，再以真实填充交集通过。原生 UIA 样本同时加入孔内交互目标与非交互 label，两者都不进入实际 OS 树，原有四个可见语义节点/物理边界和动作仍通过。

填充交集复用已锁定 `lyon_tessellation 1.0.22`，只增加依赖边；先有界展平，再通过可拒绝的 geometry builder 限制顶点和三角形输出。每个目标依次与共享祖先 clip 的填充三角形相交，空交集不保留焦点/语义范围。中间凸多边形固定 64 点 scratch，扩展片段分配前计入共享 128000 顶点，全部目标共用 1600 万 intersection work 预算。全部工作在原有 worker，不移入输入回调。

圆角现在由 native drawing、generated SVG 与 hit/clip 构形共享逐角短边半径上限。`target-rounded.json` 实际绿色像素和原生按下结果同时验证 `[100,0,0,0]` 的 100×100 目标及同样祖先 clip，两个可见点均命中正确目标（1.50 s）。旋转预编辑回归也通过。

修复后再次执行 fmt、workspace tests、workspace check 和插件宿主定向 77 项，均通过（`target/canvas-b-review-*.log`）。当前独立 SDK 包经 `769dbfe8` 宿主实际 Release 构建，公开 Manager flow/table 分别通过 32.96/37.54 s，冻结 A 的两个场景回归通过 52.73 s。审查修复尚待双轴复核和最终原生包验证。

Standards 对 `09954722` 复核为 0；Spec 原三项已解决，又在“视口完全落在大型孔洞内”组合中发现 1 项 P2。两个新增原生样本 `target-keys-viewport-hole.json` 与 `target-semantics-viewport-hole.json` 均先复现失败：Tab 仍进入包围视口的不可见外环；独立 COM 客户端多读到 hole-target/hole-label 两项。现保留 worker 已接纳的真实凸填充片段，最后再与实际 native viewport 做固定 scratch 相交；Tab 使用原显示帧 basis，元素/语义使用实际布局 viewport。目标与对应语义共享 Arc 片段，稳定 ID 使用有序索引查找，不在 Tab 中重复扫描全部 regions。最终没有展开路径、重新 tessellate 或分配 native clip 片段。两项绿灯通过，OS 树只保留四个可见节点。

此后 13 项原生输入/文字/语义回归全部通过（`target/canvas-b-green-*.log`），覆盖原孔洞、视口孔洞、窗口失活、多键捕获、模态、普通/affine/真实 OS IME、旧帧 wheel、圆角像素/命中与长行预算。冻结 A 在 125% 与 200% 深色窗口的实际包像素回归也通过（`target/canvas-b-final-wasm-flow-*.log`）。fmt、workspace tests/check 与插件宿主定向 77 项再次通过（`target/canvas-b-viewport-*.log`）。这一组合修复仍待双轴复核。


## 第二单最终验收

源码最终固定为 `297998d10a2e0084a6bab4450ffbc375253c5c6d`，审查固定点仍为 `561aa5b2...HEAD`。两名只读审查代理最终报告 Standards 硬性问题 0/判断性问题 0、Spec 未解决 0/新增 0。原长行 P1、三项 P2 和追加视口孔洞 P2 全部解决。

`09954722` 宿主再次执行两个独立项目的 `--plugin-package`，Cargo 复用同源 Release 组件；两个 ZIP 与公开 Manager 已实测输入的 SHA-256 完全相同。SDK 为 `64a65b86088906ba33b5195ec9ec325d05ad9d6ba897e3406f048ec37bd79a11`，Rust 1.98.1 / wasm32-wasip2 / Release。后续原生修复没有改变 SDK 或 guest 源码。生产宿主摘要、guest source 摘要及构建提交见固定输入账本。

| 固定输入 | 大小 | SHA-256 |
| --- | --- | --- |
| `canvas-flow-1.1.0.zip` | 393460 bytes | `9aaa100f7b2b5af2c96f111a43f67a483dc16af52a4cbb4b5d5717d7af3bdc22` |
| `canvas-table-1.0.0.zip` | 441905 bytes | `d4567c3b52c2fffd07d820e48639d6ddcc4d95881100de2e585cfc3e8691e105` |

每包仅包含 manifest 和实际 WASM，协议 7，要求 `ui.canvas >=1.8,<2`，权限仅 `ui.panels`。历史 flow 1.0 ZIP 保持原 306702 bytes/SHA `8ada9f8754cca3fb65290b8e6f2b3823f65fbf4eb8d26c89d03f4ffe7d387833`，原账本项未修改，bare historical ID 选择不变。固定验收包不进入市场、宿主发行或 SDK。

| 工单验收项 | 实际证据 |
| --- | --- |
| AC1/AC2 | 原生段落、换行/对齐/省略、Unicode/emoji/组合字/RTL/连字样本、原文查询和实际字体环境；64 KiB 长行响应性回归。 |
| AC3/AC4 | 独立 key down/up/repeat、五项修饰键和 Text，Tab、精确 hit/capture、按键组合及一次 cancel；窗口失活、模态、epoch/来源退休；圆角像素与 hit 一致。 |
| AC5 | 普通和 90° 路径裁剪的 native EIH 预编辑、缩放/平移与候选几何、连续提交/迟到 ACK/选择、旧端点撤销；真实 OS 中文 IME 与 WASM 往返。 |
| AC6 | 四项 DocumentSession 文本/选择/IME/原子 Undo 回归；guest 领域 Undo；原生 code alignment 再验证唯一 source entity、IME 和 Undo。 |
| AC7 | 独立 Windows COM 客户端读取名称/值/角色/真实焦点/行列总数/投影物理边界，执行 SetFocus/SetValue/Invoke；两种孔洞均不暴露不可见语义。 |
| AC8 | 禁用全部 candidate overrides，从冻结账本默认加载实际 flow/table，完整运行 Chinese IME → Manager → WASM → ACK → UIA SetFocus/SetValue → 新 epoch 重绘。flow 39.82 s、table 55.74 s，均通过。 |

冻结包原生记录为 `target/canvas-b-frozen-wasm-{flow,table}-editable.log`。另外四项旧路径 ignored 测试已显式通过：Canvas 1.4–1.6 文字/列宽像素、source Canvas 不能将旧释放送到替换场景、预编辑/取消后的旧 action 拒绝、code alignment 保持 source entity/IME/Undo（`target/canvas-b-legacy-*.log`）。

阶段门禁、SDK 六项、公开 Manager 实际包、插件定向、四项文档会话及本单原生证据均通过；未运行的 ignored 项仍记为 ignored。实机为现有 Windows/D3D 环境，其他平台未实测。锁定 GPUI 没有提供物理键码/完整按键掩码时保持 None；AccessKit 0.34 没有 Windows GridPattern，类型化行列与本地化原生描述已实测。查询矩形保守包围可见内容，原生目标/语义可见性使用真实填充片段。C/D 的控件/交换、保留更新、隐藏与长期性能组合仍由后续工单负责。


## 分支交付与议题读回

`966a9a2b05fe8923e1f9077128289b7ae84a36da` 已普通推送至 origin 的 `codex/canvas-protocol-extension`，并以 `ls-remote` 核对同一 SHA。GitHub 连接器写入仍为 403，按仓库既有流程通过现有本地 Git 认证的 REST 更新 #129，随后独立连接器读取状态为 closed/completed（2026-10-25T15:06:50Z）。#128 也重新读回 closed/completed。父 #127 保持开放，未创建 PR/Release，未修改主工作区。接续 #130，再执行 #131，全部完成后统一合回 main。
