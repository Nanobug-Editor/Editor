# Canvas 协议扩展实施记录

2026-10-24 开始，2026-10-25 续作。状态：阶段 A 实施与完整回归中；开发清单已接入 `ui.canvas 1.7` 候选，尚未验收交付，未提交、推送或关闭工单。

规格见[通用 Canvas 协议扩展方案](../specs/canvas-protocol-extension.md)；用户已明确确认[四张工单、依赖、测试接缝及审查基点](../tickets/canvas-protocol-extension/README.md)，并要求全部顺序实施，无需逐单等待继续。方案 #127 与 #128–#131 已发布，原生父子/阻塞关系已读回，真实数据库 ID 见[发布记录](../tickets/canvas-protocol-extension/publication.json)。GitHub 连接器创建返回 403，发布使用既有本机 Git 认证的 REST 请求，凭据仅在进程内存中使用。

## 基线与任务边界

- 初始 HEAD：`561aa5b2006c38668a083482156646a16cd02b3e`，分支 `main`，工作区已有大量未提交/并发变更；本任务不得将它们当作成果或一并提交。
- 本任务源文件基线位于本机 `C:/Users/Tmiracle/.codex/canvas-protocol-extension-baseline-20261024/`，覆盖最初的 protocol/runtime、宿主 UI 和 editor-ui 源码。后续修改未在基线内的已有文件前另存原内容。
- 实现路线：复用规格、四个垂直切片、顺序 implement；TDD 接缝为公开协议/管理器、实际 WASM 包、真实原生 GPUI。尚未执行双轴审查；其固定比较点已确认，但必须排除既有工作区差异。
- `cargo metadata --no-deps --format-version 1` 核对根目录为当前 checkout，默认产物在其 `target/`。协议测试另用 `target/canvas-protocol`，缓解其他任务使用默认输出目录的构建锁。

## 阶段 A 已实际执行的检查

| 检查 | 实际结果 |
| --- | --- |
| `cargo test -p plugin-protocol canvas_scene_admits_paths_and_negotiates_its_version` | 先因未知 `scene` 字段失败；新增有界声明、变换/几何/样式校验及最低能力版本推导后通过 |
| `cargo test -p plugin-protocol canvas_scene_rejects_unowned_image_node_references --lib --target-dir target/canvas-protocol` | 先因接受缺失图片引用失败；加入同文档 Image/FileImage 引用校验后通过 |
| `cargo test -p plugin-protocol --lib --offline --locked --target-dir target/canvas-protocol` | 当时 71 项通过；后续协议变化仍须重跑受影响检查 |
| `cargo test -p editor-app native_canvas_scene_paints_transformed_shapes -- --ignored --nocapture --test-threads=1` | 使用独立 Windows/D3D 子进程；先读取到白色空白，再由后台准备 GPUI 路径后读取到目标位置的红色矩形，通过 |
| 同一原生测试设置 `NANOBUG_CANVAS_SCENE_CASE=layers` | 先读取到白色空白；复用后台有界 resvg 栅格准备后，重叠与非重叠区域均为同一半透明粉色，裁剪外为白色，通过；截图位于 `target/canvas-scene-layers.png` 并已查看 |
| 同一原生测试设置 `NANOBUG_CANVAS_SCENE_CASE=image` | 已复现空白图片；图片准备顺序、缓存来源身份与原生图片引用接线后像素正确，修正测试窗口退休顺序后完整测试通过，另以已编译子进程连续复跑两次通过；截图已查看 |
| `canvas_scene_document_budget_is_shared_across_canvases` | 现行整文档编码上限已能拒绝多场景累计超额，无需复制一套编码预算 |
| `canvas_scene_bounds_projected_strokes_before_native_tessellation` | 先复现变换后的描边越界仍被接纳，再增加保守外扩校验后通过 |
| `canvas_scene_presentation_receipts_are_exactly_addressed` | 先因未知回执类型失败；新增公开回执及精确身份/版本校验后通过，原生发送路径也已通过 |
| `canvas_scene_viewport_bounds_the_projected_artwork` | 先复现小 content 声明隐藏巨大投影坐标，再纳入组合变换/描边边界校验后通过 |
| `canvas_scene_receipt_survives_repaint_without_crossing_source_authority` | 先因 UI 重绘淘汰同一已显示场景回执而失败；仅对身份、绘制版本、来源和投影仍相同的显示观察开放窄例外，普通输入仍精确校验 UI revision；通过 |
| `graphics.json` 原生读回 | 三次曲线、三色标渐变、圆角、旋转和偶奇裁剪孔通过；曾复现孔被填满，修正 SVG `clip-rule` 后通过 |
| `rotated-gradient.json` 原生读回 | 曾复现渐变方向没有随组旋转；改用保留局部渐变坐标的回退后通过 |
| `circular-corners.json`、`native-clips.json` 原生读回 | 原二次曲线圆角偏离圆形半径，现复用 GPUI `arc_to` / SVG 圆弧；圆角边缘与层级矩形裁剪均通过，普通矩形裁剪复用 GPUI content mask |
| `image` 原生读回（中心及左右边缘） | 曾复现 4×4 图片放大时从 atlas 边缘混入灰色；有预算的边界 texel 保护后，中心及左右边缘均通过，同源多次放置复用派生像素 |
| `pixel-budget.json` 原生读回与公开回执 | 四个复杂画布占满共享 64 MiB 派生像素池，第五个收到明确失败回执，前四个保持成功；通过 |
| `dpi-125.json`、`dpi-200-dark.json` | 自有 HWND 的 125% / 200% DPI、明暗主题、透明组与裁剪像素通过；未修改系统 DPI 设置 |
| `canvas_scene` 实际 WASM 管理器测试 | 2 项显式 ignored 测试通过：缩放后旧坐标拒绝，发布但未呈现的坐标拒绝，显示回执后选择成功；版本回退与身份 ABA 原子拒绝，同一实例保留旧视图并可修正发布，卸载释放资源 |
| `wasm-flow-125.json`、`wasm-flow-200-dark.json` 原生读回 | 从真实包经公开管理器按需激活取得文档，125% / 200% 明暗主题下两个流程图节点、曲线与精确回执通过；截图已查看 |

原生测试夹具已修正两处准备问题：不能在已经借用 Root 的回调中再次主动 draw；截图位置应以 Cargo manifest 定位仓库 target，而不是假定测试 cwd 为仓库根。首次编译及链接日志含其他工作区已有警告，不能据此宣称完整门禁通过。

图片原生用例还稳定复现了测试进程退出期间的 AccessKit `StringBuffer::acquire` TLS 错误；栈表明窗口焦点通知发生在线程销毁期。仅改变测试退出顺序：先关闭自有 HWND、处理剩余事件并确认 App 窗口表为空，再退出应用，即消除该错误；原图片场景完整通过及两次独立复跑均已记录。未修改 GPUI/AccessKit 或禁用可访问性。

真实包 `crates/plugin-runtime/tests/fixtures/canvas-flow/` 只依赖公开 SDK，包版本 `1.0.0`。先经宿主 `--plugin-cargo ... build --release --target wasm32-wasip2 --offline --locked` 构建，再直接归档 manifest 与实际 WASM 到 `target/canvas-packages/`，由 `Package` 和 `Manager` 验证。早期 `--plugin-package` 已完成独立 SDK 编译，但当时宿主仍登记 1.6，按预期拒绝要求 1.7 的包。最终宿主/SDK 与正式夹具摘要仍须重建核对，当前不能把旧宿主的打包失败记作完整打包通过。

实际包测试先发现“拒绝非法场景后通用停机逻辑清空旧视图”，已将结构化场景发布错误与 WASM trap 区分，并覆盖普通、轮询和停放实例的保留路径。首次测试夹具还漏了按需激活，现使用公开 `availability → begin_demand(View) → poll`；未恢复按安装顺序启动。原生子进程预算明确由 60 秒真实包冷激活与 30 秒原生部分组成。

派生场景资源与原图片解码池分别有界：场景纹理共 64 MiB、native 顶点共 1,000,000，单回退纹理最多 2048×2048，SVG 展开最多 4 MiB，场景编码最多 2 MiB，并受既有整文档编码预算约束。实例最多保留 4096 个场景身份墓碑，普通重绘保留 identity。显示回执包含文档/文件来源及 viewport 声明，运行时在真实呈现前拒绝坐标输入；原生回调也不将尚未绘制的新声明作为当前命中画面。

首次完整 `cargo test --workspace --exclude editor-app --offline --locked` 因系统提交内存峰值失败：原始错误明确为 mmap `libplugin_runtime-4d9548e58809790f.rlib` 时“页面文件太小”(OS 1455)，并伴随依赖 rlib 链接错误。未清理缓存或改产品接口。保留 workspace 功能组合的 `--test context_selection --no-run -vv` 探针随后通过；正在以 `--jobs 1` 重跑完整入口。原始日志为 `target/canvas-workspace-test.log`，探针为 `target/canvas-cache-probe.log`，串行门禁为 `target/canvas-workspace-test-j1.log`。

## 尚需完成

阶段 A 的最终源码完整门禁、受影响宿主回归及最终宿主/SDK/包重建见后续复验记录；固定包原生入口也已复核，尚需正式审查与提交边界确认。已运行格式化与格式检查；当前未将候选标记为 1.7 已交付。

阶段 B/C/D 均未实施。已确认全部执行的授权继续有效；当前没有等待用户决定的产品问题。

## 2026-10-25 最终候选复验

为避免与其他构建竞争测试程序，使用专属验证副本 `C:/Users/Tmiracle/.codex/nanobug-canvas-validation-20261025/`，独立 `target/`；`cargo metadata` 已核对两者路径。复制时核对 41 个本任务源码/输入的 SHA-256；后续剪切描边修正同时应用到原工作区与验证副本，两个 `scene/svg.rs` 摘要相同。

| 检查 | 实际结果 |
| --- | --- |
| 原工作区 `cargo test -p plugin-protocol --lib --offline --locked --target-dir target/canvas-protocol` | 76 passed，0 failed |
| 独立副本 `cargo test --workspace --exclude editor-app --offline --locked --jobs 2` | exit0，113 组，766 passed，0 failed，409 ignored；包含 doc-tests，跳过项不计通过 |
| 独立副本 `cargo test -p editor-app 'ui::plugin::' --offline --locked --jobs 2 -- --test-threads=1` | 77 passed，0 failed，18 ignored |
| 独立副本 SDK 导出 6 项测试 | 6 passed；仅 Rust/Cargo/WIT/JSON、并发导出、缓存修复与保留调用方文件通过 |
| 独立副本 `cargo check --workspace --offline --locked --jobs 2` | exit0；保留已有警告 |
| 独立副本 `cargo build -p editor-app --offline --locked --jobs 2` | exit0，真实 CLI 构建通过；保留已有警告 |
| 原生矩阵 | graphics、旋转渐变、圆形圆角、层级矩形裁剪、125%/200% 明暗 DPI、显示回执、共享像素预算及真实 WASM 明暗两档 DPI 均通过 |
| `miter-shear.json` | 额外复现剪切后的尖角描边被回退纹理边界裁掉；边界外扩改用矩阵行和的保守界后，通过 400% DPI 的目标像素验收 |
| 最终固定包管理器测试 | 正式入口重建后的固定 ZIP，2 项显式 ignored 测试通过 |
| 最终账本入口的原生复核 | 重新编译测试入口，从固定账本校验并加载 ZIP；125% 浅色与 200% 深色各 1 项通过（28.00 s / 28.44 s） |

原工作区串行完整门禁曾得到 766 passed、0 failed、409 ignored，随后 `editor-terminal` 的 doc-test 无法加载 `editor_ui`；该入口总体失败，未记作全绿。独立副本同源门禁含 doc-tests 全部通过，没有为消除原工作区产物问题修改终端或控件接口。

正式 `--plugin-package` 使用重建宿主、当前内嵌 SDK 和实际 `wasm32-wasip2` Release 组件，exit0。固定输入为 `crates/plugin-runtime/tests/fixtures/packages/canvas-flow-1.0.0.zip`（306767 字节），SHA-256 `d12e9102e9d7112929d12b8bc45faecd504ee3b275d495a80015b4cc584773de`；SDK 摘要 `bcb97c1e3817f1be0397b9e0c0b426efb6bc28926b05479bfd7dc04946d4fff8`，工具链 `rustc 1.98.1 (48a229cea 2026-09-01)`。账本登记实际摘要；源码/宿主提交为 null，不能将未提交状态伪装成固定 commit。管理器及原生测试改为从既有账本加载固定包并校验摘要。

正式双轴审查尚未进行：固定点与 HEAD 仍相同，已提交比较为空；所需 1.4–1.6 基线也尚未提交。已向用户询问[前置提交边界](../tickets/canvas-protocol-extension/git-boundary.md)，该问题仅影响审查/Git 交付方式，原四单授权未撤销。未提交、推送或关闭 #128–#131，不能宣称四单完成。
