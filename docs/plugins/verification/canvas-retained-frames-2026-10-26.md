# Canvas 04：保留场景与显示帧验收记录

状态：实施中，AC1–AC8 均未验收；[#131](https://github.com/Nanobug-Editor/Editor/issues/131) 保持开放。依据为[工单 04](../tickets/canvas-protocol-extension/04-retained-frames.md)和[扩展方案](../specs/canvas-protocol-extension.md)。

## 工作边界

沿用独立工作区 `C:/Users/Tmiracle/.codex/worktrees/canvas-prerequisite/Editor`、分支 `codex/canvas-protocol-extension`。第三单及之前的交付基点为 `b5a16f57deed306df20a3376a41f42fb7e146d6e`，已核对 origin；#128/#129/#130 已关闭。固定审查点继续为 `561aa5b2006c38668a083482156646a16cd02b3e`，另以本单基点排除前三单既有改动。全部四单验收前不合并 main。

当前并行职责为原子发布与实际时间轴消费者、绘制/文字/纹理缓存、双轴滚动与统一逆映射、可见帧请求与有界队列。共享协议接缝先明确命名，真实窗口和输入法测试由主执行者串行调度；最终统一源码冻结、仓库门禁、实际包及原生长测、双轴审查。并行过程的局部测试不代替最终快照验证。

## 原子补丁第一切片

- `target/canvas-d-patch-red.log` 复现公开 `CanvasScenePatch` 缺失；绿测验证替换一个绘制项、原输入不变、scene revision 原子递增及实际旧/新脏区联合。
- `target/canvas-d-patch-damage-red.log` 捕获未变化的大图形被计入单项脏区（1200×1300）；改为仅遍历变化子树的祖先变换后，`target/canvas-d-patch-damage-green.log` 通过（期望 70×30）。提示矩形只能扩大，不能缩减实际脏区。
- `target/canvas-d-patch-remove-red.log` 复现缺少结构删除；`target/canvas-d-patch-remove-green.log` 为 2 通过，包含父组删除后关联目标撤销及原场景不变。
- `target/canvas-d-patch-output-red.log` 复现公开 Output 缺少场景补丁；`target/canvas-d-patch-output-green.log` 为 3 通过，明确 outer UI 与 scene 的两个基础版本，重复应用返回 StaleRevision，不能借场景补丁伪造后续全量 View 的基础。

这组最初的协议测试只证明纯数据接缝，后续实际管理器、WASM 和原生集成结果单独记录在下文；不能据协议局部绿测宣称 #131 完成。

## 实际时间轴消费者

新增独立 `canvas-timeline 1.0.0` 源码，只依赖宿主内嵌 SDK，逻辑数据为固定种子 1729、1000 行×100 列，共 100000 项。数据按逻辑索引确定，客体仅保留稀疏编辑，并只生成视口及 overscan 邻域；单项修改走绘制项补丁。候选已完成 SDK 构建和真实 Manager 接纳；最终固定 ZIP 尚未冻结，不登记为已交付输入。

最终还必须保存 Windows 参考机、Release/profile、刷新率、DPI、实际帧/输入延迟及 10 分钟资源、字节、塑形/栅格和队列原始采样。已有 C 的 Release 依赖预编译仅节省后续构建时间，不是本单性能验收。

## 集成候选的定向验证（尚未冻结）

以下日志属于工作区候选源码及候选 ZIP，不是最终已提交产物的验收结论；最终冻结后仍需核对 SDK、包摘要和相关回归。

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

`canvas-d-timeline-source-final.log`：候选包与最终源码的 Release 原生完整 smoke 为 1 通过（5.95 秒），包含可见帧、隐藏暂停且业务定时继续、恢复续调、停止后 300ms 零新增 draw、真实 SendInput 双轴滚动及 GPU 像素变化；只准备约 262 项缓存而非 100000 项逻辑数据。`canvas-d-plugin-source-final.log` 为 85 通过、22 ignored（2.23 秒），`canvas-d-mailbox-source-final.log` 为 10 通过（0.48 秒）。最终固定 SDK/包和十分钟实机长测仍未验收，#131 不关闭。

## 候选双轴审查

固定 tree `f36374280dcc50c708d408b3b3b3a819ba176912` 与本单基点比较，另核对用户固定点 `561aa5b2`：Standards 0 项确认的 P1/P2；Spec 1 项 P2：新 scene identity 复用旧 scroll epoch 时未更换原生滚动 Rc，旧惯性可能移动新内容。正在补同 epoch/extent 替换场景的回归并修复。审查 tree 之后的改动必须补查，不能直接沿用零发现结论。

补审 tree `925085da00e71d41a24a31daf93be7c4a6f22d5e`：Standards 新增 0，Spec 确认原 P2 在代码层解决、新增 0。`deactivate()` 统一更换滚动 Rc，原生 motion ID 包含 scene identity；`canvas-d-scroll-identity-release-green.log` 实际 5 通过（0.01 秒），其中 CanvasView 级回归验证同 epoch/content 的新 identity 不受旧 Rc 晚到移动影响。最初新测试因遗漏 RuntimeStyles 初始化而失败，该轮不是行为红测；补正常主题初始化后通过。

本轮仓库门禁：`canvas-d-format-final.log` 格式检查通过；`canvas-d-workspace-tests.log` 为 574 通过、0 失败、413 ignored；`canvas-d-workspace-check.log` 编译检查通过（19.21 秒）。这些结果覆盖当时工作区源码，随后原生 smoke 的公开最小化/恢复调整及最终产物仍需单独核对。

首次 `cargo test -p editor-app --release --no-run --offline` 编译应用测试成功，但整体命令因历史 `svg_preview_render` example 未接入已有 Canvas 模块、且未覆盖 `Paint::TextBox` 而失败；已核对本单基点也存在对应缺口。限定实际应用目标重跑 `cargo test -p editor-app --release --bin editor-app --no-run --offline` 成功。该结果仅为 Release 测试二进制准备，不是性能结果。

## 实机采样条件（采样尚待执行）

本机只读 CIM 查询：AMD Ryzen 9 9950X3D（16 核 / 32 逻辑处理器），NVIDIA GeForce RTX 5080，显示驱动 `32.0.16.1088`；Windows 11 专业版 `10.0.26200`，可见内存 64545140 KiB；显示模式为 3840×2160、144 Hz。实际窗口 DPI 和每帧刷新间隔由原生采样记录，不能把方案的 60 Hz 建议条件写成本机实测条件。

同机 full / patch 比较使用相同保留缓存、相同普通时间轴包、种子与真实双轴 SendInput 往返轨迹；full 指现行宿主上的全量视口发布对照，不冒称历史旧二进制。每秒记录工作集、private commit、当前/仍存活派生像素、资源、累计字节、塑形/栅格工作及有界 fixture ingress；该 ingress 不是生产 WorkSender 邮箱。每帧保存 GPUI 实际 draw/present trace，HDR 输入延迟只涵盖窗口派发至绘制，未包含硬件输入延迟。观察器也参与资源开销，两轮使用相同设置；原生 GPU atlas 内存未直接测量。
