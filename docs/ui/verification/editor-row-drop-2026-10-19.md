# 编辑器局部列与底部跨度

后续[窗口实时拖放与通用停靠模块](live-dock-placement-2026-10-19.md)扩展了本阶段规则：编辑区边缘移动原共享组、保留同方向厚度，拖动中实时重排，并移除蓝色指示遮罩。下文保留本阶段当时的实现与验证证据。

用户确认的结构：原本全高的资源管理器在左，右侧上方是编辑器、下方是 Git；将资源管理器拖到编辑器左侧后，上方成为“资源管理器｜编辑器”，Git 横跨两者的完整底部。右侧窗口打开仍保持资源管理器宽度，仅缩小编辑器。

## 原因与修正

首轮实现把编辑器左侧落点提升到全高 Left dock，虽然新列宽度与预览一致，底部仍被左栏截断。真实标题鼠标回归可重复失败：资源管理器底边为 787，编辑器底边为 527，底部面板从 x=500 开始，未覆盖资源管理器。首次与冻结旧测试程序重复的日志分别为 `target/editor-row-bottom-before.log`、`target/editor-row-bottom-red-repeat.log`。

三个待排查原因依次为落点进入错误的父区域、底部保留旧宽度、后续同步覆盖拖放。单独去掉全高左栏拦截后，行与底部跨度正确，但打开右栏使资源管理器从 640 缩为 480，见 `target/editor-row-bottom-center-probe.log`。因此保留 Base 的 Center 局部分割，同时在根绘制前将工具列当前像素宽度交回同一 DockArea，宽度变化由编辑器分支吸收。

本地 `dock/columns.rs` 只组合一次性 Base DockLayout，复用注册面板、活动标签和原有垂直分割偏好，没有第二棵持久树。拖放使用目标所在行的高度，移出原生左栏时释放空列占宽。列间拖动仍交由 Base，绘制前处理不干预正在进行的拖动；极窄窗口为编辑器保留 Base 的最小宽度并约束工具列。

保存恢复首次绘制之前采用 Base 已保存的水平大小偏好，防止临时初始窗口大小永久缩放用户列宽。会话布局版本提升至 2：旧版 0/1 的侧栏迁移仍执行一次，之后保持显式拖出的 Center 局部列；未知的更高版本不降级。旧布局测试显式声明版本 1，保留完整树及尺寸断言。

## 验证

专项覆盖完整行列几何、预览宽度、手动拖动分隔条、开关右栏、调整窗口大小、保存后以另一窗口大小重开，以及拖回原侧栏释放列宽。通用面板测试使用原生 Problems 作为底部窗口；Windows 专项另用真实 Git 包检查相同关系。

`cargo test -p editor-app dock --jobs 1 -- --test-threads=1` 为 20 passed、0 failed、3 ignored，见 `target/editor-row-bottom-dock-final.log`。较广的 sidebar 过滤发现一个旧测试通过新默认会话构造旧树，却未指定旧版标记；补充显式版本 1 后同入口通过。未放宽树、尺寸或显隐断言，生产行为未因此改变。

为避免其他构建改写验收中的共享 EXE，使用两个独立副本。原生及 XML 验收程序为 `target/editor-row-bottom-final-runner/editor_app.exe`，SHA-256 `4dc1f44e3ac9d517cc5261c53a7f417534b1575977c6999debed213ebeb44af4`；补齐上述旧测试版本声明后的程序为 `target/editor-row-bottom-verified-runner/editor_app.exe`，SHA-256 `d2d15021ac1ae39f5cd7e034bcf9b3221cb67cbc6256d87b0d2710b95f7e1c89`。本任务在两次冻结之间只调整了旧测试夹具和一处说明注释。

| 入口 | 结果 | 日志 |
| --- | --- | --- |
| 最终独立程序 `dock --test-threads=1` | 20 passed、0 failed、3 ignored | `target/editor-row-bottom-dock-verified-frozen.log` |
| `cargo test -p editor-app sidebar --jobs 1 -- --test-threads=1` | 10 passed、0 failed、1 ignored | `target/editor-row-bottom-sidebar-verified.log` |
| 最终独立程序 `app::session::tests --test-threads=1` | 8 passed、0 failed | `target/editor-row-bottom-session-final.log` |
| 最终独立程序 `exclusive_tests --test-threads=1` | 3 passed、0 failed | `target/editor-row-bottom-exclusive-verified-frozen.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78 组、304 passed、0 failed、311 ignored | `target/editor-row-bottom-nonui.log` |
| `cargo check --workspace --jobs 1` | exit 0 | `target/editor-row-bottom-workspace-check.log` |
| `cargo fmt --check` | exit 0 | `target/editor-row-bottom-format-final.log` |

真实 XML ZIP 沿用固定归档账本，显式执行 `tests::outline::docking::native_outline_four_directions_resize_and_workspace_restore --exact --ignored --test-threads=1`，1 passed、86.34 秒，日志 `target/editor-row-bottom-outline-final.log`。该用例包含实际编辑输入、四方向拖动、缩放与重开；没有将未执行的 ignored 测试计为通过。

Windows/D3D11 专项使用隔离 Git 包 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`，未改动用户安装。设置 `NANOBUG_GIT_NATIVE_EXCLUSIVE=1`、`NANOBUG_GIT_CANDIDATE` 和独立输出目录后，显式执行 `extensions::worker::native_acceptance::exclusive::git_native_exclusive_dock_replaces_and_restores_peers --exact --ignored --test-threads=1`，父测试 1 passed、141.86 秒，日志 `target/editor-row-bottom-native-final.log`。

| 原生组合 | 新列预览／实测宽度（逻辑像素） | Git 的实际底部范围 | 完整流程 |
| --- | --- | --- | --- |
| 深色中文 100% | 496.5／496 | x=0，宽 1273；上方两列同高 569 | 放置、开关右栏、拖回、独占替换与恢复、退出均通过 |
| 浅色英文 150% | 500／500 | x=0，宽 1280；上方两列同高 573.33 | 放置、开关右栏、拖回、独占替换与恢复、退出均通过 |

证据位于 `target/editor-row-bottom-native-final/{dark-zh-CN-100,light-en-150}/evidence.json`。两组 `observations.editor_left_drop_complete` 的 `bottom_spans_both_upper_columns`、`explorer_stays_in_editor_row`、右栏及拖回检查均为 true；`shutdown_complete:true`、`observations.unexpected_error_logs:[]`。已逐图核对两组 `editor-edge-placed.png` 与 `editor-edge-right-open.png`：资源管理器在上方，Git 横跨完整可用底部。

输入通过生产 `Window.dispatch_event` 进入真实 WindowsPlatform，原生截图和绘制几何均检查，不声称操作系统 SendInput 或 IME 候选验收。本次未运行完整 UI 测试集、全部 ignored 用例或八组合原生矩阵；未生成安装包。`git diff --check`、新增文件空白和文档本地链接检查通过。

返回[原生 UI 目录](../README.md)。
