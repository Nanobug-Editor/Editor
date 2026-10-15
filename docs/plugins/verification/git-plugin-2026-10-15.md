# Git 插件实施验证（2026-10-15）

状态：用户已批准保留已有进度，并将剩余工作按场景拆分后继续；已发布 #119/#120，当前执行 #119，尚未完整交付。暂停时现场见[历史快照](git-plugin-paused-2026-10-15.md)，后续结果按下面的候选身份分别记录。依据为[已确认规格](../specs/git-plugin.md)。补白已按用户最新要求重新纳入范围；本记录不把未实现的能力标记通过。

## 工单 01 当前候选门禁

实现 tree：`0f9ac5d2b88434191c7e82faaf286216c3afe9ef`（未包含下面文档记录的刷新）；比较点 `1379f58df76622bdf4d6a7c4167469f124d17a0b`。源码由 Git 临时 index 导出到 `target/git-ticket119-source/` 后验证，未借用其他任务的未提交文件。实际插件包仍为 0.1.1，SHA-256 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`。

| 导出目录中的命令 | 实际结果 | 日志（相对宿主仓库根） |
| --- | --- | --- |
| `cargo test -p editor-app aligned_editor_disjoint_native_viewports_use_independent_native_editing --target-dir .. -j1 -- --test-threads=1 --nocapture` | 1/1，0.26 秒；独立原生输入/Undo/真实滚轮和重新排列恢复通过 | `target/git-ticket119-disjoint-green.log` |
| `cargo test -p editor-app align --target-dir .. -j1 -- --include-ignored --test-threads=1` | 39/39，8.48 秒；13 项 ignored 已显式包含，未跳过 | `target/git-ticket119-alignment-final-gate.log` |
| `cargo test -p editor-app native_code_sync --target-dir .. -j1 -- --test-threads=1` | 4/4，0.28 秒 | `target/git-ticket119-legacy-final-gate.log` |
| `cargo test -p editor-app panel_visibility --target-dir .. -j1 -- --test-threads=1` | 3/3，0.27 秒 | `target/git-ticket119-visibility-final-gate.log` |
| `cargo test -p editor-app editor::comparison::diff --target-dir .. -j1 -- --test-threads=1` | 2/2，0.08 秒 | `target/git-ticket119-comparison-final-gate.log` |
| `cargo test -p editor-app extensions::windows_tests --target-dir .. -j1 -- --ignored --test-threads=1 --nocapture` | 同包 5/5，395.07 秒；显式执行 ignored | `target/git-ticket119-windows-final-gate.log` |
| `cargo fmt --check` | 通过 | `target/git-ticket119-final-fmt.log` |
| `cargo check --workspace --target-dir .. -j1` | 通过，3.26 秒 | `target/git-ticket119-final-check.log` |
| `cargo test --workspace --exclude editor-app --target-dir .. -j1` | 276 通过、272 ignored；复用同一非 UI 内容的隔离结果，此后只修改 UI 及文档 | `target/git-ticket119-isolated-workspace.log` |

定向 Cargo 命令也可从宿主根添加 `--manifest-path target/git-ticket119-source/Cargo.toml` 并改用 `--target-dir target`。该二进制完成一次构建后，重复过滤词直接运行 `target/debug/deps/editor_app-d4e1859b30e4cb6a.exe`；上表给出等价 Cargo 入口，日志不伪装为每项都重新编译。自动行为与阶段门禁全部通过，当前准备普通提交及固定点至 HEAD 的 Standards/Spec 审查；审查、推送和 issue 关闭尚未完成。

## 恢复实施与新工单（历史过程）

以下保留每轮失败、修复和通过记录；当前状态以上表为准，不能把中间候选或旧包的通过数累计为最终结果。

用户已批准[剩余工作场景拆分](../tickets/git-plugin-remaining/README.md)。已发布 [#119](https://github.com/Nanobug-Editor/Editor/issues/119) 和 [#120](https://github.com/Nanobug-Editor/Editor/issues/120)，读回 #120 被 #119 阻塞，标签均为 `ready-for-agent`。连接器创建议题返回 403、未创建议题；随后通过仓库已有 Git 凭据调用 REST 完成发布和原生依赖核对，凭据未写入文件或输出。

0.1.1 候选（SHA-256 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`）的 55 项访客与同包 17 项真实 Git 测试保留通过证据，见暂停现场和 `target/git-alignment-guest-tests.log`、`target/git-alignment-package-tests.log`。本轮未改访客；不借用旧 0.1.0 的窗口结果证明 0.1.1 已通过。

首次恢复修复将绘制期的父实体权威查询改为已准备的来源元数据；事件分发仍精确校验身份和 revision。补白 34/34、旧同步 3/3 复验通过，日志 `target/git-ticket119-alignment.log`、`target/git-ticket119-legacy-sync.log`。新的 5 窗口套件为 2 通过、3 失败（312.29 秒，`target/git-ticket119-work-windows.log`）；初始化借用 panic 已消失，但出现真实窄列与字形对齐问题，未宣称窗口验收通过。

随后确认：测试需要推进每个窗口的实际下一帧回调；软换行公共行在窄列中占两行，原锚点出屏，改为可见公共行后暴露真实 8px 内边距差异。生产形状的快速原生夹具在关闭行号时可点击，启用真实行号后 0.48 秒稳定复现首块按钮缺失（`target/git-ticket119-gutter-red.log`）。分栏未采用声明的 220px 初始文件区，结果列被双操作栏和行号栏挤空；各列字形起点不一致还造成物理行偏差。初始尺寸和字形起点已修复，并由下面后续回归验证。

采用声明的初始分栏尺寸并统一实际字形起点后，当时工作区的补白原生回归 37/37（6.02 秒，`target/git-ticket119-alignment-final.log`）、旧同步 3/3、面板可见性 3/3 通过。真实包 5 窗口复验中，三项主比较通过，两项冲突窗口持续工作未返回，已终止该自有测试进程；没有把未结束项计为通过。有限绘制诊断显示三栏原点、布局、滚动和 epoch 已稳定，但窗口仍反复绘制；90 次绘制断点在 53.37 秒内捕捉到 `App::flush_effects` 的脏窗口刷新循环（`target/git-ticket119-isolated-merge-backtrace.log`）。跨窗口通知转发已由独立双窗口原生回归修复。

本轮非 UI workspace 测试已通过（`target/git-ticket119-workspace-tests.log`）。为隔离其他任务已有变更，另以临时 index 导出 Git 必要实现闭包；补齐可见性依赖后该隔离候选的 workspace check 通过（`target/git-ticket119-isolated-check.log`）。此为中间候选，最终分栏/对齐与测试定稿后还须刷新、复验和审查，未提交或推送。

### 工单 01：隔离候选的后续回归

固定比较点为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`。使用独立临时 index `target/git-ticket119.index` 导出 `target/git-ticket119-source/`，只含 Git 功能所需的公开机制、消费者、测试与文档；真实 index 及其他主题的工作区改动保留。下列 Cargo 命令均在导出目录执行，目标目录为 `..`，不能以含其他未提交实现的工作区通过结果代替候选验证。

- 跨窗口读取同一 `EditorState` 的原生回归在修复前 8.97 秒内达到 257 次绘制上限，修复后 2.67 秒通过。局部 authority paint fence 只抑制同一轮已绘制几何的额外观察者转发；原生通知和后续输入、IME、私有状态通知仍保留。日志：`target/git-ticket119-cross-window-red.log`、`target/git-ticket119-cross-window-green.log`。
- 该隔离候选补白相关 38/38（8.70 秒）、旧同步 3/3（0.16 秒）、可见性 3/3（0.29 秒）通过，日志为 `target/git-ticket119-isolated-align.log`、`target/git-ticket119-isolated-native_code_sync.log`、`target/git-ticket119-isolated-panel_visibility.log`。之后追加旧版父实体借用与零可视交集回归，最终套件尚待重新汇总。
- `cargo fmt --check`、`cargo test --workspace --exclude editor-app --target-dir .. -j1`、`cargo check --workspace --target-dir .. -j1` 通过，分别见 `target/git-ticket119-isolated-fmt.log`、`target/git-ticket119-isolated-workspace.log`、`target/git-ticket119-isolated-check-final.log`。非 UI 测试共 276 通过、272 ignored；ignored 未执行，不能计入通过。后续只修改 UI 路径，仍须复查最终格式与编译。
- 新的旧版同步回归在 0.05 秒复现绘制期读取正在更新的父实体。几何测量和动画请求保留在绘制期，权威校验与滚动写入延后到外层更新结束；校验包含场景、成员代次和精确来源版本。最终 4/4 通过（0.33 秒，`target/git-ticket119-legacy-owner-final.log`）。两次中间实现触发绘制阶段断言，均已修正，未作为通过证据。
- 实际包窗口中间复验为 3 通过、2 项在原“连续八个静帧”条件下超时（`target/git-ticket119-isolated-windows.log`）。超时诊断中原生文档入口为空、场景稳定，仍有本地 Git 查询运行。夹具改为等待公开刷新按钮恢复可用，再要求两轮场景与文档入口稳定；未关闭定时器、放宽版本门禁或延长 30 秒等待上限。冲突逐块操作定向通过（100.74 秒，`target/git-ticket119-merge-ready-final.log`），随后同包全部 5 项通过，见当前门禁表。
- 零可视交集原生回归在 0.12 秒复现错误的 `Ready`，使两栏源码片段均被裁空（`target/git-ticket119-disjoint-red.log`）。改为独立原生降级后，真实滚轮暴露覆盖层 auto inset 落在正常流编辑器下方，引起错误坐标及反复重测；新增原生视口断言在 0.26 秒复现（`target/git-ticket119-disjoint-diagnostic.log`）。两处 full-pane canvas 固定左上角后，输入/Undo、独立滚轮、静止收敛和重新排列恢复全部通过（`target/git-ticket119-disjoint-green.log`）。不完整测量仍保持 Measuring，仅全部有效但没有共同高度的布局降级。
- 新等待器首次完整窗口回归在插件禁用后错误等待已撤回的刷新控件，场景为空、实例和进程均已退休。该自有测试在定位后终止，日志保留为 `target/git-ticket119-windows-withdrawal-fixture-failure.log`，未计通过；等待器现将完全退役视为静止，同时对活动实例继续检查公开 readiness、稳定场景、空原生入口和进程结束。

## 工作范围与现场

- 宿主：`C:/Projects/RustProjects/Editor`，开始时已有大量其他任务未提交改动。只将下面列出的 Git 插件及通用接缝计入本任务，不把整个工作区 diff 视为本次成果。
- 插件：`C:/Projects/RustProjects/Git-for-Nanobug`，独立源码目录，当前没有 `.git` 或远程。宿主提交不等于插件源码已推送；本批候选交付不创建远程插件仓库或发布版本。
- 补白候选：`target/git-plugin-final/git-0.1.1.zip`，SHA-256 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`。清单与访客版本同步升至 0.1.1，要求 `ui.code_editor >=1.1,<2`。通过重建宿主的 `--plugin-package` 直接构建，实际包验证进行中；包含摘要校验的 Windows 原生变基辅助程序，SDK 由宿主缓存注入。
- 旧基线：`target/git-plugin-final/git-0.1.0.zip`，SHA-256 `bae3594de6cd2d167a74144e3bd5d912b0e5ded7377979f009af767e1dee35f5`。先前的 5 项原生窗口结果属于此基线。此前 `git-plugin-candidate` / `git-plugin-next` / `git-plugin-delivery` 为迭代快照，分开记录结果。候选包需要配套宿主；不声称兼容未包含新能力的旧宿主。
- 审查固定点为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`，交付前审查 `git diff 1379f58df76622bdf4d6a7c4167469f124d17a0b...HEAD` 及对应提交列表。此前针对未提交现场的调查只属预检查，不算正式 Standards/Spec 审查通过。

## 实现与验证接缝

宿主新增的是面向全部插件的能力，不按 Git 插件 ID 分支：

| 能力 | 契约和实现 |
| --- | --- |
| Pipe EOF | `process` 1.8 的 `CloseStdin`；先排空已接受字节，再关闭输入管道，不终止子进程；PTY 拒绝 |
| 独立窗口 | `ui.windows` 1.0；声明 window 面板；借用唯一原生文档，关闭窗口保留草稿和 Undo，禁用回收窗口资源 |
| 持续比较 | `editor.virtual` / `editor.diff` 1.1；隐藏快照、文件绑定预览／固定标签、持续重算、统一视图、差异导航和滚动联动 |
| 原生代码列 | `ui.code_editor` 1.1；1.0 默认行为保留，1.1 通过 `Document.code_alignment` 显式启用本地补白；来源不成为第二份可编辑文档 |
| 块边缘操作 | `editor.actions` 1.0；现有文档几何定位按钮，操作发出时再次校验来源 revision，保留原生编辑状态 |
| 列联动 | 1.0 保留有界语义滚动；1.1 同组以不可变行槽与实际原生软换行几何形成共享显示视口、独立横向滚动，补白不进入文本；超过预算明确独立滚动 |
| 可见性 | `ui.visibility` 1.0；真实窗口、dock 折叠和 tab 选择采样；只通知已运行实例，不以采样启动插件 |

插件负责暂存／分支树、拓扑与分页历史、index 提交、精确 patch 暂存、仓库与 linked worktree 识别、网络操作、stash、高级操作预览、交互式变基计划和三方合并策略。命令使用可执行文件与参数数组；诊断遮蔽 URL userinfo；不保存凭据，不使用用户输入拼接 Shell。

## 历史 0.1.0 阶段的针对性验证

以下保留补白接入之前的实际执行记录，窗口结果属于 0.1.0，不能用于证明当前 0.1.1 窗口通过。后续修改必须复验相关项；被 ignored 的测试不计入普通测试通过数。0.1.1 的 55/17 项及本轮结果见上方恢复实施记录。

| 命令／入口 | 当前结果与范围 |
| --- | --- |
| `cargo test -p plugin-protocol workbench` | 6 通过；代码列／动作、补白 1.1 独立协商和序列化验证 |
| `cargo test -p plugin-runtime panel_visibility` | 3 通过；所有者、能力和冷实例边界 |
| `cargo test -p editor-app native_code_sync -- --test-threads=1` | 3 通过；真实三栏滚轮、来源选择／文本不变、场景撤销取消定位 |
| `cargo test -p editor-app panel_visibility -- --test-threads=1` | 3 通过；实际 dock 折叠／同组选择与隐藏 fallback、原生 close／重新显示／hide |
| `cargo test -p editor-app extensions::windows_tests -- --ignored --test-threads=1` | 旧 0.1.0 ZIP 的 5 项全部通过（335.11s，`target/git-work-windows-final.log`），涵盖下面三组行为；显式执行 ignored，并非普通 workspace 测试计数 |
| 主比较窗口的 3 个回归 | 原生输入／撤销、双击固定、隐藏快照、统一视图选择／只读／导航、40 次刷新与回收、长软换行双向滚动 |
| 独立窗口原始回归 | 真实 9 宽标记、选块与 Undo、中文 emoji、主窗口其他文件下 Ctrl+S、磁盘冲突明确覆盖、隐藏及隐藏后手动刷新保留草稿与 MERGE_HEAD |
| 新增独立窗口合并回归 | 真实按钮点击验证 base 初始化、逐块接受、Undo 保留非冲突修改、非冲突单独应用、手工编辑后明确确认和保存暂存；主窗口选中另一文件时仍操作正确原生文档 |
| 独立访客单元测试 | 最新 55 通过；含真实 Git LF index／CRLF 多块合并、SHA-1/SHA-256 intent-to-add、硬重置阻挡路径与禁止隐式 autostash，以及普通／linked worktree × merge／apply 两种 rebase 后端连续同文件冲突的身份与结束清理 |
| 实际 WASM 包 → Manager → Git | 16 项初轮为 14 通过、2 失败（890.25s，`target/git-package-test-final.log`）；通过项包括大提交说明与 updateRefs 保护的交互式变基、部分暂存／重命名／未出生 HEAD、二进制 add/add、7 项真实仓库／Stash／Pull、3 项 index 边界。失败的两项变基恢复已定位并修复临时只读 base 栏范围错误，定向复验 2 通过（102.31s，`target/git-rebase-resume-final.log`） |
| 新增远程变化 force-with-lease 包回归 | 1 通过（47.90s，`target/git-lease-final.log`）；预览后远程推进，确认时旧 lease 被拒绝，没有覆盖新的远程提交 |

补充修复包括：Edited 回执不能将较新文档版本降回旧版本；快照在连续输入中失效时重新枚举同一文档身份并读回最新版本，关闭／重新打开不借用旧身份；冲突文件建议按完整状态与稳定操作身份统一发布，rebase 步骤使用稳定 orig-head／onto 身份，未知／准备中暂停写入口，新操作撤销旧建议但保留宿主文档和 Undo。已解决文件仍可核对，不再启用旧 base／块／整版本写入。linked worktree 的 rebase 元数据通过经过校验的 `worktrees/<id>/...` Git 命名空间读取；成功结束后 Git 遗留的 `REBASE_HEAD` 不单独表示仍有进行中的 rebase。

新交互式变基包回归的早期缺失视图索引，不是 helper 的继续协议失败：临时中栏用 base 文本却错配 right ranges，导致公开 UI 校验撤回实例。新 guest 回归已红→绿。定向实际包回归覆盖原生文档请求 ApplyEdits／保存／暂存后继续，以及恢复计划被插入 exec 时拒绝执行并保留草稿与 Git 状态。上述两项与新增 lease 用例使用最后一次 linked 元数据修复前的 final 快照（SHA-256 `7423c1e7471d1abf12f43a72087806f7da4f55d13171d9f944c908322c691522`）；不能把不同快照的分批结果冒充同一 ZIP 全量 17 项通过。此历史阶段的 55 项访客和 5 项原生窗口通过；后续 0.1.1 同包 17 项通过另行记录。

可见性初次 fixture 假设面板默认在同一 tab 组而失败。Nanobug 实际默认拆分 leaf；修正 fixture 通过公开 DockArea API 明确合并测试 peers 后，3 项通过。没有为迁就测试改变产品布局。

合并窗口夹具此前遗漏生产 actor 的文档事件转发，且仅按 Git 进程数判定空闲，可能在原生场景追上 Manager 之前点击旧按钮。夹具现将真实文档事件送回 Manager，并要求无文档事件且场景 revision 连续稳定后才结束泵送；点击前核对面板与 Manager revision。按钮点击定位到实际 Base 控件，避免点中包装容器的空白。未直接注入业务事件，也未降低生产来源版本校验。

## 补白实现与验证进展

用户允许在本地 `ui/` 新建自定义组件，但要求不复制或 fork gpui-kit、gpui-base 等上游。此授权已采纳，不再沿用此前“补白延后”的范围结论。

当前锁定的 gpui-base 0.7.0 未公开直接修改 display map 的接口。现已验证另一条公开组合路径：本地组件裁剪原生显示片段，最终原生挂载保留原 `EditorState` 的输入、选择、IME 与 Undo；根级 RootPlugin 适配真实指针事件，InputHandler 只适配几何。补白只进入不可变行映射与像素布局，不写入源文本，不建立第二份可变 Rope 或撤销栈。

一次性 public-API 试验位于本地忽略目录 `target/git-spacing-probe`，没有复制上游实现。运行：

```powershell
cargo test --manifest-path target/git-spacing-probe/Cargo.toml --target-dir target --offline -- --nocapture --test-threads=1
```

实际 3 通过（包含 2 个原生布局试验）：

1. Inline token 拒绝空源范围和换行；CodeEditor 模式丢弃 token，不能借此产生显示补白。
2. 同一 EditorState 分段裁剪、重复挂载后，只有最后一处 input/caret/IME 几何生效；试验测得前一挂载被后一挂载覆盖。不能作为可编辑补白实现。
3. 额外 compile-fail feature `missing-display-seam` 访问 `EditorState.display_map` 得到预期 E0616 私有字段错误；仅说明此路径不公开。

这些早期试验仅否定未经输入适配的简单裁剪，不能据此认定公开组合不可行。未接入的 `src/single_mount.rs` 未运行，不计入实测。

后续 `forward_mouse.rs`、`aligned_bridge.rs` 和 `aligned_scroll.rs` 已通过公开 API 验证：延迟原生指针转发、根级恢复事件隔离、真实光标及 IME 候选框、中文 emoji 预编辑／提交／Undo、跨补白拖选、3 处不同高度补白（含大于视口的空白）、第 22 行点击、长软换行、滚轮与翻页。`aligned_scroll` 最近 2 项通过（`target/git-spacing-scroll-probe.log`），另验证稳定绘制后没有持续通知或动画帧。

生产实现位于 `ui/controls/aligned_rows`、`aligned_pointer`、`aligned_editor`，接入主比较和插件代码组。`ui.code_editor` 1.1 新增 `Document.code_alignment` 显式启用，1.0 默认行为保留；4M 单元由同场景组共享，输入时重算归还旧预算，不为每个组另发 4M。布局受每源 1 MiB／2001 行、测量高度上限约束，失败明确独立滚动；搜索或代码操作打开时整组暂停并显示提示。生产接入后的全量验证仍在进行，下面旧候选包结果不包含此能力。

## 历史门禁与仍有效的交付限制

此前 `cargo test --workspace --exclude editor-app` 首次遇到 Windows `msvcrt.lib` 链接失败；同环境以 `-j1` 重试全量通过。补白接入之前的阶段门禁 `cargo test --workspace --exclude editor-app -j1`、`cargo check --workspace`、`cargo fmt --check`、`git diff --check` 均通过；不代表本轮隔离候选的最终门禁。普通 workspace 测试未执行 ignored 项。原生辅助程序 `cargo test --manifest-path ../Git-for-Nanobug/native/Cargo.toml` 编译通过但包含 0 项测试，不把它当成行为覆盖；辅助程序行为通过实际包变基用例验证。

当前尚不能宣称全部功能完成：补白生产接入后的集成复验尚未结束。真实 WASM 包验证按上述快照分批执行，旧候选 ZIP 的 5 项原生比较／合并测试全部通过；没有将分批结果伪装为新 ZIP 的全量包测试。当前没有生产界面截图验收；GPUI 测试覆盖真实布局和输入，但不替代维护者对深浅主题、缩放和整体外观的验收。macOS/Linux 未构建或实测，原生变基资产当前声明 Windows x86-64。

外部 Git 可并发，内部队列并非整个仓库的独占事务。尤其 `stash drop` 的 CLI 没有 reflog CAS：执行前核对 immutable stash OID 与 selector，可以拒绝已观察到的变化，但不能声称消除核对与删除之间所有外部竞争。
