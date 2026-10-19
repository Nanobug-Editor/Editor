# 四侧组合拖放与单侧唯一占用

## 本轮确认规则

用户新增三成员示意图明确：原左侧一级组合中的窗口1、3、2，抓取任意一个拖向编辑区左缘时，整个组合一起移动。移走原组合后左侧不再另有区域，因此允许放置；原底部窗口4扩展到完整底部。右侧、顶部、底部组合使用同一判断，没有“主窗口”或只允许首成员触发的限制。

本轮补充[单侧唯一组合与子窗口拖放](dock-single-side-2026-10-19.md)，保留每侧最多一个独立窗口或组合。该阶段把所有标题拖动固定成单窗口，导致整组拖动回退；下表替代其“标题始终只移动自身”和“本侧已有成员一律拒绝”的说明。

| 命中目标及来源层级 | 移动范围与结果 |
| --- | --- |
| 编辑区空侧 | 只移动抓取窗口，原组合的其他成员留在原区域 |
| 编辑区本组所在侧，抓取一级直接成员 | 先检查单窗口范围；若只有来源组合占用该侧，改为整组移动，排除整组后再次检查占用 |
| 排除来源整组后仍有其他区域占用该侧 | 拒绝另建第二个区域；不能自动带走或合并另一个区域 |
| 嵌套组合里的子窗口 | 始终只移动自身；本侧仍被兄弟占用时拒绝，不能提升为整区拖动 |
| 已有具体窗口的合法边缘 | 只移动抓取窗口，加入该区域的局部分栏，不合并标签页 |
| 指针切换目标 | 同步重算来源、目标标记和紧凑标题卡片的成员；不锁定上一个落点的范围 |
| 有效位置松手 | 再次计算当前成员及实时占用，一次提交 Base 布局，释放空来源区域 |
| Escape、外部、中央或非法位置松手 | 清除标记和卡片，原布局不变 |

组合迁移保留成员顺序、原始分配比例及单元内部嵌套结构。左右一级上下排列，顶部／底部一级左右排列；内部单元可继续四方向嵌套。默认厚度取拖动开始前整个移动范围的实际外接宽／高，包含标题；对应尺寸 `>=` 整个目标尺寸时采用目标的一半，最终仍遵守 Base 最小窗格约束。

占用同时检查原生 Left／Right／Bottom 和 Center 内编辑区路径各层的同侧兄弟。隐藏、被独占替换和关闭区域不占用可见侧。成员范围从开始时的可见快照推导；中途新显现但未参与本次拖动的同侧窗口会阻止旧预览提交。

## 组件与插件边界

`LocalDock::with_elastic_panel` 继续由调用方提供弹性中心身份，宿主与插件复用同一手势。`placement::Band::drag_group` 只允许一级直接成员取得来源组合；`Snapshot` 保存手势期间的不可变关系和实际尺寸；`Session::editor_members` 根据落点选择单窗口或整组；`editor_side_available` 排除整个移动集合后检查 Base 的当前可见树。

没有增加按窗口名称、插件 ID 或业务类型区分的分支，没有修改公开 SDK、插件包、用户安装或依赖。`editor-ui` 迁移仍是后续工作。

## 验证记录

新增 `extensions::dock_tests::drag::live::grouped::` 的六项真实标题输入回归，覆盖四侧各三个一级成员、即时松手、范围切换与取消、另一个同侧区域阻挡、嵌套子窗口不提升，以及整组携带嵌套兄弟结构。

修改前独立程序 `target/dock-group-red-runner/editor_app-15e561580e501764.exe` 执行该过滤为 **2 passed、4 failed，2.75秒**，见 `target/dock-group-red.log`。失败是缺少整组标记、卡片与快速松手未迁移全部成员；两个已有通过项仍检查无关占用和嵌套子窗口边界。

核心修改后独立程序 `target/dock-group-green-runner/editor_app-c66eac8a70de7fba.exe` 的相同过滤为 **6 passed、0 failed，11.26秒**，见 `target/dock-group-green-smoke.log`。其完整停靠过滤为 **55 passed、5 failed、3 ignored，33.16秒**；五项仍断言旧的“本侧已有成员就拒绝”，保留日志 `target/dock-group-green-dock.log`，没有把该轮报告为全通过。后续同步这些被新规则替代的断言，并强化原生区域已释放、成员进入 Center 和磁盘恢复检查。

最终使用 `cargo test -p editor-app dock --jobs 1 -- --test-threads=1` 构建并执行同一源码，结果为 **60 passed、0 failed、3 ignored，32.43秒**，见 `target/dock-group-final-dock.log`。新增六项包含在60项中，不重复相加。该命令生成的 `target/debug/deps/editor_app-81b56278c415338a.exe` 复制到独立运行位置 `target/dock-group-final-runner/`，SHA-256 `6c84229363e18c3db49ecd7a3a5ab953d62d9bb6ab69459b863c52acf628d5d2`；后续原生和实际 XML 验收均运行这份冻结程序，没有覆盖用户正在运行的宿主。

| 最终检查入口 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo test -p editor-app dock --jobs 1 -- --test-threads=1` | 60 passed、0 failed、3 ignored | `target/dock-group-final-dock.log` |
| 冻结程序 `sidebar --test-threads=1` | 10 passed、0 failed、1 ignored，0.96秒 | `target/dock-group-sidebar.log` |
| 冻结程序 `app::session::tests --test-threads=1` | 8 passed、0 failed，1.24秒 | `target/dock-group-app--session--tests.log` |
| 冻结程序 `exclusive_tests --test-threads=1` | 3 passed、0 failed，2.85秒 | `target/dock-group-exclusive_tests.log` |
| 冻结程序 `tests::problems --test-threads=1` | 6 passed、0 failed，2.58秒 | `target/dock-group-tests--problems.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78组、304 passed、0 failed、311 ignored，exit 0 | `target/dock-group-nonui.log` |
| `cargo check --workspace --jobs 1` | exit 0，保留既有警告 | `target/dock-group-workspace.log` |
| 本轮 Rust 文件 `rustfmt --check --edition 2024 --config skip_children=true` | 通过 | `target/dock-group-owned-format.log` |
| 全仓 `cargo fmt --check`、`git diff --check` | 被未修改的 `crates/editor-app/src/ui/controls.rs:54` 行尾空格阻挡；没有顺手修改该已有工作区改动 | `target/dock-group-format.log`、`target/dock-group-diff-check.log` |

本轮源码及六份相关文档的行尾检查与本地链接检查通过；全仓源码无 `gpui_component`／`gpui_kit::component` 调用，`cargo tree --workspace --prefix none` 无 `gpui-component` 依赖，见 `target/dock-group-deps.log`。没有将普通过滤中的 ignored 用例算作通过。

## 实际 Windows 与插件

Git 使用既有隔离候选 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`。设置 `NANOBUG_GIT_NATIVE_EXCLUSIVE=1`、`NANOBUG_GIT_CANDIDATE` 及 `NANOBUG_GIT_NATIVE_OUTPUT=target/dock-group-native`，执行 `extensions::worker::native_acceptance::exclusive::git_native_exclusive_dock_replaces_and_restores_peers --exact --ignored --nocapture --test-threads=1`。父测试为 **1 passed，248.68秒**，见 `target/dock-group-native.log`。

| 实际 Windows 组合 | 原生子进程结果 | 输出 |
| --- | --- | --- |
| 深色中文100%，实际缩放1.0 | 1 passed，104.86秒；57张截图 | `target/dock-group-native/dark-zh-CN-100/` |
| 浅色英文150%，实际缩放1.5 | 1 passed，143.55秒；57张截图 | `target/dock-group-native/light-en-150/` |

每组通过真实标题输入依次组成左／右上下组合、顶部左右组合，以及资源管理器、大纲、实际 Git 面板的三成员底部组合。每个成员都经历整组预览或提交，底部抓取实际 Git 标题也携带全部成员。原始可见内容在悬停时不动，来源／目标标记和紧凑卡片使用相同成员；Escape 保持原布局，其他空侧与具体窗口仍可单独移动。

两份 `evidence.json` 中 `observations.four_side_group_fallback` 的四方向、任意成员、实际插件、标记成员、布局不动及取消断言均为 true；`exclusive_complete` 和 `shutdown_complete` 为 true，非预期错误列表为空。进程退出记录中的 GPUI 窗口和原生非子窗口均为空。已查看最终深色右侧放置、深色／浅色三成员底部预览，以及浅色顶部放置截图。输入经生产 `Window.dispatch_event` 与 WindowsPlatform/D3D11，未声称操作系统 SendInput 或 IME 候选窗验收。

实际 XML 使用固定 `crates/plugin-runtime/tests/fixtures/packages/xml-0.2.2.zip`，SHA-256 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`。显式运行 `tests::outline::docking::native_outline_four_directions_resize_and_workspace_restore --exact --ignored --test-threads=1` 为 **1 passed，72.45秒**，见 `target/dock-group-outline.log`；覆盖公开插件接入后的四方向移动、局部分栏、缩放、结构刷新与磁盘恢复。该项使用 GPUI TestPlatform，不等同于上表的 Windows/D3D11 验收。

没有执行其余 ignored 测试、完整 UI 测试集、完整八组合原生矩阵、Release 或安装器打包；没有自动提交、推送、发布或替换用户插件安装。

返回[原生 UI 目录](../README.md)。
