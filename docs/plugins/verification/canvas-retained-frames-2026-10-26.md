# Canvas 04：保留场景与显示帧验收记录

状态：本页保留前期源码、固定包及旧显示环境的验证历史；最新统一 Editor 集成、修复与验收进展见[2026-10-27 记录](canvas-retained-integration-2026-10-27.md)。完整十分钟实机采样仍未完成，AC6/AC7 未验收，#131/#127 保持开放，尚未合并 main。依据为[工单 04](../tickets/canvas-protocol-extension/04-retained-frames.md)和[扩展方案](../specs/canvas-protocol-extension.md)。

## 工作边界

沿用独立工作区 `C:/Users/Tmiracle/.codex/worktrees/canvas-prerequisite/Editor`、分支 `codex/canvas-protocol-extension`。第三单及之前的交付基点为 `b5a16f57deed306df20a3376a41f42fb7e146d6e`，已核对 origin；#128/#129/#130 已关闭。固定审查点继续为 `561aa5b2006c38668a083482156646a16cd02b3e`，另以本单基点排除前三单既有改动。全部四单验收前不合并 main。

实现阶段分别完成原子发布与实际时间轴消费者、绘制/文字/纹理缓存、双轴滚动与统一逆映射、可见帧请求与有界队列。真实窗口和输入法测试由主执行者串行调度；源码已冻结并通过下述门禁及双轴审查。并行过程的候选测试与最终源码/固定包验证分开记录，完整实机长测仍未完成。

## 原子补丁第一切片

- `target/canvas-d-patch-red.log` 复现公开 `CanvasScenePatch` 缺失；绿测验证替换一个绘制项、原输入不变、scene revision 原子递增及实际旧/新脏区联合。
- `target/canvas-d-patch-damage-red.log` 捕获未变化的大图形被计入单项脏区（1200×1300）；改为仅遍历变化子树的祖先变换后，`target/canvas-d-patch-damage-green.log` 通过（期望 70×30）。提示矩形只能扩大，不能缩减实际脏区。
- `target/canvas-d-patch-remove-red.log` 复现缺少结构删除；`target/canvas-d-patch-remove-green.log` 为 2 通过，包含父组删除后关联目标撤销及原场景不变。
- `target/canvas-d-patch-output-red.log` 复现公开 Output 缺少场景补丁；`target/canvas-d-patch-output-green.log` 为 3 通过，明确 outer UI 与 scene 的两个基础版本，重复应用返回 StaleRevision，不能借场景补丁伪造后续全量 View 的基础。

这组最初的协议测试只证明纯数据接缝，后续实际管理器、WASM 和原生集成结果单独记录在下文；不能据协议局部绿测宣称 #131 完成。

## 实际时间轴消费者

独立 `canvas-timeline` 只依赖宿主内嵌 SDK，逻辑数据为固定种子 1729、1000 行×100 列，共 100000 项。数据按逻辑索引确定，客体仅保留稀疏编辑，并只生成视口及 overscan 邻域；单项修改走绘制项补丁。初始 `1.0.0` 保留为历史故障输入；当前固定版本为 `1.0.1`，其源码、SDK 和 ZIP 摘要见下文。

Windows 参考机、Release/profile、刷新率、DPI 和短时实际帧/输入采样已记录在下文；仍缺完整 10 分钟资源、字节、塑形/栅格和队列原始采样。已有 C 的 Release 依赖预编译仅节省后续构建时间，不是本单性能验收。

## 集成候选的定向验证（历史过程）

以下日志属于当时的工作区候选源码及候选 ZIP，不是最终已提交产物的验收结论；冻结后的 SDK、包摘要和相关回归另列在最终快照段。

- `canvas-d-protocol-combined.log`：公开协议共 119 通过（91 单元、28 集成）；随后 `canvas-d-publication-regressions.log` 的保留补丁 9 通过，包含父组移动/裁剪后关联元数据的实际脏区。
- `canvas-d-manager-labelled.log`：实际时间轴包经公开 Manager 的 2 项通过（48.05 秒），覆盖局部补丁、陈旧/未知引用/ABA 原子拒绝、显式重同步、单个待发帧、隐藏暂停及业务定时继续。这里直接调用原生帧 lease 的管理器边界测试不代替 GPUI 帧验证。
- `canvas-d-lifecycle-actual.log`：实际包的 2 项生命周期测试通过（95.97 秒），覆盖禁用/启用、准备失败回滚、正常更新切换、逻辑快照与旧 lease 撤销。
- `canvas-d-mailbox-final-green.log`：9 项有界邮箱测试通过，包含字节预算、终结输入保留、显式取消及 shutdown 独立槽。
- `canvas-d-paragraph-cache-green-final.log`：真实原生文字缓存测试通过（2.03 秒），核对平移复用，以及字号、字体、真实 DPI 消息失效和文字位置映射。
- `canvas-d-scroll-native-final.log`：真实 Windows 双轴滚动/坐标测试通过（1.83 秒）。
- 实际 Flow D 的 B 编辑链回归通过（99.80 秒）；Table D 的 C 交换 5 项通过（135.04 秒）。冻结的 Table B 旧包回归另行通过（37.69 秒）；不能把 D 包新增单元格后的历史数量先验差异当成旧包回归通过。
- `canvas-d-three-consumers-actual.log`：同一 Manager 同时安装 Flow、Table、Timeline 的组合测试通过（220.72 秒）；相同 Canvas 节点名不共享帧或剪贴板资源，Flow 与 Timeline 局部更新不替换 Table 的已接纳 Arc，停用 Flow 只撤销自身 lease。
- `canvas-d-terminal-regression.log`：终端 46 通过、2 ignored（14.20 秒）；`canvas-d-plugin-regression.log`：通用插件 UI 84 通过、22 ignored（2.11 秒）。ignored 项均未计入通过。
- `canvas-d-transfer-regression.log`：Flow D 的交换回归 10 通过、2 项在 Debug 包激活阶段超时；并行编译期间的串行 Debug 重试也未越过激活。限定 Release 入口低并发重跑两项，`canvas-d-transfer-two-release.log` 为 2 通过（19.83 秒），实际进入并通过原拖出与嵌套所有者撤销断言，没有改等待期限。
- `canvas-d-paragraph-release.log`、`canvas-d-scroll-release.log`：候选 Release 原生文字缓存和双轴滚动各 1 通过（1.88 / 1.25 秒）。

实际 WASM 预算探针最初在序列化大路径时先耗尽访客 fuel，未到宿主预算接纳；改为一次序列化小型 typed Completion 并批量复制固定合法路径 JSON 后，没有提高生产 fuel 或弱化预算断言。`canvas-d-hybrid-budget-final.log` 为 1 通过（107.09 秒），实际到达宿主编码/展开预算层并验证旧 Arc 保持不变。编译负载下另一轮在 demand 阶段超时，原失败日志继续保留。

原生时间轴早期隐藏后恢复出现停滞或 `StaleRevision: Canvas coordinates have no current native presentation`，失败日志 `canvas-d-timeline-native-resume.log`、`canvas-d-timeline-native-envelope.log` 保留。最终修正区分正常生产初始化与平台可见性转换：Root 创建前调用 `editor_ui::init` 注册滚轮路由；用公开最小化/激活恢复产生真实 WM_SIZE，不直接修改可见性缓存；恢复仅刷新一次。生产接缝使失效代次的回调不占据新请求，被拒绝的真实呈现回执在下一次真实绘制重发，撤回源时不自行重绘。

`canvas-d-timeline-source-final.log`：第一份源码交付前，候选包的 Release 原生完整 smoke 为 1 通过（5.95 秒），包含可见帧、隐藏暂停且业务定时继续、恢复续调、停止后 300ms 零新增 draw、真实 SendInput 双轴滚动及 GPU 像素变化；只准备约 262 项缓存而非 100000 项逻辑数据。`canvas-d-plugin-source-final.log` 为 85 通过、22 ignored（2.23 秒），`canvas-d-mailbox-source-final.log` 为 10 通过（0.48 秒）。这些是后续版本竞争与运动寿命修复前的历史证据，当前源码及固定包结果见最终快照段。

## 候选双轴审查

固定 tree `f36374280dcc50c708d408b3b3b3a819ba176912` 与本单基点比较，另核对用户固定点 `561aa5b2`：Standards 0 项确认的 P1/P2；Spec 1 项 P2：新 scene identity 复用旧 scroll epoch 时未更换原生滚动 Rc，旧惯性可能移动新内容。随后补充同 epoch/extent 替换场景回归并修复；后续 tree 均增量补查，未直接沿用此前零发现结论。

补审 tree `925085da00e71d41a24a31daf93be7c4a6f22d5e`：Standards 新增 0，Spec 确认原 P2 在代码层解决、新增 0。`deactivate()` 统一更换滚动 Rc，原生 motion ID 包含 scene identity；`canvas-d-scroll-identity-release-green.log` 实际 5 通过（0.01 秒），其中 CanvasView 级回归验证同 epoch/content 的新 identity 不受旧 Rc 晚到移动影响。最初新测试因遗漏 RuntimeStyles 初始化而失败，该轮不是行为红测；补正常主题初始化后通过。

本轮仓库门禁：`canvas-d-format-final.log` 格式检查通过；`canvas-d-workspace-tests.log` 为 574 通过、0 失败、413 ignored；`canvas-d-workspace-check.log` 编译检查通过（19.21 秒）。这些结果覆盖当时工作区源码，随后原生 smoke 的公开最小化/恢复调整及最终产物仍需单独核对。

首次 `cargo test -p editor-app --release --no-run --offline` 编译应用测试成功，但整体命令因历史 `svg_preview_render` example 未接入已有 Canvas 模块、且未覆盖 `Paint::TextBox` 而失败；已核对本单基点也存在对应缺口。限定实际应用目标重跑 `cargo test -p editor-app --release --bin editor-app --no-run --offline` 成功。该结果仅为 Release 测试二进制准备，不是性能结果。

## 实机采样条件

本机只读 CIM 查询：AMD Ryzen 9 9950X3D（16 核 / 32 逻辑处理器），NVIDIA GeForce RTX 5080，显示驱动 `32.0.16.1088`；Windows 11 专业版 `10.0.26200`，可见内存 64545140 KiB；显示模式为 3840×2160、144 Hz。实际窗口 DPI 和每帧刷新间隔由原生采样记录，不能把方案的 60 Hz 建议条件写成本机实测条件。

同机 full / patch 比较使用相同保留缓存、相同普通时间轴包、种子与真实双轴 SendInput 往返轨迹；full 指现行宿主上的全量视口发布对照，不冒称历史旧二进制。每秒记录工作集、private commit、当前/仍存活派生像素、资源、累计字节、塑形/栅格工作及有界 fixture ingress；该 ingress 不是生产 WorkSender 邮箱。每帧保存 GPUI 实际 draw/present trace，HDR 输入延迟只涵盖窗口派发至绘制，未包含硬件输入延迟。观察器也参与资源开销，两轮使用相同设置；原生 GPU atlas 内存未直接测量。

## 第一次源码与固定包

普通源码提交 `a8bf1974e34dc08a19a93ae3fce8984cc7913df8` 与最终补审 tree `8f37ea4eb7b6ae8140408731cc4d351f1abf9528` 相同；两轴未解决项均为 0。生产宿主 SHA-256 为 `7e19f8e23c5be1d3db43f47c5e26c50add2cbe55ed75f36706e6eea828b092fe`；提交后同一 Cargo 入口确认通过。SDK 为 `24727749b6873d0323751868262e106b7e8446582f1d0db826cfdea45e7096b5`，92 个源码/清单/WIT/JSON 输入、无 Markdown。三个独立 guest 都核对了 metadata workspace_root/target_directory，并通过该宿主 `--plugin-cargo`、各自输出目录直接构建。

固定 Flow 1.3.0、Table 1.2.0 的最终 Release 实际包回归：`canvas-d-frozen-hybrid.log` 4 通过（29.90 秒）、`canvas-d-frozen-editable.log` 1 通过（6.90 秒）、`canvas-d-frozen-table.log` 5 通过（28.05 秒，排除另用历史包验证的 B 六格先验）、`canvas-d-frozen-transfer.log` 12 通过（95.81 秒）。`canvas-d-frozen-native-*.log` 的 8 项均通过，包含实际 D 包 125% 深色流程图、200% 表格的中文 IME/UIA，段落缓存、双轴坐标、系统剪贴板及基本/透明图层/图片像素。

Timeline 1.0.0 的首个固定输入保留为历史故障样本：`canvas-d-baseline-full-30s.log` 在 5.624 秒因边界 legacy Wheel 与动画版本竞争失败，没有完成 30 秒，不作为性能通过。随后 Timeline 升为 1.0.1，声明稳定 `timeline-surface` 目标并关联 Grid 语义，typed 输入不重复执行原生滚动。`canvas-d-v101-target-wheel.log` 的真实 Manager 回归 1 通过（3.99 秒），旧 shown-target 在纯绘制推进后仍可接纳、旧 legacy 仍拒绝、identity 换代撤销旧 proof。

初次 1.0.1 验证还修正了两个测试/声明错误：Grid 关联 target 后移除互斥 rect；Manager 原生 ScenePresented 使用与真实 PluginView 一致的 ViewportInput 外壳。对应失败日志保留。`canvas-d-v101-fixed-smoke.log` 实际原生通过（6.31 秒）；`canvas-d-v101-retained.log` 原有生命周期/三包/补丁/帧的 5 项通过，新增目标用例修正外壳后单独通过。

`canvas-d-v101-baseline-full-30s.log` 又在 5.617 秒捕获迟到 ScenePresented（UI 392）与已接纳 UI 393 的竞争，不能作为 30 秒基线。已保存原始采样，随后补充通用有界已接纳/已呈现版本处理；没有忽略 StaleRevision、合成呈现证明或扩大 legacy 输入例外。

回执竞争的公开 Manager RED 为 `canvas-d-late-presented-red.log`（3.51 秒，真实复现 `UI document was replaced`）。运行时新增全实例最多四份旧 Document Arc，仅成功 ScenePresented 可按原快照校验并登记原目标；source/file/modal/overlay lineage/identity 撤销时永久裁剪历史资格。没有历史、已淘汰、未来、伪造投影和旧失败回执均拒绝，旧失败不能撤销较新已显示证明。`canvas-d-late-presented-final-green.log` 为 2 通过（7.83 秒）；`canvas-d-history-lifetimes-unit.log` 为 5 通过，直接覆盖 source/file/modal/overlay ABA 与旧失败隔离。补审 `cf2098a1` 的 Standards / Spec 新增发现均为 0。

`canvas-d-history-functional-full-30s.log` 已实际跑完 30.011 秒且没有上述 Manager 版本错误，但实际双轴往返检查失败（两轴增加/减少均 false），因此不计性能通过。诊断确认隐藏后的按钮 Visibility 凭据会随普通 UI 重绘更新；此前把滚动 Rc 和 kinetic ID 绑定它，导致每个动画更新重置运动。随后分离纯绘制与真正场景/来源/可见性撤销的寿命；这份失败采样继续保留，未因帧耗时较低而跳过实际移动门槛。

## 最终源码、固定包与功能门禁

普通修复提交 `bd5d920f38c88fc60ac722b4989c96ccbcf66bd6` 的 tree 为 `cb49ef64bca3953af4146271c74f3a00905232fb`。最终增量 Standards / Spec 均为 0 项未解决发现，沿用用户固定点并另核对本单基点。滚动以独立原生 incarnation 保持纯绘制期间的运动；目标输入另有只在真正撤销时换代的 lifetime，保留无新 draw 间隙的 KeyUp 和画布外 PointerUp。同一呈现回执继续引用原 UI revision，不能凭新按钮票据伪造更新的显示证明。

最终门禁与实际包结果如下；ignored 只记录跳过，不计通过：

| 验证 | 结果与本地日志 |
| --- | --- |
| `cargo fmt --check` | 通过；`canvas-d-pre-perf-format.log` |
| `cargo test --workspace --exclude editor-app` | 579 通过、0 失败、416 ignored；`canvas-d-pre-perf-workspace.log` |
| `cargo check --workspace` | 通过；源码冻结后再次以 `--offline --locked -j 4` 核对，3.66 秒；`canvas-d-artifact-final-check.log`（先前记录为 `canvas-d-pre-perf-check.log`） |
| Debug 通用插件 UI | 87 通过、22 ignored，2.64 秒；`canvas-d-final-plugin-regression.log` |
| 有界生产邮箱 | 10 通过，0.47 秒；`canvas-d-final-mailbox-regression.log` |
| SDK 导出 | 6 通过，0.26 秒；`canvas-d-final-sdk-regression.log` |
| Release 双轴运动及 identity 换代 | 6 通过，0.01 秒；`canvas-d-accepted-motion.log` |
| Release 同回执、无 draw 间隙的目标输入 | 1 通过，0.01 秒；`canvas-d-accepted-release.log` |
| 实际包迟到呈现证明 | 2 通过，7.18 秒；`canvas-d-history-final-presented.log` |
| 实际包补丁、帧、恢复、更新及三包隔离 | 6 通过，41.16 秒；`canvas-d-history-final-retained.log` |
| Flow/Table 混合局部发布及编码/展开预算 | 4 通过，28.47 秒；`canvas-d-history-final-hybrid.log` |
| Flow 编辑链 | 1 通过，7.71 秒；`canvas-d-history-final-editable.log` |
| Table C 交换 | 5 通过，25.79 秒，历史 B 六格先验另用原包验证；`canvas-d-history-final-table.log` |
| Flow 交换 | 12 通过，101.36 秒；`canvas-d-history-final-transfer.log` |

固定包的原生 8 项回归结果见上面的 `canvas-d-frozen-native-*.log`，后续源码针对运动、迟到回执及终结输入补充了上述回归。最终 Release 完整原生 smoke 随下述 full 30 秒运行再次通过：实际可见帧、最小化暂停/业务定时继续、恢复续调、停止后零新增 draw、真实双轴输入及像素变化均进入断言。

额外运行整个 Release 虚拟 UI snapshot 过滤集得到 **58 通过、29 失败、22 ignored**（`canvas-d-accepted-plugin.log`）；失败集中在测试元素/几何 snapshot 缺失，不能算通过。使用修复前 `a8bf1974` 的 Release 测试二进制单独运行 `checkbox_press_cannot_activate_a_replacement_scene`，同样因元素查询 `Option::unwrap()` 失败（`canvas-d-old-release-snapshot-control.log`）；当前 Debug 全集 87 项通过。该对照只证明这个代表用例在旧二进制也失败，不声称逐一证明 29 项同根因；此处记录为额外 Release 虚拟 snapshot 限制，不扩大本轮修复范围，也不替代真实原生验收。

`a8bf1974` 至 `bd5d920f` 的公开协议与 SDK 导出源码无差异，三包继续使用上述 `24727749…96b5` SDK 和 `7e19f8e2…92fe` 生产宿主构建入口；修复后的实际运行时/原生测试二进制来自 `bd5d920f` 源码。Timeline `1.0.1` 的源码提交为 `bd5d920f`，其宿主构建来源仍明确登记为 `a8bf1974`，不将构建来源改写成新提交。已重核 ZIP SHA-256：

| 固定输入 | SHA-256 |
| --- | --- |
| Flow `1.3.0` | `7b7fc0852b5eb7bc92c690ebbb3fe442942ff49cf91c90b592bb73971237358d` |
| Table `1.2.0` | `2bb37201415b752cb02e1424c286019fd70e4af9b982447a5f03ccb466c96346` |
| Timeline `1.0.1` | `ee6132d9724d9c308b43be05bfcb4b4e2e0345f608af16bd8db19b96ca2d57cb` |

独立包的完整构建来源和源码摘要登记在 `crates/plugin-runtime/tests/fixtures/packages/inputs.json`，历史包条目及字节保持不变。新增条目明确源码目录与摘要规则：按相对路径排序，以 UTF-8 正斜杠路径、NUL、对应提交的原始 Git blob 字节计算，不混入 checkout 的 CRLF 转换。构建宿主与验证宿主分别登记；时间轴实机测试 EXE SHA-256 为 `100dbb72dfc99d115e455e3af89dc747bb6fb493bef6d3985bda53bf050828c9`。

## 完成的 30 秒 full 对照与中断的长测

最终源码、固定 Timeline `1.0.1`、Release、种子 1729、窗口 DPI 150% 下，full 对照完成 **30.001 秒、75 次真实双轴滚轮输入**，包含实际往返检查（`canvas-d-final-full-30s.log`，整项原生测试 1 通过，36.51 秒）。这是当前宿主上全量视口发布路径的对照，不是历史旧宿主二进制；后续 patch 比较必须保留相同条件。

| 30 秒 full 实测 | 数值 |
| --- | --- |
| 实际 draw / present | 各 4292 次，约 143.06 次/秒；trace 与 HDR 计数一致 |
| CPU draw p95 / p99 | 0.857 / 1.135 ms |
| present interval p95 / p99 | 7.623 / 8.045 ms |
| dirty-to-present p95 / p99 | 7.565 / 7.999 ms |
| 窗口派发到绘制的输入反馈 p95 / p99 | 6.955 / 7.381 ms，74 个样本 |
| 实际 Canvas 机会 / 发布字节 | 1912 次 / 145416550 bytes；字节记录完整 |
| 最终 / 采样峰值工作集 | 133599232 / 135000064 bytes |
| 最终 / 采样峰值 private commit | 127033344 / 128610304 bytes |
| 缓存项最终 / 峰值、原生顶点 | 320 / 349 项、2022 顶点 |
| 原生塑形累计值与起点 / 栅格累计 | 1051 与 72 / 0 |
| 实例资源 / fixture ingress 峰值 / overflow | 2 / 19 / 0 |

本机为实际 144 Hz、DPI 150%，不能声称在方案的 60 Hz 条件下采样；CPU draw、呈现间隔、脏区反馈和输入反馈分别报告。上述分位低于建议阈值，但短时对照不能证明十分钟资源稳态，也未直接测量 GPU atlas 内存。观察环发生淘汰，记录器已持续排出并核对发布计数、字节及 frame trace 完整，未把淘汰数写成数据缺失。

两次请求 600 秒的 patch 采样分别在 **20.014 秒、40.002 秒** 因 `timeline input window lost foreground` 中止（49 / 99 次真实滚轮输入）。对应失败日志和退出前原始采样均保留；两次短记录不能相加为长测，更不能报告 600 秒通过。用户当前需要使用桌面，本轮不再启动 GUI 长采样；待桌面可用后，仍需一次完整 600 秒往返滚动、资源稳态和同条件性能比较，再决定 AC6/AC7 及整批关闭。

原始 header 也保留预热差异：两轮 patch 的起始 offset 均为 `[168,168]`，但 overscan 的小数边缘及第二次尝试的预热缓存计数略有差异。后续完整比较需核对这些起始记录，不能把中断样本描述成严格相同预热状态的性能对照。后续运行沿用 `native_canvas_timeline_real_frames_scroll_and_idle --ignored --nocapture --test-threads=1`，设置 `NANOBUG_CANVAS_TIMELINE_MODE=patch`、`NANOBUG_CANVAS_TIMELINE_SECONDS=600`、固定 `1.0.1` 包和新的 `NANOBUG_CANVAS_TIMELINE_RAW` 文件；以真实前台窗口连续完成采样，不关闭输入归属检查。

便携证据与结构化摘要：

- [full 30 秒原始 JSONL（gzip）](canvas-retained-frames-2026-10-26/full-30s.jsonl.gz)、[运行日志](canvas-retained-frames-2026-10-26/full-30s.txt)。
- [patch 20 秒中断原始 JSONL（gzip）](canvas-retained-frames-2026-10-26/patch-20s-interrupted.jsonl.gz)、[失败日志](canvas-retained-frames-2026-10-26/patch-20s-interrupted.txt)。
- [patch 40 秒中断原始 JSONL（gzip）](canvas-retained-frames-2026-10-26/patch-40s-interrupted.jsonl.gz)、[失败日志](canvas-retained-frames-2026-10-26/patch-40s-interrupted.txt)。
- [采样摘要](canvas-retained-frames-2026-10-26/sampling-summary.json)。

截至本记录，源码和固定包已完成，AC6/AC7 因完整十分钟实机采样缺失仍未验收，#131 保持开放，任务分支未合并 main。
