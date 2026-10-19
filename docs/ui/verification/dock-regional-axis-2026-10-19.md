# 停靠窗口的区域方向限制

> 本页保留上一阶段实现和验收。用户后续明确方向限制仅作用于一级区域，嵌套子窗口可以继续局部布局，并修正相等尺寸边界；当前规则及验证见[嵌套布局与拖动范围](dock-nested-layout-2026-10-19.md)。

## 确认规则

本轮在[标记预览式拖放](marked-dock-placement-2026-10-19.md)上增加区域方向限制，保留拖动期间原布局不动、默认采用拖动前实际尺寸、黄色来源、蓝色落点和标题小块。

| 目标区域 | 允许的窗口间放置 | 被拒绝的方向 |
| --- | --- | --- |
| 顶部、底部工具窗口区域 | 左、右 | 上、下 |
| 左侧、右侧工具窗口区域 | 上、下 | 左、右 |
| 编辑区本身 | 左、右、上、下 | 无方向限制，中央仍不合并标签 |

规则根据目标当前所在区域判断。位于 Base Center 树内、编辑区上／下方的工具条带同样采用左右排列，位于编辑区两侧的工具列同样采用上下排列。不是所有 Center 树中的窗口都被当成编辑区。未指定弹性锚点的通用 Center 表面保留四方向能力；原生左右／底部区域仍遵守各自限制。

命中检测直接使用允许的轴：侧栏角落按上／下方向解释，顶部或底部角落按左／右解释。禁止方向不显示蓝色预览，松手也不提交；从有效预览直接移到禁止边缘并在下一帧前松手，同样不会提交旧落点。

整组移入新区域时，保持成员顺序和原分配比例，按目标区域重新排列：顶部／底部组左右排列，两侧组上下排列。该条规则替代上一阶段跨方向移动时保留原组结构的部分。原实际外接宽高仍决定整组落点厚度，超过整个目标对应尺寸时才回退一半。

## 实现边界

`placement::stack_axis` 通过调用方指定的弹性面板 ID 和当前 Base 树识别区域，不按插件 ID 或窗口标题判断。预览命中、松手提交、组合标记和组合布局都采用同一目标轴；标记与 Base 布局共享同一份原始成员尺寸权重。宿主恢复时为缺少的大纲选择相邻方向，也复用该区域判断。插件使用相同面板机制，不扩展公开协议。

本轮约束新拖放操作，不主动重写用户已有会话中的历史分割。窗口被重新放置时按当前规则生成目标组合；已有尺寸和会话标识保留。

## 自动回归

最终独立程序为 `target/dock-axis-delivery-runner/editor_app-f05ab8b433d326bb.exe`，SHA-256 `c3d04d803e74c4bbe4c1cdcd3d7578542f558f099f73e73087aa57f77d3a3316`。通过 `cargo rustc -p editor-app --tests --jobs 1 -- -o C:/Projects/RustProjects/Editor/target/dock-axis-delivery-runner/editor_app.exe` 构建，日志 `target/dock-axis-build-delivery.log`。独立输出沿用前阶段的共享测试 EXE 占用规避方式，未替换其他任务使用的程序。

| 最终入口 | 实际结果 | 日志 |
| --- | --- | --- |
| 独立程序 `dock --test-threads=1` | 33 passed、0 failed、3 ignored | `target/dock-axis-dock-final.log` |
| `sidebar --test-threads=1` | 10 passed、0 failed、1 ignored | `target/dock-axis-sidebar-final.log` |
| `app::session::tests --test-threads=1` | 8 passed、0 failed | `target/dock-axis-session-final.log` |
| `exclusive_tests --test-threads=1` | 3 passed、0 failed | `target/dock-axis-exclusive-final.log` |
| `tests::problems --test-threads=1` | 6 passed、0 failed | `target/dock-axis-problems-final.log` |
| 实际 XML 四方向 ignored 测试 | 1 passed、81.75 秒 | `target/dock-axis-outline-final.log` |
| `cargo fmt --check` | exit 0 | `target/dock-axis-format-final.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78 组、304 passed、0 failed、311 ignored，exit 0 | `target/dock-axis-nonui.log` |
| `cargo check --workspace --jobs 1` | exit 0 | `target/dock-axis-workspace-check-final.log` |

新增回归覆盖原生左／右／底部禁止方向、角落沿合法方向放置、编辑区四方向、Center 内上下条带和侧边工具列、组合预览与实际放置的顺序和比例、有效预览后直接在禁止方向松手，以及无编辑区锚点的通用 Center。旧有回归继续验证原位标记、底部跨度、右栏显隐保留左列宽度、实际默认尺寸、取消、缩放与重开。各过滤入口存在重叠，不将数量相加为唯一用例数。

首次方向过滤执行为 32 passed、1 failed、3 ignored（`target/dock-axis-dock.log`）：通用 Center 测试将外层单子节点框架的轴当成实际分叉方向。现沿不可变树越过该框架后检查实际分叉；未改变生产布局以迎合该断言。最终过滤全部通过。

实际 XML 使用固定 `xml-0.2.2.zip`，SHA-256 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`，入口 `tests::outline::docking::native_outline_four_directions_resize_and_workspace_restore --exact --ignored --test-threads=1`。首轮在大纲加载阶段失败，日志 `target/dock-axis-outline-initial.log`；最终程序单独复验通过所有方向、局部重排、真实分隔条、编辑刷新和恢复。未将首次失败计为通过，也未确认首次结构激活失败的根因。该测试使用 GPUI TestPlatform，不能等同于 Windows/D3D11 原生验收。

## Windows 与实际 Git 插件

使用既有隔离候选 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`，未修改用户安装。设置 `NANOBUG_GIT_NATIVE_EXCLUSIVE=1`、`NANOBUG_GIT_CANDIDATE`，显式执行 `extensions::worker::native_acceptance::exclusive::git_native_exclusive_dock_replaces_and_restores_peers --exact --ignored --test-threads=1`。

第一次原生验收在 `target/dock-axis-native-final` 保留失败证据：底部插件拖动时黄色来源正常，但蓝色目标缺失，标题卡片出现在窗口顶边。为区分放置判断与平台消息等待期间的坐标变化，原生测试增加指定坐标与 `window.mouse_position()` 的对照记录，并在平台帧后将生产鼠标派发与实际绘制放在同一更新内采样；没有直接修改停靠树或移动系统光标。

该项只修改测试输入采样。复验程序为 `target/dock-axis-native-runner/editor_app-90d5c7ef0b431fd8.exe`，SHA-256 `a7f2da22166058521f9bc7a104ffa2e1a60cca5f3378f8404d4195136d4ebd57`；构建日志 `target/dock-axis-native-build.log`。同一程序的 `dock --test-threads=1` 再次为 33 passed、0 failed、3 ignored，日志 `target/dock-axis-dock-verified.log`。

`target/dock-axis-native-verified` 的第二次 100% 场景已通过组合预览和区域限制断言，但在 `editor-edge-right-closed.png` 的 DPI 校验失败：实际 `scale_factor:1.5`、客户区1922×1351，与请求100%不符。其 `observations.group_marker_drag` 和 `observations.regional_split_limits` 均为 true，不能据此把完整100%场景标记通过。首轮预览消失未再次出现；本轮坐标记录没有采到被覆盖的输入，因此不将首轮失败的根因写成已确认。

后续改为当前显示缩放下的 `dark-zh-CN-150` 与 `light-en-150`，使用同一原生子测试 `extensions::worker::native_acceptance::git_native_matrix_child --exact --ignored --nocapture --test-threads=1`。每次由 PowerShell 父进程准备全新私有根、profile、空 Git 配置及独立输出，设置 `NANOBUG_GIT_NATIVE_CASE`、`NANOBUG_GIT_NATIVE_ROOT` 和对应 case 输出路径；复用正常 Fixture、实际 Manager、WindowsPlatform 与完整独占／拖放流程。没有修改系统 DPI、用户 Git 配置、用户安装或生产放置规则。

| 原生组合 | 实际结果 | 输出 |
| --- | --- | --- |
| 深色中文150% | 1 passed、92.26秒；退出与资源清理成功 | `target/dock-axis-native-dark-150.log`，`target/dock-axis-native-150/dark-zh-CN-150/` |
| 浅色英文150% | 1 passed、96.08秒；退出与资源清理成功 | `target/dock-axis-native-light-150.log`，`target/dock-axis-native-150/light-en-150/` |

两组150%凭据中 `observations.group_marker_drag.top_preview_horizontal_and_side_preview_vertical:true`，`observations.regional_split_limits` 四项均为 true；原有上方行、完整底部 Git 跨度、右栏开关保留左列宽度和拖回同样通过。两组均为 `shutdown_complete:true`、`observations.exclusive_complete:true`、`observations.unexpected_error_logs:[]`，各19张截图的实际缩放均为1.5，必需图像与资源清理凭据已读回核对。没有把100%中断的父测试计为通过。

已查看深色顶部组合和底部合法预览、浅色顶部组合截图，确认顶部预览成员左右排列、来源仍保持原位。浅色150%的插件来源手势实际记录到：请求坐标为(290,322)，平台帧后却为(676,12.666667)，与首次标题卡片跑到窗口顶边的现象一致。同更新重新派发并绘制后仍通过全部放置断言，确认原生测试确实存在平台等待期间的坐标覆盖，需要隔离该输入采样干扰；未捕获具体外部消息来源。

原生输入走生产 `Window.dispatch_event` 和 WindowsPlatform，截图读取实际 D3D11 目标；没有操作系统 SendInput 或 IME 候选窗验收。本轮未运行完整 UI 测试、其余 ignored 用例或八组合矩阵。最终 `git diff --check`、相关文件行尾空白和四份相关文档本地链接检查通过；原有并行工作区改动保留。

返回[原生 UI 目录](../README.md)。
