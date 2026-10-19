# 编辑区左边缘拖放修正

本记录保留首轮修正与当时证据。后续用户明确了上下层级：拖到编辑区左侧的窗口只占编辑器所在行，底部 Git 横跨两者。首轮将新列放入全高 Left dock，遗漏了底部跨度；当前行为及补充验收见[编辑器局部列与底部跨度](editor-row-drop-2026-10-19.md)。下文实现不再代表当前策略。

用户场景：资源管理器和大纲共用左栏时，把资源管理器标题拖到中间编辑区左边缘；出现新列预览后松手，资源管理器却仍回到原有组合，表现为放置失败。

## 复现与原因

`cargo test -p editor-app explorer_drop_on_editor_left_matches_preview --jobs 1 -- --test-threads=1` 通过真实宿主标题的 MouseDown / MouseMove / MouseUp，读取生产落点预览及提交后的实际内容宽度。修改前预览宽度为 500 个逻辑像素，提交后资源管理器仍为 280，日志 `target/dock-drop-before.log`。冻结旧测试程序后重复同一用例，0.17 秒再次出现同样失败，日志 `target/dock-drop-red-repeat.log`。

测试前的排序假设为：落点预览与实际提交策略不一致；移动时目标节点失效；后续会话同步覆盖成功的布局。单独取消编辑区左边缘的特殊改写后，新列确实出现，但打开右栏使其从 500 缩为 340，日志 `target/dock-drop-probe-center.log`。这证实第一项：原有改写为维持独立左栏宽度，将显式新列落点改成了旧列顶部，预览和提交不一致；简单移除改写又会违反已确认的左右宽度规则。

## 实现

显式放到编辑区左边缘时，使用 Base 的公开移动接口，将窗口放入独立 Left dock 最靠近编辑区的新列。新列使用预览的宽度，已有可见列保留当前实测宽度；注册插件时的默认左侧位置仍加入现有组合。新列拖回已有侧栏组合或移出左栏时，释放其独占列宽，避免拉宽其余窗口。

持久树、规范化编辑、大小偏好和保存恢复仍由原来的 DockArea 维护。本地仅读取当前帧的叶分配，水平分割合计宽度、垂直组合共享宽度，隐藏叶不占宽度。新列编辑使用一次性 DockLayout 给两侧完整尺寸，再交回原来的 DockArea，复用所有面板句柄，没有增加第二套布局模型。仅给新列宽度曾让 Base 在首次测量时重新分配为原列 380、新列 400；同时给两侧尺寸后，原列保持 280、新列与 500 的预览一致。右栏显示继续只消耗中间编辑区。

## 验证

新增三项鼠标回归覆盖大纲可见、大纲隐藏及移动后立即松手；逐项比较预览与实际新列宽度，并检查原列、开关右栏、写入会话后创建新编辑器恢复，以及拖回大纲上方后释放列宽。保存夹具初轮用了临时目录的原始路径，改为与真实会话一致的 Workspace 规范化根路径，并用 `try_save` 显式检查写入结果；未修改生产会话存储逻辑。

一次重链遇到测试 EXE 的 `LNK1104` 文件访问错误（`target/dock-drop-verified.log`）。按同一 `cargo test -p editor-app explorer_drop_on_editor_left --jobs 1 -- --test-threads=1` 入口重试后，3 passed、0 failed，运行 1.26 秒，日志 `target/dock-drop-verified-retry.log`。固定测试程序到 `target/dock-drop-final-runner/editor_app.exe`，SHA-256 `b9607ce03e6d956c41cb670c88b57aa6eae73755a49ccb7cb6834a9cd2835efa`。复制时也短暂遇到共享输出文件访问错误，复制成功后在独立程序中重新运行专项，避免后续共享构建改写正在验收的程序：

| 入口（冻结程序） | 结果 | 日志 |
| --- | --- | --- |
| `dock --test-threads=1` | 19 passed、0 failed、2 ignored | `target/dock-drop-dock-final.log` |
| `sidebar --test-threads=1` | 10 passed、0 failed、1 ignored | `target/dock-drop-sidebar-final.log` |
| `exclusive_tests --test-threads=1` | 3 passed、0 failed | `target/dock-drop-exclusive-final.log` |

工作区门禁：`cargo test --workspace --exclude editor-app --jobs 1` 为 78 组、295 passed、0 failed、309 ignored（`target/dock-drop-nonui.log`）；`cargo check --workspace --jobs 1` exit0（`target/dock-drop-workspace-check.log`）。中间一次 `cargo fmt --check` 通过，收尾重跑因其他并发改动 `ui/controls/aligned_editor/`、`aligned_editor.rs` 与 `aligned_pointer.rs` 的格式差异失败，详见 `target/dock-drop-format-final.log`；本任务未修改这些文件。本次涉及的 Rust 文件单独运行 `rustfmt --check` 通过，`git diff --check` 和新增文件空白检查通过。未执行的 ignored 测试不计为通过；真实 XML 包及 Windows 原生专项结果另记于下文。

Windows/D3D11 原生专项使用同一冻结程序、隔离 Git 测试包 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`（SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`），未修改用户安装。显式运行 `extensions::worker::native_acceptance::exclusive::git_native_exclusive_dock_replaces_and_restores_peers --exact --ignored --test-threads=1`，父测试 1 passed、168.72 秒，日志 `target/dock-drop-native-final.log`：

| 原生组合 | 新列预览宽度（逻辑像素） | 放置、开关右栏及拖回组合 | 退出与错误日志 |
| --- | --- | --- | --- |
| 深色中文 100% | 496.5 | 全部通过，旧列保持 280 | `shutdown_complete:true`，错误 0 |
| 浅色英文 150% | 500 | 全部通过，旧列保持 280 | `shutdown_complete:true`，错误 0 |

证据为 `target/dock-drop-native-final/{dark-zh-CN-100,light-en-150}/evidence.json` 的 `editor_left_drop_complete`；截图为对应目录的 `editor-edge-preview.png`、`editor-edge-placed.png`、`editor-edge-right-open.png` 和 `editor-edge-returned.png`。已核对深色打开右栏和浅色放置后的实际截图。使用真实 Windows HWND/D3D11 绘制与生产 `Window.dispatch_event` 客户区鼠标事件，未声称 OS SendInput、完整八组合、原生进程间重启或 IME 候选验收；磁盘会话恢复由上述三项 GPUI 回归覆盖。

真实 XML 0.2.2 固定包（账本 SHA-256 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`）的四方向专项首轮失败：大纲呈现失败状态，91.56 秒（`target/dock-drop-outline-final.log`）；单独复跑出现 `native fixture demand did not complete`，89.32 秒（`target/dock-drop-outline-retry.log`）。同一输入在此前冻结程序 `target/right-width-final-runner/editor_app.exe`（SHA-256 `40b04c3610cc4ca064f5aaf79fa02cee87ec919a94fd38620712eb1a71d68be9`）上为 1 passed、89.55 秒（`target/dock-drop-outline-baseline.log`）。随后在本轮同一个冻结程序启用 `RUST_BACKTRACE=1` 复跑，1 passed、86.84 秒，日志 `target/dock-drop-outline-trace.log`；四方向、实际分隔线调整及保存恢复均通过，没有修改源码、等待期限或断言。未得到失败调用栈，前两次加载／需求等待的间歇失败原因仍未确认，作为实际包验收的不稳定限制保留；不能把前两次结果改写为通过。

本次没有生成安装包，未提交、推送或修改议题；保留本任务以外的已有工作区改动。没有执行完整 `editor-app` UI 测试集。

返回[原生 UI 目录](../README.md)。
