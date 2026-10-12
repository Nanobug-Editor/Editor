# 项目首页与单项目关闭实施验收

日期：2026-10-12。依据：[工单 #103](../tickets/project-windows/01-project-home.md)、[总规格 #102](../specs/project-windows.md)。审查记录见[双轴审查](project-home-review.md)。

范围为第一张工单：无工作区首页、项目选择与信任、最近项目、单项目编辑与安全关闭、重开恢复。#104 的并行项目窗口及 #105 的跨进程/目录右键/安装器尚未实施，不能把本记录称为整个 #102 已完成。

## 候选与工作区

- 按用户要求，先将原主工作区全部既有改动提交到 main：`ee59a5692a95e9c32f1cea9948497f4ebff393c9`；原工作区 `C:/Projects/RustProjects/Editor` 保持该提交且干净。
- 以该提交创建独立受管工作区 `C:/Users/Tmiracle/.codex/worktrees/project-home/Editor`，分支 `codex/project-home`。没有把工单实现写入原主工作区，也没有合并 main。
- 最终代码候选：`9b18bbcad2699f770d279b83d27b6a7c8138b6d4`，树 `68b236dd5ec021139f2fbcc311ec31ed7b7a03a5`。后续交付记录提交只修改文档。
- 每个独立修正按根因正常提交；未改写历史。每次将“此前 HEAD + index”通过 Git archive 导出到隔离目录验证，包含实际固定包及 include_str/include_bytes 输入，未借用后续未暂存实现。

实现提交依次为 `f378123`（首页闭环）、`e7e1c4`（窗口 profile 贯穿资源）、`d30991a`（可观察、可重试的资源清理）、`d82e97f`（输入排空与稳定文档判断）、`ecf90b6`（外部删除文档的批量保存保护）、`eea561f`（队列末尾 IME 提交）、`9b18bbc`（文件备份清理错误及重试）。`c0ac737` 记录既有工单发布映射。

## 最终行为

`ProjectWindow` 在选择项目之前不创建 `Workspace` 或 `EditorApp`，普通无参数 GUI 启动不再打开进程当前目录；原有显式文件/目录参数及独立工具命令保留。目录选择、信任决定及会话准备成功后，才在原窗口创建编辑器与项目资源。

最近项目按规范路径去重，保存本机完整路径及打开顺序，列表可滚动并移除记录。缺失目录、损坏记录、失败持久化不会创建项目或覆盖损坏数据。本机既有且匹配的信任记录继续有效；未知项目先选择信任、受限或取消，项目文件不能授予信任。受限模式仍可编辑。

关闭项目、窗口及单窗口 Quit 使用同一项目关闭流程：先排空原生输入 FIFO，提交 IME 并等待 DocumentSession revision 与原生文本指纹稳定，再收集保存/放弃/取消及停止程序的决定。决定后再次校验目标和文本，保存前检查所有实际保存候选的磁盘状态；外部修改和删除均不被批量关闭静默覆盖。文本仍由既有 DocumentSession/EditorState 管理，取消后原生 Undo/Redo 继续可用。

会话和终端持久化失败先阻止资源停止；后续清理等待文件、终端、插件所有者后汇总错误。失败保留原视图及文本，允许重试；插件快照、原生进程和文件备份清理不再假报成功，失败备份仍登记。成功后才撤销旧编辑器并返回首页或退出。重开恢复已保存标签、活动文件与布局，终端逻辑输出可恢复，上次程序保持停止；测量和恢复不会启动 Shell。

## 自动验证

Cargo 使用既有 Windows Rust 工具链，`CARGO_TARGET_DIR=C:/Projects/RustProjects/Editor/target`；没有升级依赖或安装工具。日志保存于本机隔离目录 `C:/Users/Tmiracle/.codex/visualizations/2026/10/12/01a12e28-69b5-7ea2-b14e-f155ef6bf763`，不随源码提交。

| 命令与实际输入 | 结果与证据 |
| --- | --- |
| `cargo fmt --check`，最终精确树 | 通过；`index-transfer-format.log` |
| `cargo check --workspace -j 1`，最终精确树 | 通过；`index-transfer-check.log` |
| `cargo test --workspace --exclude editor-app` | 266 passed、0 failed、254 ignored；`workspace-reviewed-tests.log`。运行后非 UI crates、依赖及固定包内容未变，后续仅修改 editor-app，复用该结果；ignored 不计通过 |
| `cargo test -p editor-app project_home -- --include-ignored --test-threads=1`，`eea561f` 的精确树 `bee37481c4a99ef5006c36126abf5be84e09af6c` | 17 passed、0 failed、0 ignored，27.59 秒；`index-queued-ime-green.log`。包含两项真实延迟语言服务/原生队列 ignored 用例的显式运行，后续传输清理改动另由全量 UI 回归覆盖 |
| `cargo test -p editor-app project_home_close_drains_native -- --include-ignored --test-threads=1`，最终精确树 | 2 passed、0 failed、0 ignored，25.13 秒；`queued-ime-delivery.log`。传输收尾依赖修改后只补跑两个真实队列用例，最终普通首页用例已由全量 UI 覆盖 |
| `cargo test -p editor-app explorer_transfer_ -- --test-threads=1`，最终精确树 | 29 passed、0 failed、0 ignored，9.42 秒；`index-transfer-green.log`。涵盖实际剪贴板复制、窗口关闭、冲突取消及文件历史；复用既有关闭回归增加 Windows 受管目录锁失败及解锁重试 |
| `cargo test -p editor-app shutdown_tests -- --include-ignored --test-threads=1` | 2 passed、0 failed、0 ignored，50.84 秒；`checked-worker-shutdown.log`。冷启动失败及真实冻结 capability-example WASM 的快照文件锁失败、同实例保留、解锁重试与新管理器恢复。该 Worker/Manager 代码此后未变 |
| `cargo test -p editor-app -j 1 -- --test-threads=1`，最终精确树 | 526 passed、7 failed、227 ignored，130.04 秒；`editor-app-delivery-full.log`。失败清单与原主分支完全一致，本单普通回归全部通过；没有将整套 UI 测试记为全绿 |
| `cargo build -p editor-app -j 1`，最终精确树 | 通过，1 分 11 秒；`native-delivery-build.log` |

应用级测试统一走生产首页、原生目录选择结果、信任选择、真实文档输入与关闭控件，不增加测试专属宿主 API。主矩阵覆盖首页无项目、取消/无效路径、信任取消/受限/复用信任、最近目录失效及移除、重开、保存/放弃/取消、保存失败、磁盘变化、外部删除、项目拥有的设置窗口收尾、终端停止恢复，以及中英文/深浅主题/键盘/焦点/滚动/缩放。场景对应 P01、P02、P05–P09、P13–P17、P19、P24 的本单部分。

测试驱动的红阶段保留实际失败：缺少普通 IME 提交时没有保存确认；连续 History 后判断过早会使第一次保存决定被瞬态 revision 拒绝；Undo 后队列中的 IME Mark 仍有原生 marked range；真实文件备份删除失败返回成功。修正后断言转绿。最后一个 IME 组合仅确认未结束预编辑，该次迟到 Change 恰好保留保存文本，不声称已复现数据丢失。

### 原主分支已有失败

对原主分支 `ee59a569` 独立运行相同全量 editor-app 命令：510 passed、7 failed、224 ignored，139.21 秒，见 `main-baseline-full.log`。最终候选核对以下同一组失败，没有将它们记为通过或为本单顺手修改：

- `app::plugins::status_tests::status_popover_preserves_language_loading_and_keyboard_dismissal`
- `app::plugins::tests::stale_rust_timeout_after_project_reenable`
- `app::plugins::tests::status_indicators_open_plugin_lists`
- `editor::definition_tests::ctrl_click_definition_on_declaration`
- `editor::definition_tests::ctrl_click_definition_releases_source_editor`
- `editor::definition_tests::middle_click_definition_stays_on_target`
- `extensions::worker::worker_tests::operation_errors_retain_plugin_ownership_through_worker_publication`

最终候选核对为同一组 7 个失败，未新增失败。语言夹具仍按旧即时服务索引访问，三个状态用例仍按旧加载状态期待，Worker 用例仍按旧英文错误文本期待；这是实际基线失败边界，不据此宣称全量测试全绿。未显式选择的 ignored 外部夹具测试仍未验证。

一次较早的并发链接遇到 Windows 1455（页面文件资源不足）；后续单 job 的非 UI workspace 全量测试已成功，不把环境失败记为通过，也未修改系统设置。构建保留既有未使用项及 Tree-sitter/Wasmtime 链接警告，没有执行无关 cargo fix。

## Windows 原生交互

通过 computer-use 操作专用副本 `native-fixture/editor-app.exe`，使用隔离 `ME_EDITOR_PROFILE_HOME` 和 `ME_EDITOR_PLUGIN_HOME`，仅操作临时“演示项目”及最近记录夹具。没有操作用户真实编辑器窗口、项目或信任记录。自动矩阵已充分覆盖的逻辑不逐项重复人工重放。

- 无参数启动为首页；通过真实 Windows 文件夹选择器打开中文路径的普通文件夹，同一 HWND 进入项目。取消选择保留首页；实际最近列表包含 20 条，滚动可见后续记录。
- 真实编辑中文与 emoji；关闭时取消保留文本，保存并关闭落盘后回到同一窗口，放弃不写入磁盘。再开恢复标签、活动文件与布局。浅色/深色外观可辨识，首页空项目边界正确。
- 明确文件参数打开 `演示项目/demo.layout` 并定位目标。实际 Layout Example WASM 的布局 UI 可见；固定包 `layout-example-0.3.1.zip` SHA256 为 `b08dfd00b6693c5b9e118285b34c8748966347f0c01551fed9e8bf37895ec409`，通过公开 Manager 准备到隔离运行目录，没有新增插件白名单或测试业务入口。
- 恢复终端历史时没有 PowerShell 程序。点击真实新标签入口才启动 PowerShell；取消关闭保留 PID 41204，确认停止并关闭后该进程及 PTY conhost 27548 消失。只有验收实例自己的普通控制台保留至最后退出，没有向终端输入命令。
- 用实际 WASM 的私有 `state.json` 文件句柄拒绝替换：关闭显示具体拒绝访问错误并保留编辑器；错误后继续输入“收尾失败后仍可编辑 中文🙂”，解除锁后保存并关闭成功，磁盘内容核对正确。后续重开再次得到真实插件布局。发现旧错误横幅留在首页后已修正，并由真实会话保存失败的 GPUI 重试测试断言横幅消失。
- 最后首页 Alt+F4 退出后，验收 PID 50752 已消失。

上述较早原生副本 SHA256：`C7A1C8F9C1A242CC88378AD763EBA65392FB416967A09F843A409A5459CAF08B`。它验证主要交互及实际 WASM/终端生命周期，不冒充上述全部观察来自最终二进制。

最终精确代码树构建的副本 SHA256：`B2D8E68B87B403D2A5F17CED2FAEAD683FCECA47CBBBE42D11D3AF6746BC82A1`，PID 35152。仅补验后续修正影响的代表性边界：显式文件参数打开真实 Layout WASM → 持有私有快照锁 → 关闭 → 拒绝访问错误可见且编辑器保留 → 解锁重试 → 同一 HWND 6950082 返回首页且旧错误横幅消失 → Alt+F4 最后窗口退出，PID 消失；`app-errors-delivery.log` 为空。没有重复全部原生组合，也没有把较早副本的观察改记为最终副本的新观察。

曾在旧验收实例关闭设置窗口后读取 UIA 时，遇到上游 `accesskit_windows 0.34` 的无效 HWND（HRESULT `0x80070578`）并终止该实例；该尝试不是通过证据。项目拥有的设置窗口清理已由 GPUI 原生窗口回归通过；本单未升级或 fork 上游以修复平台自动化问题。

## 审查与交付

前两轮 Standards 和 Spec 发现及修正边界完整保存于双轴审查记录。最终代码固定提交 `9b18bbc` 独立复审：Standards 0 项、Spec 0 项，之前发现均已解决。原生补验完成，本单行为验收满足。

源码及验收记录推送到 `origin/codex/project-home`，关闭前核对远程与本地提交同为 `8ec57afd9d32f41b443e49287b592dbabf10fb49`。GitHub 连接器更新返回 403（Resource not accessible by integration），按既有跟踪器约定使用仓库已有 Git 认证及 REST API，只更新本单；凭据未写入文件或日志。#103 于 `2026-10-12T15:09:41Z` 关闭，读回为 **closed/completed**，11 项验收勾选及完整正文一致。#102、#104、#105 均读回为 open。后续仅提交这份关闭读回记录，不改变已经验证的代码。
