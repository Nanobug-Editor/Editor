# Canvas 1.4–1.6 独立前置基线

2026-10-25。用户已授权将必需前置单独整理并提交，再连续实施 Canvas 扩展四单。

## 范围与来源

起点为 `561aa5b2006c38668a083482156646a16cd02b3e`（Canvas 1.3）。以任务开始前保存的 `canvas-protocol-extension-baseline-20261024` 快照核对已有实现，仅纳入以下公开能力及直接依赖：

- `ui.canvas 1.4`：TextBox 原生单行塑形、裁剪、对齐及省略。
- `ui.canvas 1.5`：有界样本文字测量及精确 UI revision 的 `TextMeasured` 回执。
- `ui.canvas 1.6`：相邻列固定总宽度拖动、TextBox 本地预览、一次完成事件和 epoch 校验，见[原生列宽契约](../specs/canvas-native-columns.md)。
- 协议校验/能力推断、editor-ui 文字和列宽控件、宿主适配、唯一 SDK 文件登记及回归测试。
- 旧终端的 Canvas 事件穷举仅补忽略其未启用的测量/列宽回执，使独立前置可以编译；不搬迁终端实现。

未纳入工作区的 Textarea、终端提取、语言服务、窗口和其他业务改动。SDK 登记不复制快照中重复的路径。此提交不含 Canvas 1.7 场景实现，不关闭四张实施议题。

## 隔离验证

候选在 `C:/Users/Tmiracle/.codex/worktrees/canvas-prerequisite/Editor` 构造，独立 `target/`；`cargo metadata --no-deps --format-version 1` 已核实 workspace_root 和 target_directory。验证对象为起点加本页范围，不借用后续场景代码或其他工作区源码。

- `cargo test -p plugin-protocol --lib --offline --locked --jobs 2`：67 passed，0 failed，0 ignored。
- `cargo test --workspace --exclude editor-app --offline --locked --jobs 2`：108 组，526 passed，0 failed，383 ignored，含文档测试。
- `cargo test -p editor-app 'ui::plugin::' --offline --locked --jobs 2 -- --test-threads=1`：76 passed，0 failed，18 ignored。
- `cargo test -p editor-app native_canvas_text_and_columns_reach_pixels --offline --locked --jobs 2 -- --ignored --nocapture --test-threads=1`：1 passed，0 failed，0 ignored；本机实际 150% DPI，1.40 s。前后图像保存在该候选的 `target/canvas-columns-before.png` 与 `target/canvas-columns-after.png`。
- `cargo test -p editor-app sdk_export --offline --locked --jobs 2 -- --test-threads=1`：6 passed，0 failed，0 ignored。
- `cargo check --workspace --offline --locked --jobs 2`、`cargo fmt --check`、`git diff --check`：通过。

以上记录该候选的实际验证数据；审查和 Git 交付以提交 SHA 关联，不把 ignored 用例计为通过。原生性能基准保留为显式 Release 测试，本次未执行或宣称性能结论。

原生测试 `native_canvas_text_and_columns_reach_pixels` 使用真实 Windows/D3D 窗口与 GPUI 输入路由，检查比例字体实际宽度、拖动前后红色字形覆盖、移动期间不通知访客、释放只通知一次；关闭自有 HWND 后再退出平台。输入通过 `Window.dispatch_event`，不将其称作 OS SendInput 或人工验收。

首次宿主编译暴露旧终端穷举缺少新回执，已按最小范围补齐并重跑通过；新增测试的 Fill 初始化补齐 `extend_to_bottom`。原生支架第一次绘制因 typed Root update 重入而失败，改用 AnyWindowHandle；后续回执和截图证实本地宽度及字形已变化，稀疏逻辑点采样在分数 DPI 下只命中 12 个字形点，故改为遍历新增裁剪区的全部物理像素。最终测试仍要求旧画面该区无红色、拖动后出现字形，以及唯一一次精确完成回执。

## Standards

固定点 `561aa5b2` 到候选 `9fcd56eb` 的独立规范轴初审：硬性违规 0、判断性发现 0。

## Spec

同一比较的独立规格轴初审发现 1 项 P2：Canvas 仅在尺寸变化时取消列宽拖动，布局平移会用新原点解释旧按下锚点，违反列宽规格的外部几何取消要求。

公开 UI 回归 `repositioned_canvas_cancels_its_native_column_gesture` 用固定尺寸 Canvas 和左侧 sibling 从 0 到 80 的宽度改变，保留同一节点、列 epoch 与宽度：未修复时收到错误完成事件，测试确实失败。修复在完整 bounds 改变时取消手势；重新运行 `ui::plugin::` 后 77 passed、0 failed、18 ignored。Windows/D3D 原生原入口再跑 1 passed（1.44 s），`cargo fmt --check`、`cargo check --workspace` 与差异检查通过。该修复不改变已完成的非 UI workspace 和 SDK 源码，复用此前对应检查证据。

`4fdebb00` 复查确认原 P2 解决，规范轴仍为 0；规格轴又发现 1 项 P2：已有一次未确认释放时，取消第二次拖动会保留第二次未提交宽度。公开本地控件回归 `cancelled_second_gesture_preserves_only_the_submitted_preview` 在原代码下观察到错误的 260 而非已提交 240，确实失败；覆盖确认在取消前、后到达，以及随后新的提供者更新。

修复用独立的待确认宽度快照替代布尔标记，确认回包按已提交结果匹配；取消恢复当前手势起始宽度，再采用已经确认的提供者状态。两个列宽行为测试通过；重跑完整 workspace 为 108 组、527 passed、0 failed、383 ignored，宿主 `ui::plugin::` 为 77 passed、0 failed、18 ignored，原生原入口 1 passed（1.47 s），workspace check、fmt 和差异检查通过。SDK 6 项对应源码未变，复用前述通过证据。该修复仍属同一最小前置，不包含后续 Canvas 扩展。
