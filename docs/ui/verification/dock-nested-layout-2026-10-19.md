# 停靠窗口的嵌套布局与拖动范围

> 本文保留上一阶段实现及实际验收记录。当前标题拖动范围按[四侧组合拖放修正](dock-group-fallback-2026-10-19.md)执行：本侧一级成员可带动整组，空侧／具体窗口只带动自身，嵌套子窗口不提升；[单侧占用阶段](dock-single-side-2026-10-19.md)保留历史验证。

## 用户确认的规则

本轮补充[区域方向限制](dock-regional-axis-2026-10-19.md)的适用层级，并保留[标记预览式拖放](marked-dock-placement-2026-10-19.md)的交互和通用组件边界。

| 层级／操作 | 当前规则 |
| --- | --- |
| 区域一级布局 | 顶部、底部左右分栏；两侧上下分栏 |
| 一级分栏内部 | 可以继续四方向拆分，并继续嵌套子窗口布局 |
| 一级成员拖到编辑区边缘 | 保留原来的组合移动，按目标区域排列一级成员；成员内部的嵌套布局保留 |
| 嵌套组合内的子窗口拖到编辑区边缘 | 只移动该子窗口，不提升为整区组合移动 |
| 拖到具体工具窗口边缘 | 只移动抓取的窗口，落点按区域一级／局部分栏层级判定 |
| 原实际宽／高大于或等于目标完整宽／高 | 对应落点尺寸采用目标的一半；小于时保留原实际尺寸 |

区域尚无一级分栏时，首次拆分遵守区域方向。已有一级分栏后，在某个分栏窗口上放置会创建或继续局部布局。同区移动会先排除来源检查剩余分栏，不能借移走最后一个同级窗口，把一级区域改成禁止方向。编辑区自身仍允许四方向，中央不合并标签。

例如底部为 `[上下(窗口1, 窗口3), 窗口2]`：一级左右布局合法，左侧单元中的上下子布局也合法。拖窗口1到编辑区左侧，只标记和移动窗口1；松手后窗口3补满原左侧单元，窗口2的位置不变。拖一级直接成员窗口2到编辑区边缘时，仍按原有规则组合移动，并保留窗口1、窗口3的内部布局。

尺寸使用拖动开始前包含标题的实际分配。左右落点比较宽度，上下落点比较高度，判断为 `desired >= available`。拖动期间真实窗口保持原位，黄色来源、蓝色目标和紧凑标题卡片同步反映当前成员；取消只清除手势。极小空间仍由 Base 最小窗格约束决定最终可分配尺寸。

## 实现边界

`placement::region_band` 从 Base 当前规范树临时借用区域和一级分栏；`Band::drag_group` 区分直接成员和嵌套后代，`drop_axis` 区分一级分栏与局部拆分。Center 中的工具条带通过调用方提供的弹性面板 ID 判定，未指定弹性面板的通用 Center 保留四方向能力。

`dragging/snapshot/group.rs` 让预览和提交共用一级单元的原始尺寸权重，只转换组合最外层排列轴，不再将所有后代平铺。嵌套节点、视图、注册身份及持久布局仍全部交给 Base；没有第二套持久布局，也没有插件 ID 分支、插件私有 API 或 SDK 变更。`editor-ui` 迁移继续按之前约定另行处理。

Base 继续归一化空节点、单子节点及同轴分割，层级判断读取归一化后的当前树。隐藏面板保留注册和位置，重建时不再以过时像素偏好参与可见空间分配。嵌套成员离开侧栏时保留侧栏原宽度；仅历史并行外侧列可能释放整列宽度。同区重排先计算来源移走后目标补满的实际单元，再放入来源，避免把来源尺寸随被腾空单元再次放大。

## 验证记录

修正前通过独立程序 `target/dock-nested-red-runner/editor_app-41246a456dd45036.exe` 执行 `extensions::dock_tests::drag::live::nested --test-threads=1`，结果为 **0 passed、3 failed**，日志 `target/dock-nested-red.log`：

- 实际宽度等于目标1000时仍预览1000，期望500。
- 嵌套窗口拖到编辑区时错误出现 `local-dock-source-1`。
- 底部已有左右分栏，其单元仍拒绝局部上下布局。

最终停靠实现使用独立程序 `target/dock-nested-minimum-runner/editor_app-74acc0fcf02f509b.exe`，SHA-256 `3f32688e9777aa6d9081c7d0850c91bf37d9c6880f83feec757932c3d10a8e75`。构建入口为 `cargo rustc -p editor-app --tests --jobs 1 -- -o C:/Projects/RustProjects/Editor/target/dock-nested-minimum-runner/editor_app.exe`，日志 `target/dock-nested-minimum-build.log`。独立 EXE 避免覆盖其他任务使用的测试程序。

| 最终入口 | 实际结果 | 日志 |
| --- | --- | --- |
| `dock --test-threads=1` | 43 passed、0 failed、3 ignored | `target/dock-nested-minimum-dock.log` |
| `sidebar --test-threads=1` | 10 passed、0 failed、1 ignored | `target/dock-nested-minimum-sidebar.log` |
| `app::session::tests --test-threads=1` | 8 passed、0 failed | `target/dock-nested-minimum-app-session-tests.log` |
| `exclusive_tests --test-threads=1` | 3 passed、0 failed | `target/dock-nested-minimum-exclusive_tests.log` |
| `tests::problems --test-threads=1` | 6 passed、0 failed | `target/dock-nested-minimum-tests-problems.log` |
| 实际 XML 四方向／分隔条／恢复（显式 ignored） | 1 passed、0 failed、78.73秒 | `target/dock-nested-minimum-xml.log` |
| Windows 深色中文150%（显式 ignored） | 1 passed、0 failed、93.50秒 | `target/dock-nested-minimum-native-dark.log` |
| Windows 浅色英文150%（显式 ignored） | 1 passed、0 failed、91.90秒 | `target/dock-nested-minimum-native-light.log` |
| `cargo fmt --check` | exit 0 | `target/dock-nested-minimum-format.log` |
| `cargo check --workspace --jobs 1` | exit 0 | `target/dock-nested-minimum-workspace.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78组、304 passed、0 failed、311 ignored，exit 0 | `target/dock-nested-minimum-nonui.log` |

10项新增回归覆盖：左右／上下落点的实际尺寸相等边界；底部嵌套成员向编辑区四方向独立移动；在一级单元内创建上下布局；左右侧栏的局部横向布局及子窗口单独移出；第三级成员的取消与局部回收；一级成员移动保留嵌套兄弟拓扑；同区来源移走后仍遵守一级轴；同组合重排保留实际高度及其他单元；半尺寸小于 Base 下限时提交仍保持单元总尺寸。原有行跨度、右栏宽度、即时松手、无效落点、缩放和会话过滤继续通过。各过滤入口有重叠，不相加作为唯一测试数。

首轮完整过滤为37 passed、4 failed、3 ignored，日志 `target/dock-nested-dock-initial.log`；改为同帧稳定测量后的复验仍失败，日志 `target/dock-nested-dock-final.log`，没有将采样调整当作布局修复。进一步修正隐藏权重、剩余侧栏宽度及同组合目标分配后，中间9项为6 passed、3 failed，日志 `target/dock-nested-allocation-tests.log`。剩余夹具需保留隐藏注册成员，按可见单元断言，并为第三级拆分预留足够宽度，避免把 Base 的100像素下限或标题内容溢出当成普通比例空间；最终仍断言来源单独移动、其他可见单元不变和原尺寸保留，没有放宽这些行为断言。

## 实际插件与 Windows 原生验收

使用既有隔离候选 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`。每次由父 PowerShell 准备全新私有根和 profile，指定空 Git 配置；不修改用户安装或配置。原生子入口为 `extensions::worker::native_acceptance::git_native_matrix_child --exact --ignored --nocapture --test-threads=1`，设置 `NANOBUG_GIT_NATIVE_EXCLUSIVE=1`、`NANOBUG_GIT_CANDIDATE`、`NANOBUG_GIT_NATIVE_CASE`、`NANOBUG_GIT_NATIVE_ROOT`、`ME_EDITOR_PROFILE_HOME` 和独立 `NANOBUG_GIT_NATIVE_OUTPUT`。

生产 `Window.dispatch_event` 驱动真实 WindowsPlatform 和 D3D11。新增流程先验证完整目标宽度相等时取一半，再通过标题拖动创建底部左右一级布局、左单元内上下子布局，将其中资源管理器独立移到编辑区，最后确认存留一级成员仍可组合拖动。每步检查来源／目标／卡片的成员数量、松手前真实分配、兄弟补位和 Git 单元不变。原有独占、右栏宽度、组合换向、禁止一级方向与回拖流程一起执行；平台帧后的指定输入和绘制沿用前阶段已核验的同更新采样。

最终程序的两组完整复验均通过，输出为 `target/dock-nested-minimum-native-150/dark-zh-CN-150/` 与 `target/dock-nested-minimum-native-150/light-en-150/`。已读回两份 `evidence.json`：各26张截图，实际缩放均1.5；`shutdown_complete`、`exclusive_complete` 及 `nested_dock_placement` 全部行为断言均为 true，非预期错误为0。项目退出、输入退役、无原生窗口所有者及窗口退役均成功，剩余窗口为0。已查看最终深色子窗口预览和浅色放置图，来源／目标成员及大纲补位与示意图一致。

以下保留最小尺寸修正前一版 `editor_app-f993da9b60620877.exe`（SHA-256 `b7babfb2ae1b6d507ece499b8ea9794f1d54bac7d62b8ca32165ee2de793b416`）的完整原生记录；最终结果以上述复验为准。

| 前版原生组合 | 实际结果 | 输出 |
| --- | --- | --- |
| 深色中文150% | 1 passed、106.72秒 | `target/dock-nested-native-dark-delivery.log`，`target/dock-nested-native-delivery-150/dark-zh-CN-150/` |
| 浅色英文150% | 1 passed、107.73秒 | `target/dock-nested-native-light-delivery.log`，`target/dock-nested-native-delivery-150/light-en-150/` |

前版两组同样为 `shutdown_complete:true`、`exclusive_complete:true`、`unexpected_error_logs:[]`，行为与资源清理断言全部为 true。各26张图像的实际缩放为1.5，深色放置和浅色预览截图已核对；没有把前版结果替代最终程序的复验。

## XML 四方向、分隔条及恢复

实际 XML 使用固定 `xml-0.2.2.zip`，SHA-256 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`，账本为 `crates/plugin-runtime/tests/fixtures/packages/inputs.json`。入口为 `tests::outline::docking::native_outline_four_directions_resize_and_workspace_restore --exact --ignored --test-threads=1`，实际覆盖四方向、具体窗口重排、真实分隔条、编辑刷新与重开恢复，使用 GPUI TestPlatform，不等同于上述 Windows/D3D11 原生验收。

首次执行在固定方向的分隔条断言失败，日志 `target/dock-nested-xml-delivery.log`。原尺寸放置及相等取半后，重排可能使一个成员已位于 Base 最小尺寸，原测试继续朝缩小该成员的方向拖动。验收现在根据剩余空间选择正／负45像素，保留真实输入和比例改变断言，没有修改 Base 最小尺寸。

该测试调整后的程序 `target/dock-nested-verified-runner/editor_app-5eb7c6962e000d10.exe`，SHA-256 `ed1af729e7543e233daeff23499d6c867d8f78edcda8eb8bc9b1b3c81ef6c01a`，构建日志 `target/dock-nested-verified-build.log`，停靠过滤仍为42 passed。XML 此时已能改变分隔比例，但总分配为697而预期706.5，日志 `target/dock-nested-xml-verified.log`；带调用栈复验将失败定位到调整前后总尺寸比较，见 `target/dock-nested-xml-backtrace.log`。增加稳定帧测量的 `target/dock-nested-xml-runner/editor_app-0e90508672c3214b.exe`（SHA-256 `49c0265f906527720a770e0238348075ff1a5666272cbe3b3372a0c9d750ddc7`）仍以相同数值失败，日志 `target/dock-nested-xml-stable.log`，因此没有把问题归咎于测试采样。

接着通过 Base 公开的 `center_frame` 及现有停靠绘制钩子记录实际区域，中心列减去底部的真实绘制高度；同区投影使用实际分配，新增数据只含每表面至多四个区域的瞬时边界。该边界收紧本身仍未解决697／706.5的失败，见 `target/dock-nested-region-xml.log`，没有将其写成已确认根因。

最终根因是半尺寸90.5小于 Base100像素下限：原剩余窗口按90.5扣减，但 Base 实际给来源100，两边总量多出9.5。提交现在先把来源约束到可分配范围，再以同一尺寸扣减目标；预览仍遵守原尺寸／相等取半规则，最终极小尺寸服从 Base。新增快速回归 `dock_nested_half_below_minimum_does_not_overallocate_the_cell` 通过真实标题拖动验证90.5预览、100提交、两个子窗口合计600且底边不溢出，与43项停靠回归一同通过。

最终程序的实际 XML 回归为1 passed、0 failed、78.73秒，日志 `target/dock-nested-minimum-xml.log`。原分隔条、总尺寸、编辑刷新及恢复断言均保留并通过。

## 验证范围

未运行完整 UI 测试、全部 ignored 用例或八组合 DPI 矩阵。原生输入走生产窗口派发，没有操作系统 SendInput 或 IME 候选窗验证。当前极小空间继续服从 Base100像素约束；不能将发生下限裁剪时的比例当作普通宽裕布局的保证。本轮不提交、推送或修改用户安装，保留并行工作区已有变更。

最终 `git diff --check`、13份相关源码的行尾空白与5份相关文档的本地链接检查通过。

返回[原生 UI 目录](../README.md)。
