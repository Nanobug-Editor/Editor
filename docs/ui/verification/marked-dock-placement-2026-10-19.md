# 标记预览式窗口拖放

## 用户确认的交互

本轮以用户最新两张示意图及“默认尺寸就用拖动开始前的实际宽高”为依据，替代[上一版实时让位](live-dock-placement-2026-10-19.md)。`editor-ui` 迁移继续留到拖放完成之后，本轮不创建新 crate。

| 阶段或落点 | 当前规则 |
| --- | --- |
| 开始拖动 | 真实窗口、内容和布局保持原位；来源绘制黄色斜线标记，鼠标旁只显示标题小块 |
| 编辑区边缘 | 空侧只移动自身；一级成员在本侧可带动整组，排除所有移动成员后该侧须为空 |
| 具体工具窗口边缘 | 只移动抓取的窗口；区域一级按方向限制，已有分栏内部允许四方向嵌套 |
| 切换目标 | 同时更新落点及移动范围；来源、目标标记和标题小块同步反映单窗口或整组 |
| 有效悬停 | 目标内绘制蓝色斜线落点预览；实际布局在松手前保持不变 |
| 目标中央／外部 | 清除蓝色预览；目标中央不合并标签 |
| 有效位置松手 | 基于原始树和当前落点一次提交，随后回收已空区域 |
| Escape／无效位置松手 | 清除标记和小块，原布局保持不变 |

左右落点采用原实际宽度并填满目标高度；上下落点采用原实际高度并填满目标宽度。该宽／高大于或等于整个目标宽／高时改用目标的一半，小于整个目标时保留原尺寸，包含标题栏。最终极小空间仍受 Base 最小窗格尺寸约束。当前标题拖动与占用规则见[四侧组合拖放修正](dock-group-fallback-2026-10-19.md)，之前的层级与尺寸修正见[嵌套布局与拖动范围](dock-nested-layout-2026-10-19.md)。

原有提交拓扑保留：左侧组合或最后一个可见窗口移到编辑区左边后成为上方编辑行的一列，底部 Git 横跨完整可用底部；排除整组后仍有其他左侧区域则拒绝。当前四向触发及嵌套边界见[四侧组合拖放修正](dock-group-fallback-2026-10-19.md)。右侧窗口打开仍只缩小中间编辑区，保留左列宽度。

## 复用边界

继续使用 `LocalDock::new(title_height).with_elastic_panel(panel_id).create_area(...)`；没有指定弹性面板的表面按具体窗口处理。Base 是唯一持久布局和面板注册来源。`dragging.rs` 只维护手势、成员与目标，`snapshot.rs` 冻结原始尺寸并在松手时生成 Base 编辑；`markers.rs` 负责无交互的覆盖绘制；`preview.rs` 只显示标题，不重复渲染面板内容。

本地 `Panel::drag_title` 提供不带工具栏的标题，插件使用贡献清单的显示名，普通面板使用相同的停靠机制。没有按 Git 或插件 ID 增加宿主专属分支，也没有修改公开插件协议。手势期间暂停会话捕获和空区域清理；注册或移除改变了成员集合时取消旧手势，避免覆盖新面板或恢复已卸载面板。

## 验证结果

本节保留标记预览阶段的实际记录；后续新增的方向限制及组合换向验证见[停靠区域方向限制](dock-regional-axis-2026-10-19.md)。

修改实现前新增 `marked_drag_keeps_original_bounds_until_release` 并执行：编辑区由原 x=280、宽1000提前扩展到 x=0、宽1280，回归失败，见 `target/marked-dock-red.log`。这确认上一版的实时让位路径与本轮规则冲突。

原生验收和普通面板回归使用独立测试程序 `target/marked-dock-green-runner/editor_app-4b9cc6829f4d50fc.exe`，SHA-256 `b1d898a3e9c995ea58f537dca2d1295046603874468276e53e6cadd54e03184c`。新增回归初次运行发现测试测量函数错误地假定 Editor 有停靠标题栏；修正为无标题时使用完整内容边界后，重新生成 `target/marked-dock-green2-runner/editor_app-26edf3f848a1dc34.exe`，SHA-256 `a0e29571c9842acb356c9c45d3640e5aa2b75a77d66ae4179900d998e1bf7b7c`。该次调整只修正测试边界测量，不改变拖放行为。

| 入口 | 实际结果 | 日志 |
| --- | --- | --- |
| 第二程序 `dock --test-threads=1` | 29 passed、0 failed、3 ignored | `target/marked-dock-green2-dock.log` |
| 第一程序 `sidebar --test-threads=1` | 10 passed、0 failed、1 ignored | `target/marked-dock-green-sidebar.log` |
| 第一程序 `app::session::tests --test-threads=1` | 8 passed、0 failed | `target/marked-dock-green-session.log` |
| 第一程序 `exclusive_tests --test-threads=1` | 3 passed、0 failed | `target/marked-dock-green-exclusive.log` |
| 第一程序 `tests::problems --test-threads=1` | 6 passed、0 failed | `target/marked-dock-green-problems.log` |
| 第二程序显式执行实际 XML 四方向 ignored 测试 | 1 passed，68.67 秒 | `target/marked-dock-outline-green2.log` |
| 第一程序显式执行实际 Git Windows/D3D11 ignored 测试 | 父测试 1 passed，两组原生子进程均通过，142.08 秒 | `target/marked-dock-native-green.log` |
| `cargo fmt --check` | exit 0 | `target/marked-dock-format-final.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78 组、304 passed、0 failed、311 ignored，exit 0 | `target/marked-dock-nonui.log` |
| `cargo check --workspace --jobs 1` | exit 0 | `target/marked-dock-workspace-check.log` |

停靠过滤包含 9 项新手势回归：拖动和外部移动期间全部实际边界不变；组合的两份来源／目标标记与标题小块保持顺序和比例；中央与外部无效落点；具体窗口四方向单独移动；底部实际高度超过目标一半但仍能容纳时保留原高度；实际宽度与超过整个目标才回退一半；右侧来源保留左列；编辑区四方向整组提交；从编辑区切换具体窗口时同步改变标记与小块成员。还保留底部完整跨度、右侧开关、用户缩放、快速松手及重开恢复覆盖。过滤入口相互重叠，不将其数量相加作为唯一测试数。普通过滤跳过的实际包用例按表中入口显式执行；其余 ignored 用例不计为通过。

实际 XML 测试使用固定账本 `crates/plugin-runtime/tests/fixtures/packages/inputs.json` 中的 `xml-0.2.2.zip`，SHA-256 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`。入口为 `tests::outline::docking::native_outline_four_directions_resize_and_workspace_restore --exact --ignored --test-threads=1`，通过实际插件与 GPUI TestPlatform 输入检查四方向移动、局部重排、缩放、结构刷新和工作区恢复。

## Windows 与实际 Git 插件

实际候选仍为隔离的 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`，未更改用户安装。设置 `NANOBUG_GIT_NATIVE_EXCLUSIVE=1`、`NANOBUG_GIT_CANDIDATE` 和输出目录 `NANOBUG_GIT_NATIVE_OUTPUT=target/marked-dock-native-green`，执行 `extensions::worker::native_acceptance::exclusive::git_native_exclusive_dock_replaces_and_restores_peers --exact --ignored --test-threads=1`。

| 原生组合 | 放置前后左列宽度 | 松手后底部 Git 实际范围 | 原生结果 |
| --- | --- | --- | --- |
| 深色中文 100% | 280 → 280 逻辑像素 | x=0，宽1280，高260 | 来源不移动、组合双标记与双卡片、插件单窗口标记、取消、上方行放置、右栏开关和拖回均通过 |
| 浅色英文 150% | 280 → 280 逻辑像素 | x=0，宽1285.3334，高260 | 同上；独占替换和恢复亦通过 |

两组 `target/marked-dock-native-green/{dark-zh-CN-100,light-en-150}/evidence.json` 中，`observations.group_marker_drag` 确认组合两成员、两标题小块、实际布局不变和 Escape 清除标记；`observations.plugin_source_marker_drag` 确认实际 Git 来源保持底部分配、紧凑标题和黄／蓝预览；`observations.editor_left_drop_complete` 确认松手后的底部跨度、左列宽度、右栏开关与拖回。两组均为 `shutdown_complete:true`、`observations.unexpected_error_logs:[]`。

已查看两组 `group-marker-preview.png`、深色 `plugin-source-marker-preview.png` 和 `editor-edge-placed.png`，确认抓取成员更明显、同行成员较淡、插件标题为贡献显示名，且真实内容仍在来源位置。原生输入走生产 `Window.dispatch_event` 和 WindowsPlatform/D3D11；未声称操作系统 SendInput 或 IME 候选窗验收。没有执行完整 UI 测试集、其余 ignored 用例、完整八组合矩阵或安装器打包。`git diff --check` 及四份相关文档的本地链接检查通过；原有并行工作区改动保留。

返回[原生 UI 目录](../README.md)。
