# 窗口与内部布局拖动刷新检查

日期：2026-10-23。状态：保存合并、图标与树行优化、后台发布锁竞争修复已落地；用户反馈明显更顺畅，普通 Release 已构建，完整工作区稳定跟随当前屏幕刷新率仍待最终复测。逐步说明见[窗口拖动优化历程](window-drag-optimization-summary-2026-10-23.md)，本页保留各阶段原始证据与限制。

## 检查范围与刷新率口径

覆盖主窗口移动和缩放、原生对话框移动和缩放、内部停靠标题拖动、停靠边缘调整，以及横向和纵向分隔条。

目标使用窗口所在显示器的当前显示模式刷新率；可变刷新率显示器使用该模式的最大刷新率。例如面板支持 240 Hz、系统当前设置 144 Hz 时，目标为 144 Hz。程序不修改系统显示模式，也不将某台设备的 144 Hz 写成产品常量。

## 静态依据

- `Cargo.lock` 锁定 GPUI Kit/Base 0.8.0 与 GPUI 0.3.9。Windows 的 `PlatformDisplay::refresh_interval` 从当前显示模式读取周期，`Window::refresh_interval` 与 `observe_window_display` 提供跨屏、模式改变后的窗口局部信息。
- GPUI Windows 的独立 VSync 线程通过 `DwmFlush` 等待合成进度，再使窗口失效。主窗口、对话框和插件原生窗口均通过本地 `TitleBar::window_options` 创建，并共用平台绘制链路。[Microsoft 对 DwmFlush 的说明](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmflush)描述了等待待提交图形更新完成的语义。
- Win32 移动/缩放消息循环还使用 10 ms 的 `WM_TIMER` 来处理前台任务及绘制。不能仅由这个定时器推断 100 FPS 上限，因为 VSync 线程仍会使窗口失效；也不能把定时器触发次数当作屏幕帧率。[SetTimer 的最小间隔](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-settimer)为 10 ms。
- 本地停靠标题、停靠边缘和 `split_container` 直接复用 GPUI/Base 的输入、布局与失效通知，没有宿主固定 60/120/144 FPS 的拖动定时器。
- 修复前，宿主 `EditorApp` 的边界观察器在每次通知时调用 `SessionState::save`；该方法串行化整个会话、创建临时文件、执行 `sync_all` 并替换会话文件。连尺寸未变的通知也会同步写盘，给原生输入和绘制线程增加无必要的阻塞。

## 实现与回归

修改 `crates/editor-app/src/app/shell.rs` 的窗口边界观察器：尺寸立即更新到现有会话状态并通知布局；相同尺寸直接返回。连续尺寸变化只保留一个可取消的保存任务，最后一次变化后等待 200 ms，再保存当时最新的会话状态。这个间隔仅合并写盘，不用于限帧或轮询绘制。

`EditorApp` 持有任务；新尺寸取消旧任务，编辑器释放时任务随之取消。正常关闭仍经过现有 `prepare_project_shutdown`，同步保存最新内存尺寸，因此立即关闭不依赖延迟任务完成。没有新增第二份几何状态、固定高刷常量、GPUI fork 或依赖升级。

`crates/editor-app/src/tests/window_geometry.rs` 直接覆盖真实 `EditorApp` 观察器：

- 修复前，连续变化不应写盘、相同尺寸不应重写文件这两项测试均出现预期的行为断言失败。
- 修复后，模拟 240 Hz、持续约 500 ms 的缩放输入，确认每次内存尺寸立即生效，超过 200 ms 的持续输入仍不写盘，停止后保存最终尺寸。
- 相同尺寸保留会话文件原始内容；缩放后立即进入真实关闭流程仍保存最终尺寸。
- 三项 Release 回归全部通过。

## 验证入口

原生测试使用真实 Windows/D3D 窗口和生产控件。标题栏与边缘输入通过真实鼠标按钮状态进入系统移动/缩放循环；内部控件使用普通原生鼠标消息。输入线程按窗口当前显示周期的一半产生事件，帧率取 GPUI 绘制回调中观察到的几何变化，不取输入频率。每帧读取窗口当前显示周期，并检查前台运动绘制达到该刷新率的 90%；该容差是验收阈值，不是产品限帧配置。停靠标题场景还要求真实浮动拖动状态存在，释放后断言该状态已清理。

```powershell
$env:NANOBUG_DRAG_SURFACE = 'main'
cargo test -p editor-ui native_window_drag_tracks_display_refresh --release -- --ignored --nocapture --test-threads=1
```

`NANOBUG_DRAG_SURFACE` 的其他值：`dialog`、`resize`、`dialog-resize`、`dock`、`dock-resize`、`split`、`split-vertical`。测试在独立、有超时限制的进程中执行；测试结束恢复原鼠标位置。`NANOBUG_DRAG_LIGHT` 选择浅色主题，`NANOBUG_DRAG_CAPTURE` 可保存末帧图像。

```powershell
cargo test -p editor-app window_geometry --release -- --nocapture
```

实际项目窗口的尺寸恢复另行运行：

```powershell
cargo test -p editor-app project_selection_centers_native_window --release -- --ignored --nocapture --test-threads=1
```

## 本机测量与检查

Windows、NVIDIA RTX 5080；当时记录屏幕尺寸为 2560×1440，未保存独立的物理显示模式查询，不能将该尺寸直接称为显示模式。GPUI 报告 144 Hz。使用 Release 原生二进制逐项、串行运行深浅两套主题。最终 16 个场景全部通过并正常退出；位置未变化的绘制不计入运动帧率。

| 场景 | 深色 FPS | 浅色 FPS | 两主题最大运动帧间隔（ms） |
| --- | ---: | ---: | ---: |
| 主窗口移动 | 142.72 | 142.77 | 14.354 |
| 子窗口移动 | 143.98 | 142.72 | 14.174 |
| 主窗口边缘缩放 | 143.36 | 143.89 | 14.383 |
| 子窗口边缘缩放 | 142.14 | 142.75 | 14.694 |
| 停靠标题拖动 | 143.34 | 143.31 | 14.285 |
| 停靠边缘调整 | 143.31 | 143.36 | 14.161 |
| 横向分隔条 | 143.34 | 142.81 | 14.553 |
| 纵向分隔条 | 143.29 | 143.34 | 14.446 |

最终运行平均 142.14–143.98 FPS，达到本机模式刷新率的 98.7% 以上；帧间隔 p95 为 7.455–8.331 ms。深浅主题末帧图像已查看，停靠边界、标题与中文内容正常显示。

一次中间轮次的浅色停靠场景测得 130.80 FPS、最大间隔 156.405 ms，随后原生测试进程退出时报 `Thread Local Storage ... AccessError`（退出码 `0xc0000409`）。该轮不计通过，失败日志保留在本机 `target/drag-light-dock-teardown-failure.log`。单独复测及随后三次重复均正常；测试夹具又按[现有原生滚动验收的清理方式](comparison-viewport-scroll-2026-10-21.md)补齐失焦、移除自有窗口、等待释放后退出，最终上述 16 项全部正常退出。退出清理仅修改测试夹具；156 ms 长帧的具体来源没有确定，不能据此承诺所有运行都没有长帧。

- `cargo fmt --check`：通过。
- `cargo test --workspace --exclude editor-app`：519 项通过、384 项忽略；忽略项不计通过，原生拖动测试另行显式运行。
- `cargo check --workspace`：通过，存在既有警告。
- `cargo test -p editor-app window_geometry --release -- --nocapture`：3 项通过。
- 真实项目窗口尺寸恢复：1 项 Release 原生测试通过。
- `cargo tree -p editor-ui --edges normal` 与 `Cargo.lock`：未出现 `gpui-component` 或其宏库。
- `git diff --check`：通过。

首次构建曾按用户在其他会话设置的清理闸门中止，该次不计通过；闸门释放后重新执行原入口。其他会话已有改动未纳入本次修复。

## 结论边界

本次测量证明当前 144 Hz 设备上，共用原生窗口绘制链路及生产停靠/分隔控件能够接近显示刷新率；新增修复移除了宿主窗口边界事件中的逐次同步写盘。不会用固定 144 Hz 定时器替代原生刷新链路。

测量的是 GPUI 绘制回调频率，不是独立的 GPU Present 或屏幕扫描采样；原生窗口场景主动请求连续重绘来检测模态拖动循环的供帧能力，内部控件靠实际拖动驱动重绘。该结果不等于繁忙完整工作区在所有内容负载下都能每帧满速。未物理验证异刷新率跨屏、运行时切换刷新率、VRR、240/360 Hz 设备，以及所有 DPI 组合；这些不计为已验收。键盘、中文 IME 和滚动路径未被本次行为修改。

## 实际运行程序复查

用户反馈完整项目窗口持续快速移动时明显滞后。该反馈与上述绘制回调测试不是同一验收口径：绘制可以持续发生，而窗口位置跟随仍然延迟。此前测试还主动请求连续绘制、使用轻量测试内容，并裁去开头和结尾各 150 ms；不能据此宣布完整项目窗口的输入到呈现延迟合格。

本次核对用户实际窗口 HWND `398614`、PID `77588`：进程运行 `target/release/editor-app.exe`，启动于 16:09:11，但文件及 PE 链接时间均为 **14:45:39**。该文件的 SHA256 为 `D1AFD065DF7D6FC69A3AC31DC2927110B29F1A69FDAB239144880E1DBF20775B`。EXE 与 PDB 的 GUID/age 匹配，实际进程回调的 236 字节也与磁盘文件一致；其边界回调直接调用 `SessionState::try_save`，没有尺寸相等返回和延迟保存逻辑。反汇编证据保留于本机 `target/window-drag-running-old-disassembly.log`。

因此，“运行的是 Release”准确，但不能推出“运行的是包含修复的最新 Release”。此前 `window_geometry` 成功记录运行的是 `target/release/deps/editor_app-c60b40604cb0e95d.exe` 测试程序，不是上述生产 EXE。另一个任务对独立输出目录的构建也曾更新普通 Cargo 指纹；指纹时间不能代替实际 EXE/PDB 核对。

本轮通过 Cargo 官方的最终产物／中间缓存分离入口，为**同一个当前 checkout** 构建独立生产程序；其他源码副本仍须独立缓存。构建前 `cargo metadata` 核对 `workspace_root` 为当前仓库、`target_directory` 为下述新目录、`build_directory` 为既有 `target`。

```powershell
cargo build -p editor-app --bin editor-app --release --locked `
  --target-dir C:/Projects/RustProjects/Editor/target/window-drag-current `
  --config 'build.build-dir="C:/Projects/RustProjects/Editor/target"'
```

构建成功，存在既有 Rust/Wasmtime 链接警告，日志为本机 `target/window-drag-production-build.log`。新文件为 `target/window-drag-current/release/editor-app.exe`，PE 链接时间 **19:18:56**，SHA256 为 `322E3ACAD6DE75F284BA65F3E8F616796C92CE7576CB65030253B7B70C6BE0F0`。匹配的 EXE/PDB GUID 为 `21341EDB-CC02-4540-A269-FA631B5B6572`、age 1；反汇编确认 observer 调用 `update_window_geometry`、尺寸相等直接返回、变化后创建 200 ms 定时任务再保存。构建没有替换或修改正在运行的旧 EXE。该新产物还包含当前工作区其他任务已经完成的改动，不能作为只改变单一变量的性能实验。

### 真实鼠标拖动：修复写盘之后仍有问题

使用独立进程以约 500 Hz 只读采集 `GetCursorPos`、`GetWindowRect`、前台及左键状态；观察线程使用 Per Monitor V2 DPI 上下文，统一为物理坐标。自动化工具的两次拖动只产生起终点跳变，不代表持续输入，已排除。以下来自用户真实连续左右移动，窗口物理尺寸均为 3047×1676；计数使用相邻样本的位置变化次数，而非去重坐标数量。

| 实际生产程序 | 连续段时长 | 位置变化次数 | 位置变化频率 | 变化间隔 p50 / p95 |
| --- | ---: | ---: | ---: | ---: |
| 14:45 旧 Release | 10.686 秒 | 642 | 60.077 次/秒 | 16.248 / 22.226 ms |
| 19:18 含写盘修复的 Release | 9.193 秒 | 548 | 59.608 次/秒 | 16.152 / 22.376 ms |

新进程于 19:24:42 正常启动，PID `59120`、HWND `791792`，路径确认为独立产物；用户仍反馈滞后。新段鼠标位置变化约 442 次/秒，采样最大间隔 2.891 ms，足以区分约 16.7 ms 和 6.94 ms 的窗口位置变化节奏。新版出现的一次 142.585 ms 位置不变区间中，鼠标仅移动 1 px，不能把它归为卡顿长帧。

19:21:45 查询旧窗口所在 `DISPLAY1` 的 `EnumDisplaySettingsW(ENUM_CURRENT_SETTINGS)`，物理模式是 **3840×2160、144 Hz**；`QueryDisplayConfig` 的路径和目标信号均为 144/1 Hz。未经 DPI 设置返回的 2560×1440 是虚拟化尺寸，不应写成物理显示模式。另一活动屏幕为约 130 Hz；本记录不据此归因。模式查询在捕获之后，CSV 没有同步采集显示模式。

这些数据证明当次 **OS 窗口坐标更新约 60 次/秒**，不等于 GPU Present 或扫描输出只有 60 FPS；轨迹对齐得到的约 9 ms 也不是端到端延迟或延迟 p95。用户另反馈同屏拖动 Codex 正常。当前不能把同步写盘修复宣称为“持续快速拖动已解决”。

本轮再次运行 `cargo test -p editor-app window_geometry --release --locked -- --nocapture`，三项通过；日志为本机 `target/window-drag-current-geometry-tests.log`。原始坐标、只读采样器和分析器保留于本机明确的调试目录 `C:/Users/Tmiracle/.codex/tmp/nanobug-drag-latency-20261023/`，仓库 `target/window-drag-{old,new}-motion-summary.json` 保存摘要。WPR 的 CPU/GPU 采集被系统权限拒绝（`0xc5585011`，未开始录制）；后续使用 GPUI 公共 profiler 接口区分主线程绘制与提交耗时。

### 完整工作区的 CPU 阶段诊断

使用独立 Release 诊断产物及 GPUI 公开 `profiler` 接口，后台每 100 ms 排空时序记录；UI 线程不写日志文件。独立进程继续只读采样物理鼠标／窗口坐标。这里的 `present` 包含平台渲染和提交的 CPU 调用区间，不是屏幕扫描时间，也不是单独的 DXGI 调用计时。

默认 DirectComposition 诊断进程 PID `27012` 的真实拖动，在第 6–14 秒连续运动区间测得 530 次位置变化（66.25 次/秒）、646 次 draw。draw 中位数 **10.787 ms**，其中 prepaint **7.762 ms**；present 中位数 **0.107 ms**、p95 **0.133 ms**。501 次 prepaint 单独超过屏幕 144 Hz 的 6.944 ms 预算。平台信号均为 `NativeCallback`，没有看到固定 60 Hz 定时调度的证据。完整按键段的开头包含停顿和一次两进程同时发生的采样缺口，因此不以全段平均 50 次/秒作为稳态结论。摘要见本机 `target/window-drag-default-frame-summary.json`。

随后只增加静态 Rust 视图类型名探针。PID `49080` 的末个连续 9.307 秒区间内，`gpui_base::tree::TreeState` 的 prepaint self 时间中位数 **4.255 ms**、p95 **5.118 ms**，累计占 draw 时间 **45.16%**、占 prepaint 时间 **64.17%**。上游 self 计时扣除了嵌套视图，但仅记录不小于 0.5 ms 的阶段；未记录不表示零开销。外部采样最大间隔 2.559 ms，位置变化 77.791 次/秒。本轮 Git 面板已经隐藏，因此只用来归因热点，不作为前一轮的性能改善对照。摘要见 `target/window-drag-types-frame-summary.json`。

锁定依赖的移动事件会调用 `Window::bounds_changed`，进而 `refresh()` 将 `refreshing` 置位；缓存面板在该状态下重新布局和绘制。资源管理器行的图标规则又在名称／扩展名匹配之前查询祖先目录的 `project_markers`，使不相关目录行也反复同步访问磁盘。后续针对这一可观测路径修复，保持原生刷新信号及规则匹配语义。

### 第一轮图标规则修复：实测部分改善

`ui/icons.rs` 调整为先判断原有目录／文件名／扩展名选择器，仅命中时才检查祖先项目标记。未命中的可见行不再为该规则同步查询文件系统；匹配结果、祖先顺序和图标优先级保持不变。

第一轮修复诊断进程 PID `61748`、HWND `398712` 的真实拖动记录为本机 `target/window-drag-fixed/release/window-drag-61748-1792757354341.jsonl`。其中一个连续长段测得：

| 指标 | 结果 |
| --- | ---: |
| 连续运动区间 | 6.5705 秒 |
| OS 窗口位置变化 | 640 次，97.405 次/秒 |
| draw | 637 次，96.949 次/秒 |
| draw 耗时 p50 / p95 | 6.9597 / 12.2213 ms |
| `TreeState` prepaint self 耗时 p50 | 3.23 ms |
| present 耗时 p50 / p95 | 0.0886 / 0.1285 ms |

用户反馈拖动有所改善，但仍不够高刷。位置变化约 97 次/秒仍未接近当前 144 Hz 模式；draw 中位数也已接近 6.944 ms 的单帧预算。present 仍是上述 CPU 渲染／提交区间，不能由它推断屏幕呈现延迟。

本轮布局和打开的标签已变化，不能与前两轮组成严格的单变量 A/B 实验，也不能把全部差异归于图标修复。此记录只确认当次完整工作区的部分改善，持续拖动达到显示刷新率尚未验收完成。

### 第二轮树行优化：移除逐行查询与主题复制

第二轮将每个可见行的 `Path::is_dir` 改为工作区快照的目录索引查询。索引与该代树节点一同生成并整体替换，保留根目录、显式空目录，以及由文件／目录路径推导的祖先；刷新、重命名和移除后的新快照不保留旧目录分类。锁定版本的 `TreeItem` 没有可附加的类型数据，`is_folder` 仅检查是否有子节点，不能用于空目录分类，因此索引放在 `workspace_snapshot` 邻近。文件操作时的实时磁盘校验不受这次绘制路径调整影响。

同时把 `theme::active_theme` 从每行调用移到每次资源管理器树渲染时调用一次，再供当次所有行复用，避免逐行克隆包含主题组件及插件映射的完整定义。它仍随下次树渲染重新读取当前主题。

`explorer::tree::tests::directory_index_*` 两项回归覆盖路径无需真实存在时的根目录、空目录、祖先推导、根外路径排除，以及新快照中的重命名、删除、目录变文件和空工作区。运行 `cargo test -p editor-app --bin editor-app --offline -j 1 -- ui::icons::tests explorer::tree::tests window_geometry --nocapture`，14 项全部通过；日志为本机 `target/window-drag-targeted-bin-final.log`。这些测试验证分类、图标规则与窗口保存行为，**不代替真实拖动和主题切换验收**。

未限定 `--bin editor-app` 的首次针对性测试在构建既有 `svg_preview_render` 示例时失败：其 `Paint` 匹配未处理其他任务新增的 `TextBox` 变体。本轮格式复查也命中其他任务的 `crates/plugin-runtime/tests/maven_package/application.rs` 排版；上述范围外问题分别记录在 `target/window-drag-targeted-final.log`、`target/window-drag-final-fmt-recheck.log`，没有据此宣称完整检查通过。

默认并行度的 workspace 测试曾因内存耗尽失败。以 `cargo test --workspace --exclude editor-app --offline -j 1` 重试后，61 个已完成测试二进制累计 490 项通过、1 项失败、253 项忽略；Cargo 在 `language_declarative` 停止，后续目标及 doctest 未运行。失败的资源包在测试内构造，测试硬编码能力版本为 1.7.0，当前其他任务修改的公开能力版本为 1.9.0；该测试不涉及本次 UI 绘制路径，也不读取冻结 ZIP 账本。记录见 `target/window-drag-final-workspace-tests-serial.log`。`cargo check --workspace --offline -j 1` 已退出 0，记录见 `target/window-drag-final-workspace-check.log`。

独立诊断 Release 产物为 `target/window-drag-tree-fixed/release/editor-app.exe`，文件修改时间 **20:18:56**，SHA256 为 `E65C8553BA1661C7952E908A4C9FFC790E38E5418F6CD18BFD3E023B587B0D0C`。该产物已启动为 PID `22072`、HWND `595310`；最初因其他任务仍在编译而暂缓采样。随后 20:34–20:39 的采样窗口已经结束：`profile-tree-fixed-2-motion.csv` 的 149,614 个样本均为 `ButtonDown=False`，OS 窗口位置变化为 0。两份审计摘要 `target/window-drag-tree-fixed-2-{first,second}-audit-summary.json` 都没有有效运动段，诊断捕获均因 `time_limit` 结束。它们不能用于评价用户后来发生的拖动，未提供第二轮真实运动的验收证据；后续新诊断产物的有效采样另列于末节。构建、启动和空闲绘制成功不代表高刷目标已经达成。

### 多标签反馈与自动全量刷新压力测试

21:56，用户反馈第二轮程序刚开始稍顺，打开多个 tab 后明显变卡；随后补充“看 tab 项的文件长度，多了就卡”。最初尚不确定“长度”指文档内容还是标签名称；用户在后续真实采样时已明确指文件内容和行数，并确认完成了拖动。本轮继续按原始授权排查，但反馈中的内容规模相关性仍需受控测量验证。

新诊断 Release 为 `target/window-drag-tabs-probe/release/editor-app.exe`，文件修改时间 **22:10:08**，PID `55152`，SHA256 为 `2171EAEB8FAAB3EAACB97BAAC14DAE5B17BC458BE8A5AFADC1E79A650E0A23AD`。构建时的相关源码摘要保留于 `target/window-drag-tabs-probe-sources.json`；该产物来自继续变化的当前 checkout，不能与 20:18 产物视为只有探针差异的严格 A/B。

诊断入口使用公开 `Window::on_next_frame` 和 `refresh()`，在前台窗口持续约 8 秒请求全量刷新，以观察完整 UI 的 CPU 绘制压力。它不注入 OS 输入，也不移动窗口，**不是真实拖动、输入延迟或屏幕扫描验收**。下表使用各轮完整 benchmark 区间内同一窗口的 draw 开始次数，未裁去首尾，也不把 refresh 请求次数当作 draw 次数。

| run | 打开的标签数／当前文档 | 区间时长（秒） | draw 次数 | draw 开始频率（次/秒） |
| --- | --- | ---: | ---: | ---: |
| 2 | 5／`main.rs`，约 59 KB | 8.0011 | 1075 | 134.356 |
| 3 | 5／`ui.rs`，249 B | 8.0021 | 1133 | 141.589 |
| 4 | 6／`extensions.rs`，约 98 KB | 8.0032 | 1132 | 141.444 |
| 5 | 6／`ui.rs`，249 B | 8.0026 | 1106 | 138.205 |
| 6 | 10／`ui.rs`，249 B | 8.0011 | 1152 | 143.981 |

run 2–5 的摘要分别为 `target/window-drag-tabs5-main-summary.json`、`target/window-drag-tabs5-ui-repeat-summary.json`、`target/window-drag-tabs6-extensions-summary.json`、`target/window-drag-tabs6-ui-summary.json`。run 6 依据 `target/window-drag-tabs10-ui-benchmark.json` 和原始 `target/window-drag-tabs-probe/release/window-drag-55152-1792765302183.jsonl`，按 benchmark 的 `[start_ms, end_ms)` 及 `window_id` 筛选 draw 计数。最初 run 1 的 5 标签 `ui.rs` 捕获因 `window_inactive` 在 5.7285 秒提前结束，807 次 draw、140.875 次/秒，单独保留于 `target/window-drag-tabs5-ui-summary.json`，不称为完整 8 秒运行。

这些场景没有呈现“文档越长或标签越多，刷新越慢”的简单单调关系，但不能据此排除用户反馈。其他任务的 Cargo／rustc 在后台并行运行，资源竞争没有受控；当前文档、标签集合及可见内容也并非全部相同。频率是 CPU draw 启动频率，不能转换为 OS 窗口位置更新率，更不能声明拖动已经跟手。

以 run 2 为例，`tabs.prepare`、`shell.language_servers`、`shell.plugin_documents` 的累计耗时除以 draw 数分别约为 **0.0646、0.1493、0.0645 ms/帧**，未记录到 `language.linked_input` 阶段。这些 phase 是包含子调用的墙钟区间，不能相加；尤其 `tabs.prepare` 只覆盖宿主准备标签数据和元素的代码，不包含随后 `FileTabStrip`／`Tab` 渲染、文本测宽、布局求解及 prepaint，不能用它排除长标签或标签溢出的绘制成本。

### 新诊断产物的真实拖动：仍未达到 144 Hz

用户确认拖动后，PID `55152` 的真实采样已停止，capture 以 `stop_marker` 正常结束。原始诊断为 `target/window-drag-tabs-probe/release/window-drag-55152-1792765382551.jsonl`，坐标和时钟锚点为本机调试目录中的 `profile-tabs-probe-motion.csv`、`profile-tabs-probe-motion-meta.json`；合并摘要为 `target/window-drag-tabs-probe-frame-summary.json`。

摘要识别出 9 个左键持续按住且目标窗口处于前台的区间，均完整落在 profiler 捕获范围内，窗口尺寸未改变。可观测的 journal／discontinuity 丢失计数为 0；GPUI frame ring 未公开丢失计数，不能把这一未知项也写成已确认零丢失。以下列出最长区间和最后区间，保持全部区间数据可查，不将短暂停顿从统计中移除。

| 区间 | 时长（秒） | OS 位置变化次数／频率 | draw 次数／频率 | draw 耗时 p50 / p95（ms） |
| --- | ---: | ---: | ---: | ---: |
| 第 7 段，最长 | 9.8360 | 1126／114.477 次/秒 | 1107／112.546 次/秒 | 5.3890 / 10.0389 |
| 第 9 段，最后 | 5.8382 | 658／112.706 次/秒 | 654／112.021 次/秒 | 5.3922 / 9.3690 |

两段 GPUI 报告的显示周期均为 6.944444 ms，即 144 Hz；OS 位置变化仍约 112–114 次/秒，不能宣称持续拖动已达到目标。第 7 段最长 86.213 ms 的位置不变区间内，鼠标只移动 1 px，不能直接计为卡顿；第 9 段最长 47.330 ms 区间内有 22 次鼠标变化、累计运动约 79.64 px，是仍需解释的跟随间隔，不能把该间隔直接称为端到端输入延迟。

捕获未逐段记录活动文件，截图也受到 Codex 窗口遮挡；当前界面状态不能证明之前各段显示了哪个文件。因此这些数据不归因到特定文件、字节数或行数，也不与自动刷新压力测试组成严格 A/B。用户反馈的内容规模相关性、剩余跟随间隔来源及最终高刷目标，均尚未验收完成。

### 同进程采集开关对照与剩余 CPU 成本

PID `55152` 关闭内部捕获后，`restore_trace` 将 trace 恢复为关闭状态；保持 10 个标签及当时的 `main.rs` 界面，由外部进程继续只读采集坐标。用户连续拖动 10.4674 秒，窗口位置变化 1198 次，频率 **114.451 次/秒**，变化间隔 p50 / p95 为 **8.250 / 12.580 ms**。摘要为 `target/window-drag-tabs-unarmed-motion-summary.json`，原始数据为本机调试目录中的 `profile-tabs-unarmed-motion.csv`。该频率与上表开启采集的第 7 段 **114.477 次/秒** 几乎一致，不支持“开启 trace 或后台日志采集造成当次主要降速”的解释。

这仍是同一个启用了 `profiler` feature 的诊断二进制，不等于普通 Release 的无插桩对照。锁定 `gpui-pre 0.3.9` 的 `src/profiler.rs::WindowProfiler::{begin_view,end_view}` 在每个 view 阶段执行时钟读取和嵌套耗时栈操作；0.5 ms 阈值只控制是否记录 view 事件，不免除较短阶段的计时。view journal 使用线程局部状态和非阻塞 ring；frame 记录与 `FrameTimingCollector::collect_unseen` 则共享 `FRAME_TIMINGS` 自旋锁，收集器在锁内复制结果。JSON 序列化与文件写入位于后台线程。关闭捕获减少了 trace 收集工作，但仍保留 feature 内的计时、journal 和直方图成本；本轮没有测得这些残余成本的独立大小。

对前述第 7、9 段分别裁去首尾各 250 ms，位置变化频率仍为 **117.101 / 115.814 次/秒**，draw 频率为 **114.931 / 115.020 次/秒**，因此完整区间中的起止停顿不能解释全部差距。保留上述完整区间作为主记录，裁剪结果仅作补充；坐标频率分母取保留样本的首末跨度，比固定 profiler 区间分别短约 2.2 / 2.1 ms。裁剪区间的平均 CPU 成本如下：

| 阶段，毫秒／draw | 第 7 段 | 第 9 段 |
| --- | ---: | ---: |
| draw 总计 | 5.793 | 5.765 |
| request_layout | 1.541 | 1.519 |
| prepaint | 3.068 | 3.029 |
| paint | 0.809 | 0.827 |
| `EditorApp` request_layout self | 1.332 | 1.314 |
| `TreeState` prepaint self | 1.040 | 1.035 |
| `EditorDockPanel` prepaint self | 0.795 | 0.768 |

prepaint 约占 draw 时间的 53%，root layout 也是主要成本。`tabs.prepare` 约为 **0.092 / 0.090 ms/帧**，其中包含的 `tabs.icon` 约为 0.036 ms/帧；`shell.language_servers` 约为 **0.224 / 0.241 ms/帧**，`shell.plugin_documents` 约为 **0.100 / 0.109 ms/帧**，没有记录到 `language.linked_input`。这些包含子调用的 app phase 与 view self 不能相加，当前数据也不足以把主成本归因到文档行数。裁剪后的第 9 段有 614 个 draw、613 个部分 phase／present，是按各事件自身起点筛选造成的边界差异，不是已证实的记录丢失。

### 真实拖动中的连续刷新对照

继续使用同一 PID `55152`、同一内部捕获，由外部只读坐标观察器在发现前台窗口随按住的左键移动后写入已有 benchmark marker；没有注入输入或移动窗口。run 7 在 profiler 时钟 **24247.5146–32250.8659 ms** 持续请求 `Window::refresh/on_next_frame`，正常运行 **8.003351 秒** 后结束；内部 capture 由手动启动持有，因此 benchmark 结束后仍继续采集。原始记录为 `target/window-drag-tabs-probe/release/window-drag-55152-1792766032645.jsonl`，坐标／时钟锚点为本机调试目录中的 `profile-tabs-demand-motion.csv`、`profile-tabs-demand-motion-meta.json`，摘要为 `target/window-drag-tabs-demand-{frame,benchmark}-summary.json`。

实际鼠标按住区间分为两段，中途松手。第一段与 benchmark 重叠 4.8488 秒，位置变化 467 次、约 **96.392 次/秒**，draw 584 次、约 **120.442 次/秒**。完整 benchmark 的 942 次 draw、约 117.7 次/秒包含两次拖动和中间松手，不能作为一次连续拖动的频率。第二段持续按住跨越 benchmark 结束点，可按同一段的前后区间比较：

| 第二段内的区间 | profiler 区间（ms） | 位置变化次数／频率 | draw 次数／频率 | draw 耗时 p50 / p95（ms） |
| --- | --- | ---: | ---: | ---: |
| 连续刷新开启，完整 2.5612 秒 | 29689.6588–32250.8659 | 246／96.098 次/秒 | 273／106.590 次/秒 | 5.5666 / 10.0230 |
| 连续刷新结束，完整 7.9587 秒 | 32250.8659–40209.5963 | 824／103.570 次/秒 | 795／99.890 次/秒 | 5.4718 / 10.7343 |
| 开启区间首尾各裁 250 ms | 29939.6588–32000.8659 | 203／98.521 次/秒 | 225／109.159 次/秒 | 5.5487 / 9.4026 |
| 结束区间首尾各裁 250 ms | 32500.8659–39959.5963 | 782／104.875 次/秒 | 754／101.090 次/秒 | 5.4705 / 10.6749 |

区间采用 `[start, end)`；坐标仅统计区间内相邻样本的位置变化，其实际样本跨度比分列的 profiler 区间短约 0.7–2.8 ms。裁剪后的位置变化间隔 p50 / p95，开启期间为 **8.495 / 16.266 ms**，结束后为 **8.269 / 14.660 ms**。窗口尺寸未改变，最大采样间隔分别为 2.434 / 3.122 ms，journal 与 app phase 的可观测丢失均为 0。最长位置不变区间分别为 70.557 / 114.800 ms，鼠标在这两处均只移动 1 px，不能直接作为卡顿证据。

本次连续刷新增加了 draw 频率，**没有改善实际窗口位置更新率，也没有使其接近 144 次/秒**。两侧时长、鼠标轨迹与停顿不同，短段数值差异也不能证明连续刷新造成退化。裁剪后的平均 draw 耗时为 6.119 / 5.968 ms，prepaint 为 3.215 / 3.129 ms，request_layout 为 1.593 / 1.590 ms，没有出现主要成本结构的改变。该实验不支持把持续 `refresh()` 直接加入产品作为当前问题的修复；位置更新及 CPU present 仍不等于屏幕扫描频率。

### 锁定上游的纯移动刷新边界

只读核对当前锁定的 `gpui-pre 0.3.9` 与 `gpui-pre-windows 0.3.9` 源码：

- `gpui-pre-windows/src/events.rs::handle_move_msg` 更新原点并调用 moved callback；`gpui-pre/src/window.rs` 的 `on_moved` 连接到 `Window::bounds_changed`，后者先调用 `refresh()`，再触发 bounds observers。`gpui-pre/src/view.rs` 的缓存复用条件要求 `!window.refreshing`。因此宿主同尺寸早退可以避免自身保存和通知，却不能撤销上游已经发生的全量刷新。
- `gpui-pre-windows/src/events.rs::{handle_size_move_loop,handle_timer_msg}` 在进入移动／缩放模态循环时设置 timer，每个 tick 排空前台 runnable 队列，再同步调用绘制路径；这里没有普通 `platform.rs::run_foreground_task` 的 10 ms 让出预算。独立 VSync 线程仍通过 `RedrawWindow` 触发绘制，不能仅凭该 timer 推断帧率上限或故障根因。当前 profiler 的 `NativeCallback` 来源字段不能区分 `WM_TIMER` 与 `WM_PAINT`。
- `Window` 的 `platform_window`、`invalidator`、`refreshing` 均非宿主可访问的公开状态；公开 `observe_window_bounds` 是刷新后的观察器，`on_next_frame` 与 `refresh` 只增加刷新需求。本次未找到“保留原生位置更新，同时取消纯移动全树刷新”的宿主公开接缝，也未修改或 fork 上游依赖。

### 详细阶段探针：定位宿主长事件

详细探针诊断产物为 `target/window-drag-detail-probe/release/editor-app.exe`，PID `59592`，SHA256 为 `0436187989D11E50631300E0234AC19DB65F252C7CC3790E8C200CDF4BE79B24`。原始捕获为同目录的 `window-drag-59592-1792766654216.jsonl`，摘要为 `target/window-drag-detail-main-summary.json`。完整 benchmark 区间为 **0.5602–8002.1629 ms**，持续 **8.001603 秒**，1137 次 draw、**142.097 次/秒**；draw 耗时 p50 / p95 / max 为 **5.007 / 10.009 / 18.077 ms**。这仍是未移动窗口的连续刷新 CPU 压力测试。

23,877 条 app phase 均能对应到该轮的 1137 个 draw，没有帧外 phase；journal 与 app phase 可观测丢失均为 0。完整 benchmark 的宿主细项如下，phase 耗时包含子调用，不应相加：

| app phase | 平均耗时（ms） | p95（ms） | 最大耗时（ms） |
| --- | ---: | ---: | ---: |
| `shell.render_total` | 1.0550 | 1.3143 | 11.0322 |
| `shell.plugin_views` | 0.3443 | 0.2985 | 7.5187 |
| `shell.language_servers` | 0.2445 | 0.2572 | 10.0373 |
| `shell.plugin_documents` | 0.1078 | 0.1183 | 6.5172 |
| `shell.controls_sync` | 0.0187 | 0.0195 | 5.5491 |
| `source.styles` | 0.1760 | 0.2123 | 0.3029 |
| `tabs.prepare` | 0.0894 | 0.1187 | 0.3589 |
| `problems.rows` | 0.0068 | 0.0078 | 0.0255 |

`plugin_views`、`language_servers`、`plugin_documents` 分别有 40、4、2 个超过 2 ms 的事件，分布在不同的 46 帧，没有三者同帧同时超过 2 ms。`plugin_views` 长事件帧的平均 request_layout 为 **5.982 ms**，其余帧为 **1.234 ms**；扣掉各帧该 phase 后分别为 **1.049 / 1.057 ms**，基本一致。两组 prepaint 平均仅为 **3.022 / 2.951 ms**，paint 为 **0.828 / 0.790 ms**，因此这些长 layout 主要集中在该宿主调用区间，而非所有绘制阶段同时增加。46 帧占该轮 108 个超过 6.944 ms 的 draw 中的 42.6%，不能解释全部长帧。

`plugin_views` 长事件起点间隔 p50 为 100.783 ms，39 个相邻间隔中 20 个落在 85–105 ms；其耗时分布在 2.384–7.519 ms，没有稳定的 5 / 10 ms 耗时量化或复发周期。此时仅凭墙钟耗时还不能断言等待锁；后续对实际锁获取和后台持锁分别计时。

### 锁探针：确认后台发布阻塞 UI 获取共享状态

锁探针产物为 `target/window-drag-lock-probe/release/editor-app.exe`，PID `77760`，SHA256 为 `5D3E5D3ADB54EF103A361F199A0B6134BB5A48BD4233B4DF8F7AE12C54463405`。原始捕获为同目录的 `window-drag-77760-1792767006217.jsonl`。完整 benchmark 区间为 **0.4124–8004.6296 ms**，持续 **8.004217 秒**，1135 次 draw、**141.800 次/秒**，draw 耗时 p50 / p95 为 **4.961 / 9.751 ms**。

`extensions/preview/preference_import.rs` 中的 `shell.preference_lock` 与 `extensions/editor_requests.rs` 中的 `shell.document_lock` 只包围 `worker.state.lock()` 获取过程，不包括后续收据处理、文档版本差分或保存。`extensions/worker/runner.rs` 中的 `worker.publish_hold` 在后台已取得同一 `Published` guard 后开始，到 guard 释放前结束。两类记录使用同一进程的单调时钟直接求交集，后台 phase 不归入 UI draw，也不把它与父 phase 的重叠时间重复相加。

| benchmark 内超过 2 ms 的锁获取 | 次数 | 获取累计耗时 | 与后台持锁累计重叠 | 重叠比例 |
| --- | ---: | ---: | ---: | ---: |
| `shell.preference_lock` | 34 | 155.0618 ms | 154.6773 ms | 99.752% |
| `shell.document_lock` | 1 | 5.4009 ms | 5.3867 ms | 99.737% |

34 次 preference 长等待全部与后台发布持锁重叠，并在对应 hold 探针结束后 p50 **0.0107 ms**、p95 **0.0174 ms**、最大 **0.0204 ms** 完成获取。document 唯一长等待为 **3074.2890–3079.6899 ms**，对应 hold 为 **3074.2685–3079.6757 ms**，完成获取比 hold 探针结束晚 0.0142 ms。父阶段超过 2 ms 的事件也全部与 hold 重叠：`plugin_views` 37/37、累计重叠占 95.851%；`language_servers` 4/4、96.444%；`plugin_documents` 1/1、98.332%。锁获取的独立探针与同锁持有区间相互印证，确认该轮后台发布阻塞了 UI 读取共享状态。

同区间口径下，benchmark 内按起点筛选有 **83 次** `worker.publish_hold`，累计 **526.3832 ms**，平均 **6.3420 ms**，p50 / p95 / max 为 **6.0782 / 8.0997 / 20.4199 ms**。整个 capture 持续 8.091621 秒，另含 benchmark 结束后的 **8044.8086–8051.9144 ms** 一次 hold，所以全捕获为 **84 次、533.4890 ms**；不能把 84 次混入 benchmark 汇总。全捕获 hold 起点间隔 p50 为 93.703 ms，journal 与 app phase 可观测丢失均为 0。

被测实现的发布调用链位于 `Published` 互斥锁内：`extensions/worker/runner.rs` 调用 `Published::publish_manager`，经 `extensions/worker/command_epochs.rs::reconcile_availabilities` 进入 `Manager::availabilities` → `availability` → `availability_in_scope` → `package::metadata`；后三个 Manager 方法位于 `crates/plugin-runtime/src/manager/activation/mod.rs`。因此包元数据读取／验证等 Manager 派生工作与 UI 获取发布状态共用临界区。此次 hold 探针没有把内部计算、文件访问或线程调度分别计时，不能将全部持锁时间归为 CPU 或磁盘 I/O，也不能仅由该链路断言某一个元数据调用独占所有耗时。

该轮结束时已确认这一共享锁阻塞路径，锁外快照修复的行为与实际收益尚待后续验证。两轮 CPU benchmark 接近 142 次 draw/秒不能替代真实拖动验收，也不能证明文档内容规模与拖动滞后的关系已经解释完整。

### 发布快照移出共享锁：修复后的真实拖动复测

修复版为 `target/window-drag-publication-fix/release/editor-app.exe`，PID `75920`、HWND `10815898`，SHA256 为 `E3F84C8C725009ABB324174849D0BB032C55AEE62F85E1AA6ECCE84904E0DE6C`。原始捕获为 `target/window-drag-publication-fix/release/window-drag-75920-1792768343233.jsonl`，汇总为 `target/window-drag-publication-fixed-frame-summary.json`。只读坐标采样及其时钟锚点保存在本机：

- `C:/Users/Tmiracle/.codex/tmp/nanobug-drag-latency-20261023/profile-publication-fixed-motion.csv`
- `C:/Users/Tmiracle/.codex/tmp/nanobug-drag-latency-20261023/profile-publication-fixed-motion-meta.json`

`extensions/worker/command_epochs.rs` 的 `ManagerPublication::capture` 在 `Published` 锁外准备 Manager 派生快照，`runner.rs` 保持同一 Manager guard 覆盖快照准备到发布完成，取得 `Published` 锁后统一应用结果。快照不携带可覆盖现状的 command epoch；应用时按当前 `Published` 的撤销代数协调可用性和实例变更，保留已经发生的提前退役。命令实际接纳仍通过 live Manager 状态检查，不能由发布快照授权。这一调整针对后台发布临界区，不表示所有其他共享锁路径都已完成性能审计。

用户完成这轮真实拖动后反馈“明显更顺畅”。原始 capture 持续 **69.820262 秒**，由 stop marker 正常结束；坐标采样 29,561 条，目标间隔约 2 ms。metadata 与 JSONL 头对齐得到 `profiler_ms = CSV.TimeMs + 10504.6313`，锚点误差约 1 ms。按 `ButtonDown && Foreground` 连续至少 1 秒筛出以下全部三段，没有只选取最快的一段。

#### 共享锁尖峰复核

全 capture 有 **684 次** `worker.publish_hold`，p50 / p95 / max 为 **0.0450 / 0.0732 / 0.1873 ms**；2261 次 preference 锁获取的最大耗时为 **0.0038 ms**，2283 次 document 锁获取的最大耗时为 **0.0002 ms**。三个 phase 均没有超过 2 ms 的事件，因此此前“超过 2 ms 的 UI 锁等待与后台 hold 重叠”的现象在本次样本中没有再出现；没有长等待样本可用于计算同口径重叠比例。

| 真实拖动段 | hold 次数 | hold p50 / p95 / max（ms） | preference 获取最大耗时（ms） | document 获取最大耗时（ms） |
| --- | ---: | ---: | ---: | ---: |
| 1 | 42 | 0.0444 / 0.0584 / 0.0732 | 0.0007 | 0.0001 |
| 2 | 69 | 0.0381 / 0.0502 / 0.0603 | 0.0038 | 0.0001 |
| 3 | 77 | 0.0303 / 0.0485 / 0.0564 | 0.0009 | 0.0001 |

三段中的 `shell.plugin_views`、`shell.language_servers`、`shell.plugin_documents` 也均没有超过 2 ms 的事件。旧锁基准的 hold p50 为 6.0782 ms，新 capture 的 hold 已降到几十微秒，且 UI 锁获取尖峰消失，支持本次修复消除了该样本中的原有发布锁竞争。两轮进程、交互方式和布局并非严格相同，不能把这个比较扩展成整体帧率的严格 A/B 结论。journal、journal discontinuity 和 app phase 可观测丢失均为 0；frame ring 的独立丢失计数不可用。

#### 三段完整区间及统一裁剪结果

完整区间分别为 profiler 时钟 **41638.0316–46218.9676 ms**、**46845.9919–53417.6475 ms**、**62554.7325–69340.3347 ms**。补充结果对每段统一裁去首尾各 250 ms，完整区间仍作为主记录。draw 按自身起点落入 `[start, end)` 统计；位置变化只比较保留区间内的相邻坐标样本。

| 区间 | profiler 时长（秒） | 位置变化次数／频率 | draw 次数／频率 | draw p50 / p95（ms） |
| --- | ---: | ---: | ---: | ---: |
| 第 1 段完整 | 4.580936 | 481／105.000 次/秒 | 461／100.634 次/秒 | 5.6770 / 7.0645 |
| 第 1 段首尾各裁 250 ms | 4.080936 | 445／109.129 次/秒 | 425／104.143 次/秒 | 5.6770 / 7.1155 |
| 第 2 段完整 | 6.571656 | 844／128.430 次/秒 | 836／127.213 次/秒 | 5.6100 / 6.4146 |
| 第 2 段首尾各裁 250 ms | 6.071656 | 801／131.963 次/秒 | 795／130.936 次/秒 | 5.6066 / 6.4055 |
| 第 3 段完整 | 6.785602 | 1017／149.876 次/秒 | 930／137.055 次/秒 | 4.7817 / 5.6265 |
| 第 3 段首尾各裁 250 ms | 6.285602 | 987／157.066 次/秒 | 900／143.184 次/秒 | 4.7633 / 5.5465 |

位置频率使用保留样本的实际首末跨度；三个裁剪区间的该跨度分别为 **4.077752 / 6.069861 / 6.283992 秒**，比对应 profiler 区间短约 3.2 / 1.8 / 1.6 ms。三个完整区间的最大采样间隔为 **2.345 / 2.738 / 2.227 ms**，没有此前数百毫秒采样停顿。位置不变的最长间隔分别为 **100.421 / 78.176 / 118.250 ms**，各自仅伴随一次、约 1–2 px 的鼠标变化，不能直接归为 UI 卡顿；第 1、3 段的这些停顿位于内部，统一裁剪仍保留它们。

三段窗口尺寸始终为 **3047 × 1736**，非空 `tab_count` 均为 **10**，所有 draw 的 `refresh_ms` 均为 **6.944444 ms**。窗口左上角范围依次为 x=64–626、y=135–236；x=-48–775、y=-31–414；x=8–708、y=42–426。采样没有 monitor ID、DPI 或活动文件标识，不能仅凭负坐标断定跨屏，也不能断言三段活动文件完全相同；这些数据未显示窗口缩放、标签数或刷新预算发生改变。

三个完整区间的平均 draw 耗时依次为 **5.790 / 5.669 / 4.872 ms**，其中 request_layout 为 **1.276 / 1.258 / 1.174 ms**，prepaint 为 **3.212 / 3.130 / 2.640 ms**，paint 为 **0.893 / 0.870 / 0.731 ms**。第三段每帧成本更低，裁剪后的平均 draw 仍为 4.854 ms；起止停顿不能单独解释这些每帧成本差异，当前记录不足以进一步归因。

`GetWindowRect` 位置变化可在同一显示周期内发生多次，第三段超过 144 次/秒的坐标变化不等于屏幕以该速率扫描；GPUI draw 和 CPU present 同样不是 scanout 证据。用户感受与锁探针支持这次修复带来改善，但所有三段并未稳定达到 144 次 draw/秒，**整体工作区稳定跟随当前屏幕最大刷新率仍未验收**。此处尚无普通 Release 的同场景数据。

#### 修复回归及诊断代码清理

主线随后完成两项修正测试夹具后的真实回归：`prepared_publication_preserves_early_retirement_and_disable_enable_epochs` 与 `cold_command_first_birth_preserves_its_original_command_epoch` 均通过，**2 passed / 0 failed / 0 ignored**，运行 193.25 秒，日志为 `target/window-drag-publication-regressions-fixed.log`。主线此前已在相同生产代码上通过真实 shortcuts 生命周期与 restricted 测试；此处不将未执行的 ignored 项计为通过。

临时诊断插桩已清理，本机源码备份位于 `C:/Users/Tmiracle/.codex/tmp/nanobug-drag-latency-20261023/diagnostic-source-before-cleanup`。构建／测试结果如下，现有编译警告仍存在：

| 检查 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo test --workspace --exclude editor-app` | 汇总 545 passed / 0 failed / 396 ignored | `target/window-drag-publication-workspace-tests.log` |
| 清理后 `cargo fmt --check` | 通过，无输出 | `target/window-drag-clean-fmt-check.log` |
| 清理后 `cargo check --workspace --offline -j1` | 通过 | `target/window-drag-clean-workspace-check.log` |

清理后的普通 Release 随后构建成功，日志为 `target/window-drag-clean-release-build.log`，构建耗时 1 分 58 秒，存在既有编译／链接警告。产物为 `target/window-drag-clean-release/release/editor-app.exe`，SHA256 为 `C9B901C6E9F6838F29D141A73D86F7C546E82446DDC78F78173405826D10E1B3`。构建前的 `target/window-drag-clean-release-metadata.json` 确认 workspace 为当前 checkout、独立最终输出为该目录、中间缓存仍为该 checkout 的 `target`。

准备切换程序时，用户按 Escape 停止电脑操作，未完成普通 Release 的启动替换及同场景真实拖动复测。以上行为回归、格式、编译和构建结果不能替代最终帧率验收。

返回 [UI 文档目录](../README.md)。
