# 窗口实时拖放与通用停靠模块

> 历史记录：用户随后确认采用[标记预览式窗口拖放](marked-dock-placement-2026-10-19.md)。本文的实时让位、浮动真实内容与无蓝色遮罩均已被替代，以下保留当时的验证证据。

## 已确认交互

本轮用户确认先完成拖放，之后再迁移 `crates/editor-ui`。实现保留在 `crates/editor-app/src/ui/controls/dock/`，业务视图和插件公开协议继续使用现有入口。

| 阶段或落点 | 放置行为 |
| --- | --- |
| 开始拖动标题 | 原窗口立即离开其布局占位，其余可见区域补满；实体、内容与回滚资料保留 |
| 编辑区域的四个边缘 | 原共享侧栏或条带的可见窗口一起移动，保留组内顺序与结构；同方向移动保留宽度或高度，改变方向采用目标方向已有停靠尺寸 |
| 具体工具窗口的四个边缘 | 只移动抓取的那个窗口，在目标窗口内部进行局部分割；原来的其他窗口回填 |
| 有效悬停 | 实际布局先让出空间，浮动内容按照真实预留空间调整尺寸 |
| 离开有效落点 | 收回临时空位，其他区域继续填满 |
| 有效位置松手 | 保留当前预览布局，之后才允许保存会话 |
| 无效位置松手或 Escape | 恢复拖动前布局、尺寸和焦点 |

工具窗口中央不合并标签。预留空间使用普通背景，移除蓝色拖放遮罩。指针在预留空间内时保留当前意图；指针没有移动时，后续绘制和 MouseUp 不会重复解释同一位置而改变布局。

用户示例中的左栏“资源管理器＋大纲”移到编辑区左侧后，两者成为上方编辑器行内的一列，底部 Git 横跨完整底部。打开右侧窗口继续只缩小编辑区，左列像素宽度保留。

## 模块接缝与状态

`LocalDock::new(title_height).with_elastic_panel(panel_id).create_area(...)` 创建一个可复用的原生停靠表面。弹性面板由调用方指定，不通过编辑器名称、插件 ID 或 Git 业务识别。未配置弹性面板的表面，所有具体窗口边缘都采用单窗口放置。

`dragging.rs` 管理一次手势的输入、有效落点、提交与取消；`dragging/snapshot.rs` 使用 Base 的不可变树副本和已有面板句柄生成临时 DockLayout，并固定拖动开始时的可见组成员，避免区域独占过滤变化后误带其他窗口；`dragging/preview.rs` 在浮动表面绘制原面板视图，读取实际预留尺寸，并在缩放和组扩展后保持抓取位置跟随指针；`dragging/visibility.rs` 暂时过滤源面板可见分配。Base DockArea 仍是唯一持久布局，手势快照只用于回滚，不新增面板注册表或文档状态。

重组先安装目标再移除来源，回滚先恢复来源再清理目标，防止实体短暂离开所有 Base 树而收到错误的移除回调。空左／右栏在手势中临时关闭；Base 会为关闭的底栏保留条带，因此底部来源的不可见句柄先寄存在不可见的 Center 分支，再移除底部占位，结束时归还或清理寄存句柄。

宿主在拖动期间跳过会话布局捕获、空区域清理和插件显示状态发布；插件通过相同公开面板贡献注册，无需插件专属宿主分支或新协议字段。插件移除或注册变化会使旧回滚失效，避免复活已移除插件或覆盖新面板。

## 诊断与验证

最小真实标题拖动回归 `live_drag_reclaims_source_before_release` 在修正前稳定失败：已隐藏资源管理器，但编辑区仍从 x=280 开始。首次及冻结程序重复日志为 `target/live-dock-before.log`、`target/live-dock-red-repeat.log`。实际原因是 Base 保留空的原生停靠盒子；仅过滤叶节点可见性不足以释放空间。

后续回归发现 Escape 监听需要位于焦点内容的祖先节点，兄弟 canvas 的键盘监听收不到该事件；监听已移至停靠根和浮动内容根。右栏来源及连续预览还暴露旧外层尺寸权重会在 Base 合并相同方向分割时缩小新子树，现将目标回收后的实际尺寸传到对应祖先。

历史阶段的通过记录见[编辑器局部列与底部跨度](editor-row-drop-2026-10-19.md)。该历史记录中的单窗口编辑区拖放和半宽预览，已由本文的整组移动与实时让位规则替代。

共享目录中的 EXE 被另一项完整 UI 测试占用时，最终链接曾报 LNK1104，见 `target/live-dock-verified-build.log`。没有终止该测试或修改其他控件迁移；改用 `cargo rustc -p editor-app --tests --jobs 1 -- -o C:/Projects/RustProjects/Editor/target/live-dock-delivery-runner/editor_app.exe` 输出独立程序，再直接执行相同测试过滤入口。最终程序为 `target/live-dock-delivery-runner/editor_app-6461604412b9c9dc.exe`，SHA-256 `91da31fbc06e39b0b0a9a6710d13072346d95b767282a5c3cd22eb92796ec31f`；本次相关源码摘要保存在 `target/live-dock-final-source-sha256.json`。

| 最终入口 | 实际结果 | 日志 |
| --- | --- | --- |
| 独立程序 `dock --test-threads=1` | 28 passed、0 failed、3 ignored | `target/live-dock-final-dock.log` |
| 独立程序 `sidebar --test-threads=1` | 10 passed、0 failed、1 ignored | `target/live-dock-final-sidebar.log` |
| 独立程序 `app::session::tests --test-threads=1` | 8 passed、0 failed | `target/live-dock-final-session.log` |
| 独立程序 `exclusive_tests --test-threads=1` | 3 passed、0 failed | `target/live-dock-final-exclusive.log` |
| 独立程序 `tests::problems --test-threads=1` | 6 passed、0 failed | `target/live-dock-final-problems.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78 组、304 passed、0 failed、311 ignored，exit 0 | `target/live-dock-nonui.log` |
| `cargo check --workspace --jobs 1` | exit 0 | `target/live-dock-check-delivery.log` |
| 本次相关 Rust 文件的 `rustfmt --check --edition 2024 --config skip_children=true` | exit 0 | `target/live-dock-scoped-format-final.log` |
| `cargo fmt --check` | exit 1：并行语言高亮与 UI 迁移文件仍有格式差异，本次未越界格式化 | `target/live-dock-format-final.log` |

停靠过滤包含 8 项实时手势回归：原侧栏立即回收、整组实时预览与取消且不保存、离开落点及无效松手、窗口间四边单独移动、底部来源完整回收与恢复、右侧来源保留左侧宽度、编辑区四边整组提交，以及组预览切换为单窗口预览。还检查预览前快速松手、拖回、手动缩放、左右栏宽度和会话重开。不同过滤入口存在重叠，不将它们的数量相加为唯一测试数。

实际 XML ZIP 沿用固定归档账本 `crates/plugin-runtime/tests/fixtures/packages/inputs.json`：`xml-0.2.2.zip`，SHA-256 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`。显式执行 `tests::outline::docking::native_outline_four_directions_resize_and_workspace_restore --exact --ignored --test-threads=1`，1 passed、90.90 秒，见 `target/live-dock-outline-final.log`。该测试使用真实插件和原生 GPUI 视图的 TestPlatform 输入，包含四方向整组移动、指定窗口边缘重排、分隔条缩放、实际编辑触发结构刷新和重开恢复，没有将它称为 Windows/D3D 验收。

## Windows 与实际 Git 插件

使用隔离候选 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`，未修改用户安装。设置 `NANOBUG_GIT_NATIVE_EXCLUSIVE=1`、`NANOBUG_GIT_CANDIDATE` 与独立的 `NANOBUG_GIT_NATIVE_OUTPUT`，显式执行 `extensions::worker::native_acceptance::exclusive::git_native_exclusive_dock_replaces_and_restores_peers --exact --ignored --test-threads=1`；父测试 1 passed、158.37 秒，两组实际 WindowsPlatform/D3D11 子进程均通过，日志 `target/live-dock-native-final.log`。

| 原生组合 | 源列与放置宽度 | 放置后 Git 底部实际范围 | 结果 |
| --- | --- | --- | --- |
| 深色中文 100% | 280 → 280 逻辑像素 | x=0，宽 1280 | 插件来源实时拖动、完整回收底部、浮动尺寸、Escape、上方行内放置、右栏与拖回均通过 |
| 浅色英文 150% | 280 → 280 逻辑像素 | x=0，宽 1285.3334 | 同上；独占替换、恢复及退出清理亦通过 |

两组 `target/live-dock-native-final/{dark-zh-CN-100,light-en-150}/evidence.json` 的 `observations.plugin_source_live_drag` 均确认底部回收、浮动尺寸等于预留尺寸、Escape 恢复原分配，以及无彩色遮罩；`observations.editor_left_drop_complete` 确认底部完整跨度、左列 280 像素、右栏开关不改变左列和拖回释放。两组 `shutdown_complete:true`、`observations.unexpected_error_logs:[]`。

已查看两组 `plugin-source-live-preview.png`，以及深色 `editor-edge-placed.png`、浅色 `editor-edge-right-open.png`：浮动真实 Git 内容跟随抓取位置，无蓝色遮罩；资源管理器保持上方行内宽度，Git 占据完整可用底部，右栏显示只缩小编辑区。

原生输入通过生产 `Window.dispatch_event` 进入 WindowsPlatform；不声称操作系统 SendInput 或 IME 候选弹窗验收。本轮未运行完整 UI 测试集、全部 ignored 用例、八组合矩阵，未打包安装器或迁移 `editor-ui`。规定编译和本次文件格式检查通过；全局格式检查的并行文件差异保留在上述日志中。`git diff --check`、新增文件行尾空白和四份相关文档的本地链接检查均通过。

返回[原生 UI 目录](../README.md)。
