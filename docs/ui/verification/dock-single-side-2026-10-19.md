# 单侧唯一组合与子窗口拖放

> 本页保留上一阶段的实现与实际验证。用户后续确认一级组合中的任意成员拖向本侧时应整组联动；“标题始终只移动自身”和“本侧已有成员一律拒绝”已由[四侧组合拖放修正](dock-group-fallback-2026-10-19.md)替代，其他单窗口与嵌套边界继续有效。

## 本轮确认规则

编辑区域左、右、上、下每一侧最多放置一个独立窗口或一个窗口组合。组合内部的成员数和嵌套分栏不作为多个外围区域计算。

窗口标题发起的拖动只携带该窗口，不因悬停编辑区而扩大到整个来源组合；没有“主窗口”或按窗口顺序区别对待的规则。本轮替代[上一阶段](dock-nested-layout-2026-10-19.md)“直接一级成员悬停编辑区即整组移动”的判断，保留真实布局在松手前不动、黄／蓝斜线标记和紧凑标题预览。

| 子窗口的落点 | 结果 |
| --- | --- |
| 编辑区空的一侧 | 单独放置；原组合的其他成员补位 |
| 编辑区已有窗口／组合的一侧 | 拒绝另建独立区域；无可放置预览，松手不修改布局 |
| 已有具体窗口的合法边缘 | 在该区域内分栏，只移动被拖窗口，不合并标签页 |
| 来源所在侧只剩该可见窗口 | 排除来源后该侧为空，允许重放到编辑区同侧 |
| 嵌套子窗口移出 | 同级成员补满局部单元，Base 收拢空节点／单子节点；无关分栏保留 |
| Escape、中央、外围或其他非法落点 | 清除手势，保持原布局 |

左右区域一级上下排列，顶部／底部一级左右排列；已有分栏单元内部可继续四方向嵌套。放入已有区域须命中具体窗口的合法边缘，不能借编辑区边缘另建该侧的第二个区域。

占用同时包含原生 Left／Right／Bottom 及 Center 树内各层围绕编辑区的条带；顶侧由 Center 树表示。隐藏、被独占替换、已经关闭的外围区域不占用可见侧。松手重新检查实时占用，不能依靠旧预览提交。

默认尺寸继续使用开始拖动前包含标题的实际宽高；对应尺寸 `>=` 目标完整尺寸时取半。最终极小空间由 Base 最小尺寸约束。右侧出现或子窗口移出不改变留下的左栏宽度。

## 实现与公共边界

- `LocalDock::with_elastic_panel` 由调用方指定弹性中心的面板身份，不依赖 Editor 名称、插件 ID 或业务白名单。
- `placement::editor_side_available` 读取 Base 当前树及可见性，沿中心面板路径检查所有相关祖先和同侧兄弟，不维护第二份持久布局。
- 单窗口 `DragPanel` 的来源节点在开始时冻结；悬停只切换目标，不再推导父组合成员。现有 Base 插入、归一化、布局持久化和窗口视图保持复用。
- 宿主和插件面板使用同一组件；公开 SDK、能力版本、插件包和用户安装没有变更。`editor-ui` 迁移仍另行进行。

## 验证记录

修改生产实现前新增三项真实标题输入回归，通过独立 `dock-one-side-red-runner` 测试程序执行 `child_drag_ --test-threads=1`：**0 passed、3 failed，0.39秒**，见 `target/dock-one-side-red.log`。失败分别是来源标记错误包含 Explorer、已有左栏仍接受新列、已有底栏仍接受新底层。

首次实现后相同回归为 **3 passed、0 failed，0.47秒**，日志 `target/dock-one-side-fix-smoke.log`。完整停靠过滤当时为36 passed、10 failed，失败用例仍断言已被新需求替代的隐式组合拖动或选择了已占用的目标侧；结果保存在 `target/dock-one-side-fix-dock.log`，没有计为通过。

补齐所有四侧的中心内／祖先条带、原生右侧、空底侧、现有底部窗口分栏、最后成员重放、关闭区域和即时松手后，新增过滤为 **10 passed、0 failed，3.50秒**；完整停靠过滤为 **53 passed、0 failed、3 ignored，17.69秒**，日志 `target/dock-one-side-regression-smoke.log`、`target/dock-one-side-regression-dock.log`。

增加“同一指针位置松手前显现同侧兄弟”的真实可见性动作回归后，共11项新增回归。它验证不能因为指针未移动就沿用旧的有效预览。最终 `cargo test -p editor-app dock --jobs 1 -- --test-threads=1` 为 **54 passed、0 failed、3 ignored，17.64秒**，日志 `target/dock-one-side-cargo-dock.log`。新增11项包含在54项中，不重复相加。

| 最终检查入口 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo test -p editor-app sidebar --jobs 1 -- --test-threads=1` | 10 passed、0 failed、1 ignored | `target/dock-one-side-cargo-sidebar.log` |
| `cargo test -p editor-app app::session::tests --jobs 1 -- --test-threads=1` | 8 passed、0 failed | `target/dock-one-side-cargo-app-session-tests.log` |
| `cargo test -p editor-app exclusive_tests --jobs 1 -- --test-threads=1` | 3 passed、0 failed | `target/dock-one-side-cargo-exclusive_tests.log` |
| `cargo test -p editor-app tests::problems --jobs 1 -- --test-threads=1` | 6 passed、0 failed | `target/dock-one-side-cargo-tests-problems.log` |
| `cargo fmt --check` | exit 0 | `target/dock-one-side-final-format.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78组、304 passed、0 failed、311 ignored，exit 0 | `target/dock-one-side-nonui.log` |
| `cargo check --workspace --jobs 1` | exit 0，保留既有警告 | `target/dock-one-side-workspace.log` |

## 实际 Windows 与插件验收

最终原生和 XML 程序为 `target/dock-one-side-native-runner/editor_app-c7bb9a3251edc090.exe`，SHA-256 `29e330c83f088ffa28cc3fcdf641255781fbeeae383221c74bfc5bd0e7034414`，构建日志 `target/dock-one-side-native-verified-build.log`。直接使用 `cargo rustc -p editor-app --tests --jobs 1 -- -o C:/Projects/RustProjects/Editor/target/dock-one-side-native-runner/editor_app.exe` 构建独立程序；Cargo UI 过滤使用同一最终源码，未复制到其他工作区或共用其他源码快照的输出。

Windows 使用既有隔离 Git 候选 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`。每次准备独立私有根、profile 和空 Git 配置，设置 `NANOBUG_GIT_NATIVE_EXCLUSIVE=1`、候选、case、根、输出路径及 `ME_EDITOR_PROFILE_HOME`；不修改用户安装。执行 `extensions::worker::native_acceptance::git_native_matrix_child --exact --ignored --nocapture --test-threads=1`。

| Windows 场景 | 实际结果 | 日志／输出 |
| --- | --- | --- |
| 深色中文150% | 1 passed、0 failed，108.63秒 | `target/dock-one-side-native-dark-verified.log`；`target/dock-one-side-native-verified-150/dark-zh-CN-150/` |
| 浅色英文150% | 1 passed、0 failed，106.13秒 | `target/dock-one-side-native-light-verified.log`；`target/dock-one-side-native-verified-150/light-en-150/` |

已读回两份 `evidence.json`：各36张截图，实际缩放均为1.5；`shutdown_complete`、`exclusive_complete` 为 true，`single_side_child_placement`、`nested_dock_placement` 的全部行为标志为 true。非预期错误为0，项目关闭、输入退役、原生窗口退役均成功，剩余窗口为0。已查看深色单子窗口右侧预览、浅色加入已有底部组合的放置图，来源、目标及同级窗口位置正确。

这些流程通过生产 `Window.dispatch_event` 和实际 WindowsPlatform／D3D11 验证：同侧原生左／底栏拒绝新区域、空右侧独立移动、中心内右侧占位、剩余左栏宽度、具体 Git 窗口左右分栏、剩余底部成员阻止再开底层、返回原组合；同时保留独占切换、右栏宽度、底部跨度、尺寸相等取半与嵌套补位验收。

首轮原生程序保存在 `target/dock-one-side-native-initial/editor_app-c7bb9a3251edc090.exe`，SHA-256 `6e76d02c7e8b2174876712d2091c8f766d7c4d98073ba04a0ac2edddcaddebe5`。该轮新增子窗口场景均已通过，随后旧区域方向验收失败，日志 `target/dock-one-side-native-dark.log`。返回左侧的大纲恰好与编辑区底边对齐，旧坐标选择器将大纲认成 Git，向错误目标的禁用方向拖动而缺少预览。验收现在按真实标题路径排除宿主面板，再定位 Git，保留方向、布局和松手断言，未为此修改生产放置规则。

XML 使用固定 `xml-0.2.2.zip`，实际 SHA-256 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`，与 `crates/plugin-runtime/tests/fixtures/packages/inputs.json` 一致。实际分发 WASM／grammar 经公开 Manager 安装，入口为 `tests::outline::docking::native_outline_four_directions_resize_and_workspace_restore --exact --ignored --nocapture --test-threads=1`。左边缘场景先通过真实输入把 Explorer 移到空顶侧，随后仅移动 Outline，避免以隐式整组移动绕过新约束；再通过具体窗口边缘组合并调整分隔条，验证编辑刷新和重开恢复。

首次 XML 为1 passed、0 failed、84.13秒，日志 `target/dock-one-side-xml.log`。最终程序一次并行复验在实际结构节点加载阶段失败，`outline-node-/0/0` 未出现且面板显示失败状态，见 `target/dock-one-side-xml-verified.log`；该次未计为通过，也未据此改变拖放行为、放宽结构断言或延长等待。并行任务结束后，**同一最终程序单独复验为1 passed、0 failed、68.44秒**，日志 `target/dock-one-side-xml-serial.log`。加载失败的具体原因未确定，保留此限制与两次实际结果。

## 验证范围

普通过滤中的 ignored 未计为通过；仅按上述入口显式验收实际 XML 和两个 Windows 子场景。未运行完整 UI 测试、其余 ignored、完整八组合 DPI 矩阵、操作系统 SendInput 或系统 IME 候选窗验收。本次约束新的拖放，不清空或重排历史会话；继续按 Base 恢复既有布局。

`git diff --check`、13份本次涉及源码的行尾空白及5份相关文档的本地链接检查通过。未提交、推送、发布或修改用户安装；保留工作区已有改动。

返回[原生 UI 目录](../README.md)。
