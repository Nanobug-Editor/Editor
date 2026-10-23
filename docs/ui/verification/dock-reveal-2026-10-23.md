# 工具窗口显隐统一使用通用布局

## 行为与边界

用户反馈终端按钮会在已有 Git 区域之外追加底栏，大纲按钮会在资源管理器之外追加外侧列。显隐操作应遵守既有四侧布局：左右一级组合上下排列，顶部／底部一级组合左右排列；独占窗口替换所在区域，普通窗口恢复原组合。

本轮保持 Base `DockArea` 为唯一布局树。窗口内容、终端进程和插件资源保留原实体；显隐入口根据实际位置恢复窗口，不固定打开默认 Bottom，也不删除用户拖动产生的有效嵌套结构。

- `editor-ui::dock::reveal_panel` 将孤立旧位置的叶加入当前同侧组合；已有普通组合仍留在原处。
- `panel_visible` 统一判断原始意图、独占遮蔽和实际区域是否打开，供窗口按钮与切换入口使用。
- 终端、运行输出、资源管理器、大纲、消息和问题面板使用公共入口；插件面板继续共用相同机制。
- 独占替代项不分走普通组合的权重。首次注册和恢复都通过 Base 公开接口处理；编辑区祖先节点分配整条区域，不参与区域内替代项的权重调整。
- 新普通叶优先分割普通兄弟的分配，避免把隐藏或当前显示的独占零占位当成可分配空间。
- 已关闭的旧区域不因其中其他窗口的原始显示意图而占用同侧位置；仅将请求窗口迁入当前区域，其余窗口的实体、资源和关闭状态保留。
- `LocalDock` 为隐藏槽暂存尺寸偏好，以轴和稳定窗口身份索引。隐藏期间向 Base 投影零空间，显示时恢复；保存时回填原 `DockAreaState` 尺寸字段，不另存布局树、不改变会话格式。可见分隔条与拖放仍由 Base 管理。

未修改插件公开协议、安装包、用户权限或依赖版本。并行任务的 Git 提交图列宽验收代码和插件文档不属于本轮改动。

## 复现与验证

修复前执行 `cargo test -p editor-app terminal_joins_existing_center_bottom_region --jobs 1 -- --test-threads=1`，结果为 0 passed、1 failed。终端位于新底栏 `y=615`，既有普通面板位于 `y=454.5`，断言捕获了截图中的重复底部区域，运行部分耗时 0.25 秒。日志：`C:/Users/Tmiracle/.codex/terminal-layout-red-20261022.log`。

回归覆盖真实窗口按钮输入、区域实际边界、隐藏再显示、终端拖到左右后的显隐、独占与普通组合互换，以及不等宽组合的会话保存和新实体恢复。

以下是最终源码的自动检查；原生验收见后文。app 后续过滤复用 Cargo 构建出的同一测试程序 `target/debug/deps/editor_app-420de45671c333eb.exe`，均使用 `--test-threads=1`；过滤间有交集，表内数量不相加作为唯一测试总数。

| 检查 | 实际结果 |
| --- | --- |
| `extensions::exclusive_tests`（Cargo 构建后直接运行同一程序） | 17 passed、0 failed；包括 700/300 分栏保存、新实体恢复、独占显示时首次注册终端、连续显隐、隐藏期间窗口尺寸变化、旧布局连续帧稳定及 230/310 隐藏尺寸恢复 |
| 测试程序 `extensions::dock_tests` | 47 passed、0 failed；包含大纲加入已拖动侧栏及磁盘恢复 |
| 测试程序 `host_messages` | 11 passed、0 failed |
| 测试程序 `tests::problems` | 6 passed、0 failed |
| 测试程序 `run::run_ui_tests` | 19 passed、0 failed |
| 测试程序 `terminal::` | 46 passed、0 failed、2 ignored；保留原有跳过条件 |
| 测试程序 `app::session` | 8 passed、0 failed；包括编辑器和延迟插件启动的布局恢复 |
| `cargo test --workspace --exclude editor-app --jobs 1` | 519 passed、0 failed、382 ignored；含四侧独占区域边界保护及关闭区域重新显示的两项回归 |
| `cargo check --workspace --jobs 1` | exit 0；保留既有编译警告 |
| `cargo fmt --check`、`git diff --check` | 通过；Git 保留行尾转换提示 |

本轮没有将 ignored 用例记为通过。日志保存在 `C:/Users/Tmiracle/.codex/terminal-layout-*-final7-20261023.log`；核心回归为 `terminal-layout-extensions-exclusive_tests-final7-20261023.log`，完整非 app 测试为 `terminal-layout-workspace-tests-final7-20261023.log`，编译检查为 `terminal-layout-workspace-check-final7-20261023.log`。运行界面 19 项使用 `terminal-layout-run-run_ui_tests-final6-20261023.log`；随后变更仅涉及隐藏尺寸序列化，没有改变运行入口。临时诊断输出已移除。

## 恢复布局中的尺寸回归

首轮 Release 构建通过（4 分 52 秒）后，真实窗口确认终端与 Git 已在同一底部区域左右排列，大纲与资源管理器在同一侧栏上下排列。不过连续隐藏／重开终端时，宽度从 720 变为 480、再变为 360，原生验收因此尚未通过。

进一步复现需要既有 Center 侧栏。`columns::prepare` 把当前可见状态与上一帧叶子边界混合：隐藏时 Git 临时扩展的宽度加上刚恢复终端的旧宽度，触发不必要的重排；重排又把临时绘制尺寸写成了新比例。隐藏期间调整窗口还会把隐藏叶权重写成零。

新增 `terminal::repeated_toggle` 三项测试，先按 700/300 设置普通分栏，再通过真实 footer 连续切换三次。修复前为 1 passed、2 failed：Center 终端宽度依次为 700、412、292、226；隐藏期间调整 viewport 后恢复仅 100。原生 Bottom 为通过的对照。日志为 `C:/Users/Tmiracle/.codex/terminal-layout-repeated-toggle-red-round2-20261023.log`。

另补唯一 Center 侧栏隐藏期间放大窗口的回归，修复前侧栏由 280px 恢复成 323.5px。日志为 `C:/Users/Tmiracle/.codex/terminal-layout-sidebar-hidden-resize-red-20261023.log`。

第三轮修复同时检查实际 Center 区域宽度和 Editor 祖先路径的宽度，避免底部临时边界干扰侧栏重排。固定侧栏优先使用 Base 保存的尺寸；不含 Editor 的工具组合保留原始权重，隐藏叶不会被重排写成零。以上新增回归和表内检查在该快照通过。

第三轮 Release 构建通过（4 分 50 秒），但真实旧布局包含两层仍有 100px 权重的隐藏问题／消息槽。Base 0.8.0 的尺寸归一化把隐藏槽也计入总和，使 437px 侧栏被缩放；下一帧列宽校正又重建整树，形成持续刷新循环。原生验收未通过，不能把上述自动检查当作功能完成。新增回归会直接检查连续帧的可见边界、根节点身份及 `LayoutChanged` 事件数量，避免只采偶数帧而漏掉两阶段振荡。

旧布局回归在未修复快照中于 2.17 秒失败：无用户布局编辑却超过 64 次 `LayoutChanged`。事件上限将无限刷新变为可终止的失败信号。日志为 `C:/Users/Tmiracle/.codex/terminal-layout-hidden-peers-loop-bounded-red-20261023.log`。另新增 230px／310px 隐藏窗口磁盘保存、新实体恢复测试，Editor 右侧始终有可见工具列，覆盖 Editor 并非最后一个弹性槽的情况。

最终采用隐藏尺寸投影，而不是按绘制差异反复重建树。保存时保留原有约束，包括 Base 0.8.0 用零表示的 `None` 自动分配槽；否则自动槽会错误保存为隐藏同伴时撑满的尺寸。严格恢复测试先捕获可见右侧窗口高度 563px 变成 607px 的失败，修正序列化后五个窗口的完整边界一致。拖动期间显式显隐及时恢复尺寸，并取消已失效的旧拖动快照，避免松手才突然重排。

## 原生验收

2026-10-23 09:58，最终源码执行 `cargo build -p editor-app --release --jobs 1` 成功，耗时 4 分 52 秒。日志为 `C:/Users/Tmiracle/.codex/terminal-layout-release-final7-20261023.log`；保留既有未使用代码和 Wasmtime/Tree-sitter 链接警告。实际启动文件为 `C:/Projects/RustProjects/Editor/target/release/editor-app.exe`，SHA-256 为 `A15680FC74376A8FF3E3F0DB345D2C04655B669A79EA32C43A0BC31A4A65E1CC`。

使用真实用户会话和鼠标点击窗口按钮完成以下验证：

- 终端与 Git 在同一底部区域左右排列，两者宽度均为 708.5px；隐藏终端后 Git 占用释放空间，重新打开恢复原分隔位置。连续操作两轮，未增加底栏或逐次缩小终端。
- 大纲位于资源管理器与提交面板之间，三者共用 460px 侧栏并上下排列。连续隐藏、显示两轮后，恢复原有位置与尺寸，未增加外侧列。
- 操作期间既有文档标签和终端内容保留。空闲观察期间布局位置稳定，会话文件修改时间保持在 10:00:37，未继续重复保存布局；这项观察仅验证布局稳定性，不作为整体 CPU 性能结论。
- 正常关闭窗口后读回会话：外层尺寸为 `[460, 1417, 100]`，编辑区与底部区域为 `[557, 400]`，底部左右分栏为 `[708.5, 708.5]`，终端与大纲显示状态均保留。10:02:58 重新启动同一 Release，实际窗口恢复相同侧栏、底部区域和分隔位置；验收结束时保持编辑器开启。

最终原生验收前，为恢复先前失败快照造成的尺寸漂移，先正常关闭编辑器并备份最新会话至 `C:/Users/Tmiracle/.codex/workspace-backups/dock-layout-20261023/before-hidden-size-repair.json`，再仅修正七个尺寸数值。保留最新底部高度 400px、文档标签、显示状态及其他设置，并验证除这七个值外与备份语义一致；没有用旧会话整体覆盖。此前原始会话备份仍保存在同目录的 `e8ab1cbf4668bdab.json`。

本轮窗口显隐与布局恢复验收已完成。原生启动仍可出现语言分析诊断及插件警告，不将本轮布局验证扩大为全部插件功能或全部 UI 的通过结论。运行中的 Rust 工具还向 `Cargo.lock` 写入 `plugin-protocol 0.2.0` 的 `patch.unused` 元数据；该变动保留在工作区，未归入布局修复或依赖升级。

此前工作区整合、远程推送与已安装插件更新已在[整合记录](../../project/verification/main-integration-2026-10-22.md)完成。本轮提交仅包含布局修复、配套回归和本验收记录；其他任务的 Git 提交图验收、插件文档及 `Cargo.lock` 元数据保留在工作区。

## 提交前隔离检查

按用户的独立提交要求，将 HEAD 与本轮暂存区导出至 `C:/Users/Tmiracle/.codex/workspace-backups/layout-commit-20261023-101013/candidate-utf8/`。初始候选树为 `272077bc21e25995271d70674d25cd109159893e`；最终提交前仅补充本节，Rust 内容保持一致。候选的 2,202 个版本化文件与暂存内容核对一致，允许 Git 的行尾转换；六个范围外文件的未提交改动未进入候选。`cargo metadata --no-deps --format-version 1` 确认 workspace 与 Cargo 输出目录均在候选自身目录。

候选执行 `cargo fmt --check`、`cargo test -p editor-ui --jobs 1` 和 `cargo check --workspace --jobs 1` 均通过；控件测试为 112 passed、0 failed、11 ignored。通过 Cargo 构建该候选的宿主测试程序后，单线程执行独占窗口 17 项、停靠 47 项、消息 11 项、问题 6 项、终端 46 项、会话 8 项、运行界面 19 项，均为 0 failed；终端另有 2 ignored，过滤间交集不累加。

本次隔离检查重跑受影响的控件与宿主测试；完整非 app workspace 的 519 项结果仍以前文 final7 日志为准。候选检查日志、元数据、提交文件清单和范围外文件的操作前哈希保存在 `C:/Users/Tmiracle/.codex/workspace-backups/layout-commit-20261023-101013/`。

返回[原生 UI 目录](../README.md)。
