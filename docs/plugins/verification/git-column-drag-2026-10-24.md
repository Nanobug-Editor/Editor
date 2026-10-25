# Git 历史列边界光标与拖动响应

2026-10-24。本轮沿用透明分隔线，修复横向调整光标缺失和拖动依赖插件回包的问题。

## 基线与实现

本轮 Hindsight 查询 `status=ok`，相关历史 `memory_id=ded044dc-8579-42ab-83a0-428458624e77`、`document_id=conversation:01a15bdd-0356-7141-805f-12ac00606d33:turn:01a167e8-3f4a-7a02-8ea8-27783466942b`。历史仅作为已有拖列能力的依据，作者对齐等旧说明按当前确认需求和代码执行。

实际 0.1.40 包原生基线：`git_native_history_scroll_probe` 带 `NANOBUG_GIT_NATIVE_COLUMN_DRAG=1` 复现了横向调整光标缺失。驱动三条边界从按下、等待帧、移动到观察绘图变化分别为 152.91、103.84、98.72ms。这包含测试驱动等待及并行构建竞争，不等同于单次指针延迟或真实帧率，不能据此推算提速百分比。证据根 `target/git-column-drag-2026-10-24/`，见 `red-native.log` 和 `native-red/dark-zh-CN-100/evidence.json` 的 `column_drag_probe`。

旧路径把每个 Pointer Move 交给 WASM，更新列宽后重新生成场景；宿主没有对应的原生命中光标。新通用 [ui.canvas 1.6 列宽能力](../specs/canvas-native-columns.md) 在原生帧中更新文本区域，松手后发送一次最终宽度；静态拓扑图复用原绘图。插件仍拥有数据和列宽保存逻辑。调整不改变提交选择、滚动位置或兄弟提交面板，三条边界保持透明。

## 自动检查

- `cargo test -p editor-ui adjacent_resize_is_immediate_reversible_and_waits_for_ack`：1 项通过；覆盖相邻列、边界反向、旧回包、确认与身份替换。
- `cargo test -p plugin-protocol native_columns_bound_events_and_advertise_their_capability`：1 项通过；验证能力版本、绑定、最小宽度、总宽度与过期身份。
- `cargo test -p editor-app canvas_columns_paint_every_move_without_guest_roundtrips`：1 项通过。真实 CanvasView 的 sink 故意不返回绘图；12 次移动后立即绘制并检查文本矩形，期间没有 Pointer/ColumnsResized 发给访客，释放只发送一个最终值。旧绘图回包不回滚预览。复用最终测试程序再次运行仍通过，耗时 0.01s，见 `native-unit-handles.log`；这是原生更新不依赖插件的证据，不能作为显示器刷新率测量。
- 宿主 `--plugin-cargo Cargo.toml test --lib`：186 项通过，见 `guest-tests.log`。
- `cargo test --workspace --exclude editor-app`：554 项通过、402 项 ignored，见 `workspace-tests.log`。被忽略项目不计入通过。
- `cargo check --workspace --jobs 2`：最终实现通过，见 `workspace-handles-check.log`。普通悬停由局部 GPUI 命中区域处理，仅已捕获的拖动使用窗口光标覆盖。
- 最终宿主 `--plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml check --lib --offline`：通过，见 `final-sdk-check.log`；确认实际交付 Release 导出的 SDK 包含新模块。
- 完整宿主 `cargo fmt --check`、独立 Git 源码的 `cargo fmt --check`、`git diff --check` 最终均通过，见 `fmt-delivery.log`、`guest-fmt-delivery.log`、`diff-check-delivery.log`。18:02 的共享快照曾分别遇到其他会话正在编辑的 `tests/preview_tabs/optical_alignment.rs` 与 `trees/incremental.rs` 格式差异，保留早期日志；未改写这些范围外文件。

## 原生验收与制品

验收使用独立 ZIP、临时仓库和私有用户目录，不访问用户 Git 配置或安装。初次一帧后立即读取系统光标，以及随后只合成 GPUI 移动的尝试均未通过，保留 `native-hover/` 和 `native-cursor/` 证据；后一次原生状态确认 `column_hover=true`、系统光标仍为旧箭头。真实 OS 光标复查要求 `WindowFromPoint` 命中自建 HWND，只临时抬高该测试窗口，按帧交替移动一个物理像素以接收 Windows 的 WM_SETCURSOR；父进程结束时恢复原鼠标位置，清理时销毁测试窗口。

最终测试程序来自本 checkout 的 Cargo debug 产物，观测编译开始于 18:19:53，产物生成于 18:21:44，晚于本轮最终源码修改。为避免共享构建锁下重复排队，取消了尚未开始编译的同目标 `--no-run` 命令，冻结当前产物再运行。依赖路径、相关源码及程序 SHA256 见 `native-final-provenance.json`；验收后核对本轮源码哈希未变。

`git_native_history_scroll_probe --exact --ignored --nocapture --test-threads=1` 使用下列环境进入实际 ZIP 的短历史流程：

```powershell
$env:NANOBUG_GIT_CANDIDATE = 'C:/Projects/RustProjects/Editor/target/git-column-drag-2026-10-24/final/git-0.1.42.zip'
$env:NANOBUG_GIT_NATIVE_OUTPUT = 'C:/Projects/RustProjects/Editor/target/git-column-drag-2026-10-24/native-final-handles'
$env:NANOBUG_GIT_NATIVE_HISTORY_ONLY = '1'
$env:NANOBUG_GIT_NATIVE_SCROLL_ONLY = '1'
$env:NANOBUG_GIT_NATIVE_COLUMN_DRAG = '1'
$env:RUST_MIN_STACK = '8388608'
& 'C:/Projects/RustProjects/Editor/target/git-column-drag-2026-10-24/native-final-handles.exe' extensions::worker::native_acceptance::git_native_history_scroll_probe --exact --ignored --nocapture --test-threads=1
```

| 本轮列宽验收 | 深色、中文、100% | 浅色、英文、150% |
| --- | --- | --- |
| 自建 HWND 命中、系统横向调整光标 | 通过 | 通过 |
| 三条透明边界均由原生手势持有并调整 | 通过 | 通过 |
| 提交选择、滚动位置、提交面板 revision 保持 | 通过 | 通过 |
| 引用靠右、粗体作者和时间靠左 | 原生字形边界误差 0px | 原生字形边界误差 0px |

上述结果分别见 `native-final-handles/{dark-zh-CN-100,light-en-150}/evidence.json` 的 `column_cursor_probe`、`history_columns`、`history_column_alignment` 与 `column_drag_probe`；对应 D3D 截图 `history-columns.png` 已检查。此流程还包含范围更大的滚轮、滚动条和关闭检查，**完整最终探针未全绿**：

- `native-handles.log`：深色完整通过；浅色完成全部列宽和滚轮断言后，在滚动条收起时未观察到中间宽度，整体退出 101。该实现本轮未修改，失败原因尚未单独确认，不计为通过。
- 不改源码复跑一次，见 `native-repeat.log`、`native-final-repeat/dark-zh-CN-100/native.log`：深色交互场景全部通过（含滚动条收起），随后关闭窗口时 `accesskit_windows 0.34.0` 后台线程因 `HRESULT(0x80070578)` 无效窗口句柄 panic，子进程异常退出；浅色未运行。该重复运行也不计为完整通过。
- 早期含窗口级悬停覆盖的变体在 `owned-native.log` 深浅两个完整流程均通过；最终改为局部悬停命中后，以前两条最终运行记录为准，不能用旧变体替代最终完整验收。

本轮交付文件位于 `target/git-column-drag-2026-10-24/final/`，须配套更新以使用 `ui.canvas 1.6`。`artifacts.json` 保存构建时间、大小及身份：

| 制品 | SHA256 |
| --- | --- |
| `Nanobug.exe`，Release 构建通过 | `b6089d48051fa11b9aa08009945f9ce824fa8f5a0c99844921947d287c8bbf65` |
| `git-0.1.42.zip`，验证的独立插件 | `e81212f379f7a2657e29509a3d4eeda454843c5047b8d8f4bdc5517a75f0eab7` |

Release 命令为 `cargo rustc -p editor-app --release --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/git-column-drag-2026-10-24/final/Nanobug.exe`，日志 `release-final-handles.log`。没有得到稳定的显示器 FPS 测量；可确认的是每次移动不再等待 WASM 或重新发布绘图，性能数字不使用含多帧等待的探针时间代替。

现有工作区和独立 Git 源码有其他会话改动，保留并以交付制品身份区分；尚未提交、推送、发布或改动用户安装。

## 同日复查：修复包未安装与实际更新

用户反馈拖动仍然卡顿后，先核对运行来源，而不是继续修改滚动算法。实际进程是 IDE 的 `cargo run --release` 启动的 `target/release/editor-app.exe`（文件时间 19:44:38）；但默认配置的注册表和正在运行的插件管理窗口均显示 **Git 0.1.38**，其 `ui.canvas` 要求仍是 `>=1.5, <2`。重新构建宿主不会自动替换已安装的独立插件，上一阶段的 0.1.42 ZIP 尚未进入这个窗口。这足以确认用户当时未使用新增的原生列宽路径，不能把旧版体验当成新路径的性能结论。

用户明确同意“更新并继续验证”后，通过运行中应用的“从本机安装 ZIP”正常更新为上述 0.1.42 制品。安装前比较包权限、旧清单权限与已授予权限，差集均为空；没有卸载、清空私有数据或手写注册表。安装完成后，窗口显示 Git 0.1.42，注册表显示启用、`ui.canvas >=1.6, <2`，摘要为 `e81212f379f7a2657e29509a3d4eeda454843c5047b8d8f4bdc5517a75f0eab7`，与交付包一致。

在用户实际窗口验证引用/作者边界向左移动约 40px 并反向恢复：作者与引用区域按相邻列调整，时间列保持，正确命中时不改变所选提交。随后截图观察到透明列边界的横向双箭头系统光标。第一处边界的一次落点在命中带外，选中了该行，不能计为拖动通过；之后用户持续操作窗口，自动输入工具明确报告用户输入，停止继续控制鼠标。**本次未完成用户窗口三条边界及滚动往返的完整自动验收，也未取得显示器实际呈现 FPS。** 上一阶段的原生验收范围和失败限制仍以上文为准。

### 新增可重跑的文字绘制耗时探针

`crates/editor-ui/src/text/canvas/tests/performance.rs` 使用真实 Windows 字体系统，直接调用生产 `CanvasText` 进入 GPUI 绘制。固定四列、25 行可见区域、75 行保留缓冲，含长中文/emoji 提交说明、长引用、粗体作者和时间；连续改变相邻列宽，再以同样数据连续平移视口。每组预热 20 帧，采样 120 帧。探针只增加测试代码，未修改生产绘制、虚拟化、滚动缓冲或刷新节奏。

```powershell
cargo test -p editor-ui --release canvas_column_resize_and_scroll_frame_cost -- --ignored --nocapture --test-threads=1
```

第二次运行的结果保存于 `target/git-column-drag-2026-10-24/installed-followup-native-text.log`：

| 文字绘制工作负载 | P50 | P95 | 最大值 |
| --- | --- | --- | --- |
| 连续列宽变化 | 0.463 ms | 2.900 ms | 3.319 ms |
| 连续视口平移 | 0.467 ms | 0.518 ms | 0.647 ms |

第一次运行的列宽/平移 P95 分别为 2.909/0.478 ms。数字包含原生文字准备和测试窗口绘制 CPU 工作，**不包含整应用、WASM 往返或 GPU 呈现，不是屏幕 FPS，也不是旧版/新版的端到端加速比**。该探针显式 ignored，报告分位数而不设置依赖机器负载的时间断言；是否脱离插件逐次应答仍由原有冻结事件接收端的行为回归测试验证。当前证据支持先落实安装更新，没有理由为此降低滚动质量或增加未经测量的缓存改造。

本次 `cargo fmt --check`、`cargo test --workspace --exclude editor-app`（555 通过、403 ignored）、`cargo check --workspace` 和 `git diff --check` 通过；被跳过的 403 项不计为通过。完整 workspace 日志分别为 `installed-followup-workspace-test.log` 和 `installed-followup-workspace-check.log`。编译保留已有警告，未修改本任务以外的代码。

针对性 `cargo test -p editor-app canvas_columns_paint_every_move_without_guest_roundtrips -- --nocapture` 通过（1 项，0.01 秒）：冻结插件应答后连续 12 次移动的实际绘制矩形逐帧跟随鼠标，移动期间不发送插件指针事件，松手仅发送一次最终列宽，旧绘图回复不会回滚预览。日志为 `installed-followup-native-columns.log`。本次只添加耗时探针与验证记录，生产修改仍为上一阶段的 0.1.42 实现；没有提交、推送或发布版本。

滚动状态回归 `cargo test -p editor-app ui::plugin::canvas::scroll::tests::ordered_acknowledgements_preserve_newer_preview_and_navigation_retires_it -- --exact --nocapture` 也通过（1 项，见 `installed-followup-scroll.log`），验证较旧的滚动确认不会覆盖更新的本地预览，导航更换内容时会撤销旧预览。
