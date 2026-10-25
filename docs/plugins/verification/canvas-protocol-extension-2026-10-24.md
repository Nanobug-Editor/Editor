# Canvas 协议扩展实施记录

最新状态（2026-10-25）：1.4–1.6 前置已独立交付至 `origin/main@56cb99ae`，两轴未解决发现均为 0，见[前置验收](canvas-baseline-2026-10-25.md)。下文较早的 A 阶段结果来自当时整合工作副本；当前正按其保存源码在已提交前置上重新验证，最终包和结果以本次续作记录为准，不能借用包含其他工作区任务的整合门禁。02–04 尚未实施。

## 已提交前置上的 A 阶段复验

### A 候选正式审查与修复（2026-10-25）

`b5826b87` 已相对固定点 `561aa5b2` 完成首轮双轴审查。Standards 轴报告 2 项 P1（图片展开在分配后才检查、嵌入原始 SVG 后绕过最终变换的滤镜预算）；Spec 轴报告 2 项 P1（虚线展开、同一图片展开问题）。两轴分别记录，重复发现不用于掩盖另一轴。

公开协议回归 `canvas_scene_rejects_unbounded_dash_expansion` 先因接纳 500000 像素线段上的极小虚线周期失败，再以控制多边形长度上界和场景共享段数预算在接纳前拒绝，通过。PNG 原生回归 `image-reuse.json` 与 SVG 原生回归 `image-svg-pixels.json` 均先显示错误占位，修复后分别在 3.32 s / 2.65 s 通过原生像素及显示回执检查：同一来源在隔离透明组重复 24 次只编码一次；SVG 只使用此前已受限解码的 Bitmap，不能在新变换下再次执行源 XML/滤镜。

生成的 SVG 追加与 PNG 压缩输出均在增长前计费，超限不生成部分成功画面。该可信数值/PNG 序列化路径与访客 SVG 的 XML/定义复用限制分开，且不共享解析缓存，不能凭相同摘要绕过访客 SVG 门禁。阶段门禁及修复后的双轴复审正在进行；当前不关闭 #128。

候选路径 `C:/Users/Tmiracle/.codex/worktrees/canvas-prerequisite/Editor`，独立 target。以保存的 A 验证副本对初始快照的差异合入前置，保留两个已审查的列宽取消修复；未复制主工作区后来出现的 Editor 协议迁移。主工作区的 `cargo fmt --check` 因该并行迁移的格式差异返回 1，原始输出保存在 `C:/Users/Tmiracle/.codex/canvas-main-fmt-20261025.log`；本候选的格式和阶段门禁单独通过。

| 入口 | 实际结果 |
| --- | --- |
| `cargo test -p plugin-protocol --lib --offline --locked --jobs 2` | 74 passed，0 failed，0 ignored |
| `cargo test --workspace --exclude editor-app --offline --locked --jobs 2` | 109 组，534 passed，0 failed，385 ignored，含文档测试 |
| `cargo test -p editor-app 'ui::plugin::' --offline --locked --jobs 4 -- --test-threads=1` | 77 passed，0 failed，19 ignored |
| `cargo test -p editor-app sdk_export --offline --locked --jobs 4 -- --test-threads=1` | 6 passed，0 failed，0 ignored |
| `cargo build -p editor-app --offline --locked --jobs 4`、`cargo check --workspace --offline --locked --jobs 4` | exit0 |
| 实际宿主 `--plugin-package ./crates/plugin-runtime/tests/fixtures/canvas-flow --output ./target/canvas-packages` | exit0，实际 WASI Release 组件重新构建 |
| `target/debug/deps/canvas_scene-3d4ed09e7e072a05.exe --ignored --nocapture --test-threads=1` | 2 passed，0 failed，0 ignored，126.07 s；实际固定包的投影、显示门禁、拒绝后恢复与 ABA |

重建后的固定包仍为首次交付的 `canvas-flow 1.0.0`，306660 字节，SHA-256 `cd61d680422c2b5781bc64fcc82b1f356ce3dda7aa4a79d76c51de931033f40e`，实际内嵌 SDK 摘要 `6b78c801f237fc1edb2e7f7e8ea1a153d2d47167cadaa51853695d2036f1bc7a`。账本只新增该包条目，其他固定输入保持；包和 SDK 均不包含文档或宿主业务源码。源码/宿主在构建时仍含未提交的 A 差异，commit 字段继续为 null。

原生验收继续使用上述 app 测试实际生成的 `target/debug/deps/editor_app-420de45671c333eb.exe`，显式 `--ignored --nocapture --test-threads=1`。basic/layers/image 与 circular-corners、dpi-125、dpi-200-dark、graphics、miter-shear、native-clips、pixel-budget、presentation、rotated-gradient 共 12 个场景通过；旧原生 TextBox/列宽入口也通过（1 项，3.27 s）。这不是把默认跳过的用例计为通过。

另将实际 WASM 的原生用例补强为：通过公开管理器调用插件已声明的 zoom/pan 按钮，宿主不计算业务缩放策略；再绘制插件返回的场景。125% 下蓝色节点面积要求已从初始画面的 9124 像素红灯，改为放大后的至少 13500 像素；200% 深色对应至少 34000。实际 Canvas 原点取自原生布局，在局部显示坐标 `(50,55)`、`(160,55)` 检查蓝色，分别防止错误的重复缩放与漏用平移；这些坐标对应 guest 声明的 1.25 缩放和 20 像素平移。它们是公开管理器命令加真实原生像素的组合验收，不冒充 OS 鼠标点击或输入法验收。最终 125% 浅色、200% 深色均通过，分别 41.30 s、40.41 s；合计 14 个结构化场景组合及 1 个旧 TextBox/列宽原生入口显式通过。正式双轴审查及工单交付尚未完成。

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
