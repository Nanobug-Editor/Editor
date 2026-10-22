# 插件开发工具改进实施记录

日期：2026-10-21。状态：S1–S5 已实施并完成本批功能验收；全仓格式门禁仍受并行 UI 改动影响，实际通过项及限制见末节。

依据：[已确认方案](../specs/plugin-developer-tooling.md)。用户已授权连续实施三项改进；本轮不创建议题、提交、推送或发布。安装兼容插件不重启宿主列为 H01 验收。

## 固定输入

- 工作区：`C:/Projects/RustProjects/Editor`；HEAD `fc9dc531ef991315d73e21c8a092f12a0f1ff78b`。
- 开始前 1885 份现存受管/未跟踪文件的副本：`C:/Users/Tmiracle/AppData/Local/Temp/nanobug-tooling-baseline-a499b708f0a14494a4f9bee826b30c5d`。工作区既有大量 UI、协议及插件管理改动，不算本次成果。
- 本批独立 Cargo 输出：`target/plugin-developer-tooling`，`cargo metadata --no-deps --format-version 1 --offline` 已核对根工作区与输出目录。
- Rust/Cargo 1.98.1；已安装 `wasm32-wasip2`、`x86_64-pc-windows-msvc`，未安装新工具链。
- 改动前 `cargo test -p plugin-protocol --features guest --offline`：61 单元、6 集成、1 文档测试通过；这是开始时输入的基线，不代替新增功能验证。

## 实现范围

| 交付区域 | 已实现的行为 | 入口 |
| --- | --- | --- |
| SDK 助手 | 显式资源组、编辑意图代次替换、HTTP 流及轮询任务；保留取消效果与清理失败，Drop 不调用宿主 | [ResourceGroup](../../../crates/plugin-protocol/src/api/guest/resources.rs)、[EditorIntent](../../../crates/plugin-protocol/src/api/guest/intent.rs)、[HttpTask](../../../crates/plugin-protocol/src/http/task.rs) |
| 运行观测 | 默认计数、显式 60 秒详细捕获；按真实实例/作用域归属统计，记录嵌套墙钟、fuel 与可测字节；读取实际资源表及原生收尾证据 | [观测器](../../../crates/plugin-runtime/src/observability.rs)、[实例采样](../../../crates/plugin-runtime/src/instance/observation.rs) |
| 原生诊断 | 插件详情“运行诊断”，最多每秒读取快照，采集/停止/导出、慢操作和真实资源数，中英文资源 | [诊断页](../../../crates/editor-app/src/extensions/management/diagnostics.rs) |
| 检查工具 | `--plugin-check` 共享生产 Package 校验；`--plugin-dev --report` 保存实际生命周期证据与明确退出原因，不宣称插件业务已测 | [静态报告](../../../crates/plugin-runtime/src/validation.rs)、[生命周期报告](../../../crates/plugin-runtime/src/development/report.rs)、[控制器](../../../crates/editor-app/src/plugin_development/cli.rs) |
| 真实包验证 | 新 SDK 独立消费者、冻结旧 ZIP 矩阵、公开管理器/原生 UI 热安装、退出与报告回归 | [真实助手测试](../../../crates/plugin-runtime/tests/lifecycle_helpers.rs)、[固定矩阵](../../../crates/plugin-runtime/tests/tooling_compatibility.rs)、[原生 CLI 回归](../../../crates/editor-app/src/plugin_development/native_report_tests.rs) |

未新增 crate、插件专属宿主 API 或旧协议运行分支。SDK 内容仍只有源码、Cargo、WIT/JSON；新助手不增加 wire/权限要求。详细捕获默认关闭，导出不混入原有文本日志。

## 工具及不可变输入

同一份工作区源码使用两个独立输出目录：`target/plugin-developer-tooling` 和 `target/plugin-developer-tooling-sdk`；两次 `cargo metadata --no-deps --format-version 1 --offline` 均确认 `workspace_root` 为本仓库。后者用于协议、SDK 和应用测试，前者用于完整 workspace 检查及原生可执行文件。并行修改期间的具体 EXE 身份分别记录，不把同版本号当作相同产物。

| 已实际执行的宿主 | SHA-256 | 用途 |
| --- | --- | --- |
| SDK 目录中的旧候选 | `33ccae24e0e364ae1037fd97b064f1b6561b3b1d5e540b471282a1216a6edbfb` | 诊断页原生验收；保留作为退出问题红灯对照 |
| 主目录中的退出修复候选 | `dfcdd49166ed446475156f5944b106f059e9b3a2faf904882e2084275ab4250c` | 真实 WASM 正常 stop、H01 热安装/更新失败/禁用及首轮 CLI 验收 |
| 报告边界中间候选 | `6a2ba0bb1e1a8fee52ddaa9f1bcfe9c6995c7e0d0c00b4aa8d7d347ad2a0570b` | 实际子进程崩溃；未分类准入退出码的红灯对照 |
| 最终构建 | `c82565746b49011761884604da17e1bcbd30a45cb972b42a2a6ba20fda940856` | 最终 CLI 10 个场景；应用 9 项回归使用的真实原生子进程 |

上述 SDK 内容摘要均为 `8e2f359fedf819d8e8b6b3da1012722629e12d9731c02c38e5b01ccab8bd02e4`。各项证据保留实际执行时的身份，不以随后构建冒充原生人工交互产物。

当前两个独立消费者版本均为 `0.1.2`，依赖公开 SDK `=0.2.0`，不指向仓库 `crates/`；通过宿主 `--plugin-package` 直接构建 Release WASM。输入位于 [lifecycle-inputs.json](../../../crates/plugin-runtime/tests/fixtures/lifecycle-inputs.json)，包位于 `crates/plugin-runtime/tests/fixtures/packages/`。

| 冻结包 | ZIP SHA-256 |
| --- | --- |
| `lifecycle-http-0.1.2.zip` | `32c15c54e97715f7b58c2bfb291def7e8a86488421ab40fd52849a965ee3ccfa` |
| `lifecycle-editor-0.1.2.zip` | `07e91160ee8a8fa5a8387b561bde4c939cf2b9b5755bbf8dd118897ad8923e99` |

`0.1.0`、`0.1.1` 原字节也已冻结，分别标明当时 SDK 摘要；没有用新构建覆盖同名旧包。原有 `packages/inputs.json` 不变。旧兼容矩阵另外读取并校验 `capability-example-0.19.0.zip`、`cloud-0.1.1.zip`、`code-host-0.1.1.zip`，不能用上述新消费者替代旧包兼容证据。

## 功能证据

### SDK 及真实包

`cargo test -p plugin-protocol --features guest --offline`：67 单元、6 集成、1 文档测试通过。新增用例覆盖部分清理重试、迟到结果过滤、已经完成但尚未送达时的意图替换、HTTP 创建到一半失败、终态释放、有限轮询预算及 Drop 无宿主调用。

真实消费者显式执行，不把 ignored 当作通过：

```powershell
$env:NANOBUG_TOOLING_FIXTURES = (Resolve-Path crates/plugin-runtime/tests/fixtures/packages).Path
cargo test -p plugin-runtime --test lifecycle_helpers --offline -- --ignored --test-threads=1
cargo test -p plugin-runtime --test tooling_compatibility --offline -- --ignored --test-threads=1
cargo test -p plugin-runtime --test capability_packages --offline -- --include-ignored --test-threads=1
```

结果：助手 2 项通过；固定旧包矩阵 1 项/3 个旧 ZIP 通过；既有能力包 6 项通过（含 2 个真实包 ignored 场景）。后者按现行冷安装语义先安装再走公开 demand，不再把安装等同执行。

HTTP 本地服务返回 404、中文原始字节、延迟终态；断网连接失败、32 个计时器占满后的部分初始化失败、guest trap、关闭资源均有实际 WASM 证据。编辑器消费者通过公开请求门验证取消和已完成未送达替换。测试检查真实资源表及业务回执，不只断言 mock 调用次数。

### 诊断及原生 UI

Windows 原生候选中打开实际插件详情，验证以下输入与可见结果：

- 打开页面不会启动插件；默认没有详细记录。点击采集后得到 60 秒范围的 40 条记录，`partial=false`，容量上限由确定性测试另行覆盖。
- 键盘 Tab 可依次聚焦采集、停止和导出；列表可滚动，详情页签保持固定。正常/深色主题文字和按钮可读；中英文/深浅主题也由两个独立 GPUI 上下文测试覆盖。
- 通过 Windows 文件夹选择器导出到隔离工作区，实际文件 `native-workspace/nanobug-plugin-diagnostics-1792580815290.json` 为 31,355 字节，带平台 `windows-x86_64`、EXE/SDK 摘要、会话、捕获范围和覆盖信息；无日志正文、URL、请求体、文档内容或 argv。

当前测量包含 Preparation/Queue/Activation/Guest/HostCall/Request/HTTP/Native/Publication 的已接入边界。更细的共享等待、合并丢弃、迟到回执、发布到渲染归因等继续列为 `unobserved_details`；没有原生 CPU/RSS 或绘制耗时。原生收尾失败即使从 reaper 列表移除，也保留 `native_cleanup_unconfirmed`，不能显示为清理成功。

### H01：安装无需重启

使用上表 `dfcdd4…` 候选及独立 `h01-profile`，实际进程 PID `1552`、启动时间 `2026-10-21 19:33:25 +08:00`、原生窗口 ID `986428`。

1. 原始注册表为 `{}`；原生打开 `000-h01.txt`，输入 `Nanobug H01 未保存内容：热安装不重启`，编辑标签显示未保存标记。磁盘仍只有 `H01 original content on disk.`。
2. 将摘要已核对的 `lifecycle-editor-0.1.2.zip` 内容送入现有开发候选通道，经生产目录 Package/Manager 准入和公开权限，面板 `Lifecycle / 生命周期` 立即出现并显示 guest 回执。没有重建宿主或重启进程。
3. 发送独立 `0.1.3` 候选，其组件缺少 `dispatch` 导出。生产准备失败明确保留旧版本；注册表仍为 `0.1.2`，内部摘要 `3bd6731cdb280e5effe98506fc105a9c3c25cdce13f55288d23103c8d0d2b6ab`，旧面板与未保存文本保留。
4. 通过原生插件管理器选择“全局禁用”；面板退役，注册表 `enabled=false`，编辑区仍保留同一文本。
5. 实际 Ctrl+Z 恢复磁盘原文，Ctrl+Y 恢复中文未保存文本，证明原撤销/重做记录仍有效。各步骤 PID、启动时间和窗口身份不变，磁盘文件始终未改写。验收结束后才主动退出该隔离会话。

另有真实 ZIP → 公开 Manager → GPUI 的测试，直接断言进程、Editor entity、文档内容和磁盘一致，并覆盖按需执行及禁用。该夹具不声明语言 grammar；不把它当作所有语言功能热更新的证明。中文文本输入已验收，不宣称本批重新覆盖所有系统 IME 组字场景。

### CLI 与退出

首轮实际命令已核对：有效 ZIP 为 0 且 `executed=false`；坏归档为 1；缺失输入、非法格式、缺失 `--report` 参数及输出父路径为文件均为 2；初始协议拒绝为 `failed/1`。无效项目描述属于输入错误，为 2。

最终 `c82565…` 再执行 10 个实际 CLI 场景：有效 ZIP=0、坏归档=1、缺失输入=2、非法格式=2、未分类准入拒绝=1、缺失报告参数=2、不可写报告=2、超长合法版本=0、超长不满足的要求范围=1、SDK 导出=0。两个 1 MiB 元数据场景输出均小于 8 KiB，省略值保留 SHA-256 与 `metadata_partial`；合法版本仍通过静态准入。实际 SDK 导出无 Markdown，助手源文件完整。

未分类准入的真实红绿对照也保留：中间 `6a2ba0…` 已携带 typed context，但旧错误链提取仍返回 2；最终 `c82565…` 使用 `anyhow::Error::downcast_ref` 后返回 `failed/1`，并替换预置旧成功报告。证据位于 `cli-delivery/red-*` 和 `cli-delivery/legacy-*`。

异常退出场景使用 `6a2ba0…`：等待合法资源包实际 startup 后，只终止控制器下属、可执行路径完全匹配的 GUI 子进程。控制器退出 2，报告为 `termination=crashed`、`status=inconclusive`、`cleanup=inconclusive`。证据：`crash-delivery-455a2d4042034df3a7128e0859db0405/report.json`。私有控制器及 GUI 均已退出，用户 Release 会话未操作。

真实 WASM 开发会话以 `--plugin-dev … --release --report … --grant editor.read --grant ui.panels` 启动；实际激活后 stdin `stop`，结果为 `passed/0`，admission/startup/activation/cleanup 全通过，termination 为 normal，business 保留 `not_exercised`。证据：`stop-fixed.json` 和 `dev-stop-fixed/observations-21180-1792582173042999700.json`。

原生发现的退出问题已修复：开发窗口是独立 EditorApp，不能等待普通 ProjectWindow 协调器的空成员回执；现在绑定实际编辑器的现有关闭检查，等待项目及共享 RuntimePool 收尾后退出。自动化回归通过公开 CLI 使用独立资源包/profile，不增加测试专用宿主能力：

```powershell
$env:NANOBUG_TOOLING_HOST = (Resolve-Path target/plugin-developer-tooling/debug/editor-app.exe).Path
cargo test -p editor-app developer_tooling_native_report_stop_cancel_and_reload_failure --offline -- --ignored --test-threads=1
```

修复候选中 stop=0、cancel=3、失败重载后保留旧会话并 stop=1，清理全部取得实际回执，16.63 秒通过。相同测试指向旧 `33ccae…` EXE 时，明确得到 stop=2、termination=forced、cleanup=inconclusive（15.20 秒），证明回归确实能捕获原问题。第一次不完整的测试输入导致 startup 超时，已修正为合法贡献资源包，不将该次超时充当退出红灯证据。

### 观测成本

相同 Windows debug 测试宿主、相同 10,000 次元数据工作负载：关闭捕获中位墙钟 7,866 µs，开启 129,768 µs，约增加 12.2 µs/操作。两侧业务计数等价，详细记录有界。这仅为观测层微基准，不是访客/原生 CPU、端到端性能或 Release 开销承诺；默认保持关闭详细捕获。

## 检查、审查与限制

首轮完整 `cargo test --workspace --exclude editor-app --offline --jobs 1 -- --test-threads=1` 已通过：89 组结果、445 项通过、342 项 ignored。补齐报告边界回归后，同入口最终重复通过：**89 组、449 项通过、342 项 ignored**（`workspace-delivery.log`）；被跳过的其它旧集成场景不计通过。最终真实 WASM 助手 2 项及冻结旧 ZIP 矩阵 1 项也显式复测通过，分别耗时 78.76 秒、164.68 秒（`real-wasm-delivery.log`）。

应用侧首轮本批 7 项（含真实 WASM H01）、管理器 9 项、SDK 导出 6 项通过；shutdown 1 项通过/1 项 ignored，resize 场景为 ignored，未声称已执行。最终本批扩展为 **9 项全部通过、无 skipped**，包括 typed context 退出码、旧报告替换、真实 WASM 热安装和实际原生 stop/cancel/失败重载（`app-tooling-delivery.log`，67.60 秒）。入口：

```powershell
$env:CARGO_TARGET_DIR = (Resolve-Path target/plugin-developer-tooling-sdk).Path
$env:NANOBUG_TOOLING_FIXTURES = (Resolve-Path crates/plugin-runtime/tests/fixtures/packages).Path
$env:NANOBUG_TOOLING_HOST = (Resolve-Path target/plugin-developer-tooling/debug/editor-app.exe).Path
cargo test -p editor-app developer_tooling --offline --jobs 1 -- --include-ignored --test-threads=1
```

| 最终检查 | 实际结果 |
| --- | --- |
| `cargo check --workspace --offline --jobs 1` | 通过，6.65 秒，`check-delivery2.log` |
| `cargo build -p editor-app --offline --jobs 1` | 通过，2 分 28 秒，`exe-delivery-build.log`；保留既有 warning |
| 文档本地链接、6 个冻结归档 SHA-256、`git diff --check` | 通过 |
| 本批 58 个 workspace Rust 文件的 `rustfmt --check --edition 2024 --config skip_children=true` | 通过；独立冻结消费者不在 workspace 格式入口内，未在归档后改写 |
| `cargo fmt --check` | **全仓未通过**，`fmt-final-attempt.log`：并行修改的 `editor-ui/aligned_editor/{geometry_cache,geometry_measurement,pane}.rs`、`row_backgrounds/mod.rs`、`scroll/tests/native_windows.rs` 有格式差异，本批未覆盖它们 |

首次完整检查曾遇到独立 Cargo 缓存缺失，核对 metadata 后只清理本批输出中的对应产物重建；并行 UI 工作曾使完整检查报 `aligned_editor` 几何失败及 `scroll/tests/native_windows.rs` 的 E0502，后续同入口已通过，本批未修改那些 UI 实现。应用重测曾因同时运行的测试 EXE 被占用触发 LNK1104，改为串行后通过。上述失败均保留日志，不冒充首次通过。

最终重编译也曾遇到其它任务新增 `geometry_measurement.rs` 时的 E0061/E0599；等待其自行完成后，以相同入口重复，得到上面的最终 check/build/应用回归通过结果，本批未修补该模块。

最终复审分两条轴：

- **Standards**：最终冻结快照复审通过，无剩余 P1/P2。已修复有界 Owner 元数据、退役后最终采样和原生失败证据；顶层版本/范围补 128 字节上限、原值 SHA-256 和 `metadata_partial`，不改变合法包准入。
- **Spec**：最终冻结快照复审通过，无剩余 P1/P2。已修复完成未送达意图替换、准入退出码、旧报告假通过及实际 stop 路由；末轮补齐所有 Package 准入的类型化阶段边界，并把报告初始化移到 SDK 准备之前。保留原有错误文本，结构化输出不解析或复制该文本。

新边界测试包含 1 MiB 的合法 SemVer/要求范围，校验报告小于 8 KiB、身份省略有摘要、生命周期不完整不通过；未分类的旧拒绝仍保留 `unclassified` 覆盖，而开发报告按“包准入失败”返回 1。两路复审为独立只读源码审查，未冒称由审查者再次执行全量测试；实际测试结果以上述日志为准。SDK 准备失败的顺序修复经源码复审及报告初始化测试确认，未通过破坏共享用户 SDK 缓存来注入 I/O 失败。

本次只在 Windows 执行；其它平台未验收。未发布 SDK/插件/宿主、未提交或推送；不扩展市场业务、自动业务场景 DSL、CPU/RSS 分析或所有语言工具集成验收。
