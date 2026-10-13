# 并行项目窗口实施记录

日期：2026-10-13。状态：实现、自动验证、原生行为验收与双轴审查完成，待普通提交、推送与工单读回。

依据：[工单 #104](../tickets/project-windows/02-concurrent-project-windows.md)、[总规格 #102](../specs/project-windows.md)。前置 #103 已通过 GitHub 读回为 closed/completed；#104 open，评论为空。

固定起点 `3360306feac43492830cde3381048384587ade2d`，复用已附加的隔离工作区 `C:/Users/Tmiracle/.codex/worktrees/project-home/Editor`，新分支 `codex/concurrent-project-windows`。原主工作区仍在 main `ee59a569`，起始状态干净。

## 接缝与实施顺序

沿用已确认的应用级项目生命周期主接缝，以及公开 Manager/真实包、受控进程等现有依赖接口。没有增加测试专属业务接口。#105 的跨进程、安装器及右键入口不在本单执行范围。

1. 先使单个插件数据所有者能路由到多个持续运行的项目，保留每个项目的连接、请求、原生根和关闭回执。包缓存、数据锁、安装事务与应用声明仍只有一个权威所有者。
2. 宿主窗口使用共享运行时所有者，但独立收发项目消息和发布快照；关闭一个项目不销毁共享管理器。异步完成保留原始作用域与实例世代。
3. 将语言提供者、有效贡献和用户配置接到明确的项目或应用所有者；不是按窗口焦点切换一份全局快照。
4. 接入应用窗口登记、实际目录去重、新项目/首页路由及全窗口退出；复用 #103 的输入、保存和资源清理规则，先收集全部决定再统一收尾。

每步先红后绿，只因新增改动或实际失败扩大检查。最终稳定候选再执行完整仓库检查、双轴审查、双窗口原生验收、提交/推送与工单读回。

## 实现结果

- 每个原生窗口持有自己的 EditorApp、DocumentSession、语言提供者、有效贡献与插件消息通道。共享用户设置只有一个写入者，语法缓存使用项目实例身份；逻辑语言名称继续保持原契约。
- 应用持有窗口登记、实际目录身份租约、最近项目权威值和插件 RuntimePool。一个规范化插件存储只创建一个 Manager 与数据锁，多个项目的连接、进程、请求和实例始终活跃；投影路由只移动所有权，不复制可变文档，也不停放其它项目。
- 应用内重复目录激活既有窗口，信任待决窗口也保留目录预约。首页请求复用已有首页或新建首页，项目内打开不同目录新建原生窗口。项目关闭回到原窗口首页；窗口关闭仅释放其项目，最后窗口使用同一全局退出协调。
- 全窗退出先收集全部决定并重验原生输入和文档指纹，再清理各项目及应用所有者。任何收集期取消保留所有窗口；既有局部弹窗可被接管，局部收尾在途的完成或失败也能唤醒收集器。清理失败保留视图、错误与重试路径。
- 共享安装的准备、切换和持久化选择由一个 mutation lane 协调，两个项目的既有命令仍可用。仍沿用既有同包活跃实例的数据更新约束，没有取消现行权限确认或回滚保护；代表性变更验证的是新包安装成功和其升级迁移失败保留旧版本。
- 用户主题依据全局启用、协议兼容和不可变资源有效性发布。项目提供者错误不会删除另一窗口的共享主题。公开协议、能力版本与 SDK 契约没有变化，也没有插件 ID 专属宿主分支。

## 测试证据

- 首条运行时回归由冻结 `capability-example` 包进入公开冷需求、WASM 命令和原生执行接口。两个临时目录的 PowerShell 子程序写各自的受控活动输出，旧 `switch_workspace` 路径在选择 B 后停止 A；`concurrent-runtime-red.log` 确认失败于 A 的实际活动停止。
- 新增 `select_workspace` 路由保留项目资源所有权，`close_selected_workspace_checked` 只回收当前项目。相同回归确认 A/B 同时产生活动输出、两份私有数据独立、关闭 B 后其输出停止且 A 继续；1 passed，0 failed，0 ignored，65.42 秒。日志 `concurrent-runtime-green.log`。
- 外部目录移走的公开 Manager 回归先失败于旧路径不存在，修正为先恢复既有作用域后通过；不再依赖旧路径才能关闭。已有局部关闭弹窗接管与项目错误影响主题的回归也分别先红后绿。两轴发现与修正顺序见[审查记录](concurrent-project-windows-review.md)。

最终候选源码来自上述固定起点，复审索引树为 `b1bbaeea1f7106d2e5375e72bf4c798465a65a81`。后续文档更新不修改受测代码；先运行针对性回归，修正实际失败后集中进行最终完整检查。

| 命令/场景 | 实际结果 | 证据与覆盖 |
| --- | --- | --- |
| `cargo fmt --check`、`git diff --cached --check` | 通过 | 格式、补丁检查 |
| `cargo test --workspace --exclude editor-app -j 1` | 267 passed，0 failed，255 ignored | `concurrent-workspace-tests-final.log`；包含新作用域目录移动回归，ignored 未计为通过 |
| `cargo check --workspace -j 1` | 通过 | `concurrent-workspace-check-final.log` |
| `cargo test -p editor-app -j 1 terminal::tests:: -- --test-threads=1` | 9 passed，0 failed，1 ignored | `concurrent-terminal-final.log`；原生终端快捷键、并行任务、恢复与关闭。移除与终端 tab 冲突的窗口 Ctrl+Shift+W，窗口仍用原生 Alt+F4 或命令菜单关闭 |
| `cargo test -p editor-app -j 1 a_project_failure_preserves_the_shared_user_theme -- --test-threads=1` | 1 passed，0 failed | `concurrent-theme-failure-green.log`；真实公开 Package 安装、两项目目录、项目失败保留共享主题及全局禁用撤销 |
| 完整 editor-app 测试可执行文件 `--test-threads=1` | 529 passed，7 failed，230 ignored；145.39 秒 | `concurrent-ui-full-final.log`；新双窗打开/去重、全局取消、输入重问、清理失败与重试通过，既有资源树、语法、折叠、原生输入及终端回归通过 |
| editor-app 可执行文件 `concurrent_project --ignored --test-threads=1` | 3 passed，0 failed，0 ignored；167.11 秒 | `concurrent-actors-and-projects-final.log`；真实 A/B 格式化和用户设置、同包双 actor 私有状态/关闭/重开、安装成功与真实迁移失败保留旧版本，准备等待期间 A/B 命令持续可用 |
| editor-app 可执行文件 `project_home_close_drains_native_history --ignored --test-threads=1` | 2 passed，0 failed，0 ignored；23.27 秒 | `concurrent-close-queue-final.log`；沿用真实迟到 LSP、Undo/Redo FIFO 与末尾中文预编辑，不复制完整输入矩阵 |
| scoped_instances 可执行文件 `concurrent_workspaces_keep_both_native_programs_active_and_close_only_the_owner --ignored --test-threads=1` | 1 passed，0 failed，0 ignored；66.57 秒 | `concurrent-runtime-final.log`；真实 WASM、A/B PowerShell 活动输出持续增长，关闭 B 后其输出停止，A 仍增长且命令可用；B 受限重开拒绝执行，不借用 A 权限 |

直接运行已由对应 Cargo 构建的测试可执行文件，避免对相同源码反复链接。editor-app 路径为共享 target 的 `debug/deps/editor_app-bf8b72a9198c679b.exe`，运行时路径为 `debug/deps/scoped_instances-3f741c859973f9bd.exe`。完整 UI 运行包含取消解析后的 XML 原生粘贴 Undo/Redo 回归，资格键迁移后实际通过。

完整 UI 中的 7 个失败与 #103 已在干净 main `ee59a569` 复现并记录的基线完全相同，见[此前基线对照](project-home.md)。本单没有把这些失败或跳过项报告为通过：

- `app::plugins::status_tests::status_popover_preserves_language_loading_and_keyboard_dismissal`
- `app::plugins::tests::stale_rust_timeout_after_project_reenable`
- `app::plugins::tests::status_indicators_open_plugin_lists`
- `editor::definition_tests::ctrl_click_definition_on_declaration`
- `editor::definition_tests::ctrl_click_definition_releases_source_editor`
- `editor::definition_tests::middle_click_definition_stays_on_target`
- `extensions::worker::worker_tests::operation_errors_retain_plugin_ownership_through_worker_publication`

固定包由 `tests/support/packages.rs` 验证账本摘要后进入公开 Package 和 Manager：`capability-example 0.17.1`（SHA-256 `20f5d815b109f46ab54e22bf56ccd89497c83fd4b2619250294e1a02cad249d8`）与 `javascript 0.3.1`（`a09afb41de50211e5e0a866d2698dde2a6d524b54dc74149eda573f42eabe144`）。动态 A/B 语言配置与升级故障候选由冻结包重打包并通过公开检查；不是生产随附插件，不创建测试专用宿主接口。

## 原生验收与范围

`cargo build -p editor-app -j 1` 通过，日志 `concurrent-native-build-final.log`。将同一候选产物复制到隔离资料目录的 `native-concurrent/Nanobug-concurrent.exe`，SHA-256 为 `04e0e5af5a47a1a55b4a45d00442f17a1464974b4382d238182b7a76bdf49360`。使用该目录下独立 profile 和带中文、空格路径的“项目 A”“项目 B”，不修改日常用户配置或真实项目。验收由真实 Windows 桌面输入和原生窗口观察完成；以下是本单双窗证据，不复用 #103 的截图。

| 场景 | 实际可见结果 |
| --- | --- |
| P03：从 A 打开 B | A 保留未保存编辑，通过 Ctrl+Shift+O 的原生文件夹选择器选择 B 后出现第二个系统窗口。A/B 均可编辑自己的 notes.txt |
| P11、P24：焦点、输入与布局 | A 的 Ctrl+Z、Ctrl+Y 只撤销/恢复 A。B 的真实中文输入法显示拼音预编辑与候选栏，选择“中”后提交到 B；A 文本不变。隐藏 B 的资源管理器后，A 的资源管理器仍可见 |
| P12、P24：共享主题 | 在 A 用 Ctrl+Alt+T 切换到深色，B 同步更新主题，两个文档内容保留；浅色和深色均实际显示 |
| P01、P04：首页与项目去重 | Ctrl+Shift+N 新建一个首页，另一项目再次请求首页复用它，窗口总数保持 3。点击首页最近项目 A 激活原 A，未保存文本保留；关闭独立首页后两个项目继续存在 |
| P18：局部关闭接入应用退出 | A 已有“关闭项目”确认时，从 B 请求退出应用；在 A 取消后退出轮次结束，再次退出可正常开始新的确认流程 |
| P18：保存后仍可取消全局退出 | 全局退出中明确保存 A 后，B 再取消，两个项目窗口仍在，B 保留未保存编辑。磁盘核对 A 已保存、B 仍为原内容；没有撤回 A 的显式保存 |
| P13、P14：逐个关闭至应用退出 | A 关闭项目回到同一个原生窗口的首页；再关闭该首页，B 仍可见、可编辑且保留中文内容和独立布局。B 按 Ctrl+S 后未保存标志消失，磁盘核对两份文件正确；Alt+F4 关闭最后的 B，测试进程 42904 正常退出，窗口清单中不再存在验收应用，stderr 为 0 字节 |

最终磁盘内容分别为 `A original：A 中文🙂` 与 `B original：B 中文🙂中`。原生桌面使用空插件存储验证窗口、输入与关闭；真实 WASM、语言能力和原生程序的并行存活及清理由上表公开管理器/真实 GPUI 集成测试证明，没有将桌面两窗可见等同于插件并行验收。

#105 的跨进程请求、真实安装器及文件资源管理器右键尚未执行。本次未覆盖的完整缩放/滚动及单项目输入矩阵沿用 #103，不声称重复完成全部人工场景。

日志保留于本机隔离资料目录 `C:/Users/Tmiracle/.codex/visualizations/2026/10/12/01a12e28-69b5-7ea2-b14e-f155ef6bf763`，`CARGO_TARGET_DIR=C:/Projects/RustProjects/Editor/target`。本单没有安装或升级工具链。
