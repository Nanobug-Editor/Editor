# Git 暂存提交面板归属调整

用户明确修订：暂存提交应与资源管理器、大纲同类，可以在共同面板区域并列显示和拖动；不应默认在资源管理器外侧额外占一整列。此修订替代早期“审查优先”原型中暂存提交贯穿全高的摆放，不改变独立 Git 插件的业务和交付边界。

起始问题：Git 清单已声明 `position=left`，但通用面板同步直接映射外侧 DockLeft；资源管理器、大纲则位于 Center 树的左侧分区。两者由同一 Base DockArea 管理，却采用不同默认区域，形成截图中的两列。调整落在通用宿主布局，没有增加 Git ID 分支、专属 API 或上游源码副本。

本次沿用既有原生布局测试接缝；先用任意 ID 的左面板和真实绘制几何建立回归，再核对旧布局一次迁移、自行拖动后的布局保存/恢复、隐藏与重开，以及真实独立 0.1.8 包的原生界面。共享侧栏修订已验收并生成本地宿主便携包；根工作区另有既存检查问题，详见下文，不宣称全仓全部通过。

## 实施反馈循环（保留阶段结果）

`extensions::dock_tests::left_native_panels_share_explorer_outline_sidebar_without_extra_column` 通过任意 `sidebar-fixture/review` 声明进入现有 EditorApp 同步及生产 LocalDock。原实现 0.18 秒 RED：插件框 `x=0,width=280`，Explorer 框 `x=280,width=134`，确证两个不同列。冻结 runner 为 `target/git-sidebar-red-runner-2026-10-18/editor_app.exe`，SHA-256 `679bdf09adfe4106f648483423d3c9d20be5605ddbf8d73176bb21cbf71af9da`，日志 `target/git-sidebar-default-red.log`。

最小产品调整仅在本地 Dock 组合层：沿 Center 树的 Editor 路径寻找最近的真实左侧分支，将声明 Left 的新面板注册后用公开 Base move/split 加为同栏独立叶；没有该分支时保留原侧栏回退。Right/Bottom 的原默认策略未变。不能按 Explorer 名字盲跟它到用户移动后的右侧，不能重建整个 Center 树或插件视图。

首个候选调用宿主面板的 visibility getter，引发已在更新的 EditorApp 被回读；真实 backtrace 位于 `target/git-sidebar-parent-borrow-probe.log`。已移除这类回读，只检查公开持存节点，让 Base 自己处理可见性和测量；相关探针未写入生产代码。

默认布局回归随后 1/1 GREEN，0.15 秒，见 `target/git-sidebar-default-green2.log`。三面板同 x/width、独立非重叠可见槽，Editor 横向位置与宽度保持，仍只有原有 GPUI 窗口。既有 LocalDock 拖动/分割 5/5、0.17 秒，见 `target/git-sidebar-base-dock-tests.log`。该阶段冻结 runner `target/git-sidebar-default-runner-2026-10-18/editor_app.exe` SHA-256 `4bea12878f7d5e4281bf475fc30ee132e1b8b3746fd648d7bd23a419424bc084`，不当作后续迁移或最终包凭据。

本轮非 UI 工作区测试 77 组、278 passed、0 failed、302 ignored；ignored 未计通过。联合候选的面板集成 2/2、LocalDock 5/5 再次通过，分别见 `target/git-sidebar-dock-integration.log`、`target/git-sidebar-local-dock-final.log`。

旧布局回归先在真实延迟启动／恢复路径 RED：仍在外侧 Left 的面板未进入共享区，见 `target/git-sidebar-session-red.log`。最小迁移随后保留同一面板实体、自定义 Right 位置及其尺寸／开闭状态，并持久化完成标记；联合测试仅剩“旧共享侧栏绝对像素宽度不变”的过强断言失败。释放原外侧 Left 后，Base 根据扩大后的 Center 空间重新测量是正常行为，不为这一断言重建布局树。另一条真实旧 JSON 加载回归确认：没有旧 Dock 记录时迁移标记仍为 0，可能将用户之后保存的新 Left 位置再次迁移，正在补修。两项实际结果保留在 `target/git-sidebar-session-joint.log`，未计通过。

联合冻结 runner `target/git-sidebar-joint-runner-2026-10-18/editor_app.exe` SHA-256 `0b643058c99c4901b1aca31becb2a11679f45ba84e08d7f924a58227d4403431`。在该阶段真实独立 Git 0.1.8 包的原生共享侧栏专项尚待执行，没有交付宿主候选。

## 共享侧栏真实尺寸反馈

首轮原生专项在真实 WindowsPlatform、1280×900 请求视口、深色中文 100% 下，普通 Explorer＋暂存默认和隐藏全部宿主侧栏均通过。正常打开大纲后三面板同屏时 RED：暂存内容 clip 高 163px，树高仅 6px，提交按钮被裁出；截图 `target/git-shared-sidebar-native-2026-10-18/dark-zh-CN-100/sidebar-before.png` 与 `native.log` 相符。未进入拖动、未开始第二组合，不能计为已完成验收。失败后正常关闭与全部 HWND 退役通过，完整流程 verdict 仍为失败。

尺寸修正继续使用公开 `InsertTarget::Split`：对整个左侧分支分割，不再仅分最后一个可能隐藏的叶节点；以旧垂直分支的总尺寸作为新面板的初始权重，无测量时使用视口高度，Base 仍负责约束、实际测量和用户缩放，不维护第二份布局。该候选 `target/git-sidebar-band-runner-2026-10-18/editor_app.exe` SHA-256 `58c3a28d2d4677fc7bc00ab7d91cd9750880c6bb70a32a73e54a592216dd54c2`。

此候选会话完整测试 7/7、0.96 秒通过（含旧布局迁移、没有旧布局的版本确认、future 2 不降低，以及完成后用户移回外侧 Left 保存／重开仍保留）。面板集成 2/2、0.41 秒，LocalDock 5/5、0.17 秒。对应日志 `target/git-sidebar-session-band.log`、`target/git-sidebar-dock-band.log`、`target/git-sidebar-local-band.log`。候选源码 `cargo fmt --check` 与 `cargo check --workspace` 通过；根工作区格式检查仍只报本轮开始前已有的 `extensions/preview/toolbar.rs:65`、`extensions/surface.rs:86` 两处，未擅自修改，见 `target/git-sidebar-root-format-check.log`。

该候选第二轮原生检查确认三面板初始布局可用：暂存 outer 高 369px、clip 高 341px。随后的单次标题客户端拖动实际将暂存移到 Explorer 上方，却把内容 clip 缩为 129px、树高 6px，再次 RED；见 `target/git-shared-sidebar-band-native-2026-10-18/dark-zh-CN-100/failure-workbench.png` 与 receipt。未重试、未弱化尺寸检查。原因是同栏换序仍沿用新增分割的 `size=None`，将目标 peer 再分半。

修正后，同父级、同堆叠轴的换序使用公开 `DockArea.dump()` 当前测量尺寸，传给公开 `move_panel` 保留源分配；跨区域或非同级分割保留原规则。只读取瞬时快照，不缓存第二份布局或拖动尺寸，不改变输入与面板资源生命周期。产品专项只读审查未发现未解决项。

拖动修正候选 runner `target/git-sidebar-final-runner-2026-10-18/editor_app.exe` SHA-256 `176de1072904ecff29df48b1742d15279b9b8734efa5a15c3da3cf4e75bbd0b7`：会话 7/7（0.95s）、面板 2/2（0.38s）、LocalDock 5/5（0.17s）、可见性 3/3（0.28s）通过；候选格式及 workspace check 再次通过。日志分别为 `target/git-sidebar-session-final.log`、`git-sidebar-dock-final.log`、`git-sidebar-local-final.log`、`git-sidebar-visibility-final.log`、`git-sidebar-final-format-check.log`、`git-sidebar-final-workspace-check.log`（均在 target 下）。

此 runner 的原生单次拖动后，截图已显示完整树、草稿和提交按钮，但测试在首个布局帧立即检查 Editor 与侧栏的边界时失败。当时正常激活刷新尚在进行，测试没有像此前步骤那样等待 `h.settle()`。保留失败输出 `target/git-shared-sidebar-final-native-2026-10-18`；仅为验收加上 Drop 后既有稳定点，严格边界、完整控件、资源保持与单次输入断言均未改动。最终两组合使用新 runner `target/git-sidebar-final2-runner-2026-10-18/editor_app.exe`，SHA-256 `6fe077f8a41297072db4563d84d9d3c12033aa7275ad1a4fda239740d1c5f603`。该 runner 的同四组回归仍为 7＋2＋5＋3 passed（1.00／0.42／0.17／0.28 秒），对应 `target/git-sidebar-*-final2.log`。原生结果仍待实际读回。

构建沿用本任务已有的 Nanobug 自身源码快照 `target/git-ticket119-source`；它不是任何上游源码副本。工作区已有其他任务改动，`app/layout.rs`、`app/session.rs` 和其主测试文件与快照原有差异保留，只镜像本次匹配的改动；本次新增模块、通用 Dock 与原生验收代码摘要一致。完整根／候选摘要清单为 `target/git-sidebar-final-source-hashes.json`，不以候选检查替代无关工作区变更的验收。

## 实际绘制边界修正与工作区门禁

Final2 加入稳定等待后仍在同一严格断言 RED，证明不能归为瞬态：Stage outer 392px、Explorer 100px、Outline 100px，合计 592px 超出 Editor 的 569px；大纲底部确实被裁切。实际 PNG 与完整原生 widgets 快照保留于 `target/git-shared-sidebar-final2-native-2026-10-18/dark-zh-CN-100`。Base 持存／缩放偏好与实际绘制边界存在差异，不能将 `dump()` 中尺寸直接当作固定像素分配。

最终产品改用公开 `TabGroupRenderer.frame().on_prepaint()`，记录当帧外框；同栏换序取源实际绘制高度。瞬时几何表仅由本地 renderer 持有，group 销毁即移除对应 NodeId，不参与布局持久化、面板可见性或业务状态。Base 继续管理唯一布局、约束与资源移动。该修正只读审查未发现未解决项。

新 runner `target/git-sidebar-painted-runner-2026-10-18/editor_app.exe` SHA-256 `5fa578de92f783d40e9d21933deacf10f4bf697155534a3d14a9f3ba8b901671`，匹配 PDB 一并冻结。会话 7/7（0.94s）、面板 2/2（0.41s）、LocalDock 5/5（0.17s）、可见性 3/3（0.28s）通过，日志 `target/git-sidebar-{session,dock,local,visibility}-painted.log`。候选 `cargo fmt --check`、`cargo check --workspace` 通过，见 `target/git-sidebar-painted-{format,workspace}-check.log`。

根工作区也实际执行 `cargo check --workspace`，通过，见 `target/git-sidebar-root-workspace-check.log`。实际 `cargo test --workspace --exclude editor-app` 在已有、未纳入候选的 `crates/plugin-runtime/tests/application_management.rs:175,271,295` 编译失败：`Manager::select_application` 不存在；没有运行的测试不计通过。见 `target/git-sidebar-root-nonui-tests.log` 与 `git-sidebar-root-workspace-results.json`。该文件在本轮开始时已是其他工作区改动，本轮未修改。冻结候选的非 UI 278 passed／302 ignored 为前述独立命令的实际结果，不能替代这一根工作区失败。

后续启动反馈更正：上段接口缺失并非该测试源码缺少实现。此前将内容不同的源码快照与主工程共用 target，导致旧依赖产物混用；主工程失效自身 crate 缓存后，启动构建通过，`application_management` 实际重新编译为 4 passed／2 ignored。根工作区完整非 UI 测试重新执行为 289 passed／0 failed／305 ignored，workspace 编译检查与测试后的原始启动构建均通过。原失败日志保留，旧归因修正与完整检查见[构建缓存修复](../../project/verification/build-cache-2026-10-18.md)。冻结候选的原生结果与独立交付 ZIP 不受本次缓存清理影响。

优化构建曾在 Final2 原生失败后取消，仅停止了按 PID、父进程、唯一二进制名及 target 路径核对过的本任务编译器；不生成交付包。快照的临时 Cargo／build.rs 项已精确恢复原摘要，后续仅从新通过候选重新构建。

Painted 原生用例全部通过尺寸、换序与 Editor 边界检查：拖动后 Stage outer 271px／clip 243px，Explorer 164px，Outline 134px，三者结束于同一 603px 边界，完整提交表单可用。Base 根据其约束重新分配，所以源绘制高度只是初始偏好，不承诺换序后的严格像素相等。此轮完整流程仍失败：验收错误要求虚拟树中已移出可视区的 `a-review.txt` 必须始终被绘制。实际句柄、epoch、展开状态、NativeUI 实体及草稿保持比较已通过；该轮结果不能计完整通过，日志与 PNG 保留于 `target/git-shared-sidebar-painted-native-2026-10-18`。

验收改为只读观察公开 Base `TreeState.selected_item()`，在 `ui/plugin/tree.rs` 增加纯 `cfg(test)` 的任意树节点观测入口，不是生产宿主／SDK API，不增加插件 ID 分支，也不改变选择或滚动。拖动期间精确比较原生选择；自身隐藏时按既有契约退役 NativeUI、Base 选择及旧 tree epoch，重挂阶段核对新实体和推进的 epoch，仍严格保留业务选中文件文本、树句柄／展开及草稿。没有滚动、重选或扩大窗口来伪造保持；几何和完整控件断言均保留。

## 最终原生结果

最终冻结 runner `target/git-sidebar-acceptance-runner-2026-10-18/editor_app.exe` SHA-256 `8e08d8fdb1d863159089ab10caca95dba3dd6ae6a2a9acfb173879db8995a714`（匹配 PDB 同目录）。同 runner 会话 7/7、面板 2/2、LocalDock 5/5、可见性 3/3 通过，耗时 0.95／0.42／0.17／0.29 秒，日志 `target/git-sidebar-{session,dock,local,visibility}-acceptance.log`；候选格式检查通过。

显式执行 `extensions::worker::native_acceptance::sidebar::git_native_shared_sidebar_peers_drag_and_visibility --ignored`，设置 `NANOBUG_GIT_NATIVE_SHARED_SIDEBAR=1`，锁定真实独立 Git 0.1.8 ZIP：`target/git-auto-refresh-fix-2026-10-18/git-0.1.8.zip`，SHA-256 `5c6c483fd60b9fc02277aadfe800408fbc324eab1b99ffac7e1ddf0acb9b3292`。本轮没有重建、升级或内置该插件。

| 原生组合 | 实际结果 | 几何与清理 |
| --- | --- | --- |
| 深色／中文／100% | 62.13s，通过 | scale 1.0，物理 1273×896；换序后暂存 clip 高 243px；全部原始 HWND 退役 |
| 浅色／英文／150% | 78.53s，通过 | scale 1.5，物理 1920×1350／逻辑 1280×900；换序后暂存 clip 高 246px；全部原始 HWND 退役 |

父测试 1 passed、0 failed，140.92 秒。两份 evidence 的 `workflow=shared-sidebar-only`、候选摘要、`shutdown_complete=true` 与空错误列表均已读回，见 `target/git-shared-sidebar-acceptance-native-2026-10-18/{dark-zh-CN-100,light-en-150}/evidence.json` 及父 `parent.log`。每组确认普通默认 Explorer＋暂存可用、隐藏宿主侧栏后可用、显示大纲后三个独立非重叠同栏面板、完整树行／提交输入／动作在真实 clip 内、Editor 位于右侧，随后单次拖动、展开／原生选择／草稿保持及正常隐藏重开。输入是实际 WindowsPlatform 上的生产客户端 `Window.dispatch_event`，不是操作系统 SendInput；本专项不冒充完整工作台八组合、Native 重启或系统 IME 候选验收。重启与用户保存位置由前述实际 EditorApp 会话测试验证。

重开后同一 panel32v1 保留，NativeUI 由14v3变为128v3／135v3、tree epoch 由2到3；句柄、展开、业务选择 `src/a-review.txt` 与精确草稿保留。新 Base 原生选择为 None 是正常输入退役重挂；拖动过程中原 Base 选择精确保持已验证。12 张正式场景 PNG 已产生，实际逐图核对两组各 default／before／after-drag／after-reopen，共 8 张，无可行动视觉缺陷；不声称其余4张已逐图核对。

同一 runner 另显式执行通用标题栏实际 OS 鼠标回归 `native_caption_mouse_controls_remain_responsive --ignored`，最小化、最大化／还原、关闭三个独立子进程均 exit0，父 1 passed、3.28 秒。见 `target/git-sidebar-caption-mouse.log` 与 `target/git-sidebar-caption-mouse-2026-10-18/{minimize,maximize,close}.log`。本轮未修改上轮标题栏产品实现。

## 本地宿主交付

从通过验收的 Nanobug 自身源码快照直接 Cargo 优化构建，244 秒 exit0，见 `target/git-sidebar-release-build.log`。使用临时唯一二进制名避免占用用户旧进程；构建后 Cargo 清单与 build.rs 精确恢复原 SHA-256 `b27ba699…a7ec64`／`fb347f8f…e423`，候选 `cargo fmt --check` 与 workspace check 再次通过（`target/git-sidebar-delivery-{format,workspace}-check.log`）。依赖及 Cargo.lock 没有升级；既存 Wasmtime／Tree-sitter LNK4217 链接警告保留在实际日志中，构建成功不隐瞒警告。

- 宿主：`target/git-sidebar-fix-release-2026-10-18/Nanobug.exe`，117,619,200 字节，SHA-256 `dd2c139cfe86d38dd3c6b6fe4b760b653b69fef76e29635f03b58376cc36d4f0`。PE32+ x64、Nanobug 产品资源／0.1.0、8MiB 栈与 133,912 字节资源已核对；匹配 PDB 同目录，仅供诊断。
- 便携 ZIP：`target/git-sidebar-fix-release-2026-10-18/Nanobug-sidebar-fix-2026-10-18-x64.zip`，32,744,891 字节，SHA-256 `8fd49fe3e2b28cb0198bfc58c5a292f6438a9e73b2c264ff5bef9423709efcc2`。仅含 EXE、图标、沿用的许可证及中文说明；已读回 ZIP 内 EXE 摘要，无插件、SDK 或 PDB。
- 实际交付 EXE 的 `--export-plugin-sdk` exit0、50 文件、0 Markdown，输出仅在独立 `target/git-sidebar-sdk-smoke-2026-10-18` 验证目录；SDK 不在便携载荷中。
- 完整摘要、源码对应关系、检查例外及两组回执映射见交付目录 `verification.json` 和 ZIP 的 `.sha256` 文件。

退出旧 Nanobug 后使用新宿主，继续单独安装／使用 Git 0.1.8。没有自动替换用户安装、修改测试项目、提交、推送或改变任何既有工单状态。

工作区已有其他主题和前几轮修复，均保留；不把已有测试、标题栏修复、周期刷新修复或历史工单状态算成本次新增成果。

返回[插件文档目录](../README.md)。
