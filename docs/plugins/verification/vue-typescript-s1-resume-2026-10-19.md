# Vue / TypeScript S1 恢复与验证记录

日期：2026-10-19（Asia/Shanghai）。状态：**新工单 #121 已在正式基线 `b59f5ac` 上重新集成，门禁、真实包联验、相关原生交互与双轴审查均通过；三仓普通提交、推送及远端 SHA 核对已完成，工单关闭状态待读回。** 最新集成与交付状态见[正式基线复验记录](vue-typescript-s1-delivery-2026-10-19.md)。本页保留首次冻结基线上的失败与修复过程，不将其追记为当时已通过。

## 本轮范围与历史保留

用户本轮只要求执行 [#121 / S1](../tickets/vue-typescript/scenarios/01-real-projects.md)。新 #122–#126 继续暂停；旧 #108–#118 仅保留历史，不执行、不改写、不关闭；父 #107 不变。没有版本发布授权。

[六场景拆分](../tickets/vue-typescript/replan-proposal.md)、六张场景正文及 [publication 映射](../tickets/vue-typescript/scenarios/publication.json)承接 2026-10-15 已批准的全部 43 项验收要求。导入记录中的 `ready-for-agent`、连续实施和暂停阶段保留当时原文，当前授权以本段及[工单索引](../tickets/vue-typescript/README.md)为准。此次复制历史文档不恢复其他工单。

[22:51 暂停记录](vue-typescript-paused-2026-10-15-2251.md)及其引用的旧证据原样保留。旧候选、旧测试程序与旧工作树的 GREEN 不作为本轮集成通过；文件名含 `green` 的日志仍按实际退出结果判断。

## 首轮冻结时点的宿主基线及提交边界

以下表格描述首次恢复时点。用户后续提交并推送了 UI 前置，本轮已切换到 `b59f5ac546a53ba084c729d382871a046bec9d38`；旧快照、账本与日志继续按原身份保留，当前提交范围以正式基线复验记录为准。

| 项目 | 已核对的身份与边界 |
| --- | --- |
| 原任务 worktree | `C:/Users/Tmiracle/.codex/worktrees/vue-typescript/Editor`；HEAD `1379f58df76622bdf4d6a7c4167469f124d17a0b`。保存原 S1–S6 WIP，本轮不修改 |
| 原 main 工作区 | `C:/Projects/RustProjects/Editor`；只读核对 HEAD `eace01458ff422cdb37947c2a7674681336550ee`。有大量其他任务未提交改动，本轮不修改或提交这些改动 |
| 远程 main | 本轮只读 `git ls-remote origin refs/heads/main` 为 `005f6a8604fb371ca0f304f9ec13a38fa213d808`；本地 main 领先 10 个提交。不是已同步或已推送本地基线的证明 |
| 新 S1 集成树 | `C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor`；从 `eace014` 加载已冻结的外部 main 工作树快照，再迁入 S1 所需差异 |
| 外部基线归档 | 新树 `target/s1-resume-2026-10-19/external-main-baseline.zip`，58,579,214 bytes；SHA-256 `CD133A3110D7BCF158D528A9A73A99085B3F37BDAFB794D9FCBA521265B735E2` |
| 外部基线逐文件账本 | 同目录 `external-main-baseline-files.json`，1,743 条路径/摘要；它描述复制时点，不追随原 main 后续改动 |
| 编译目录 | 新宿主的 `cargo metadata` 已核对 `workspace_root` 与 `target_directory` 均位于新集成树；新树产物不与原树、旧树共用。app 用例从当前测试程序同 profile 的 `examples/lsp_fixture.exe` 解析；`native_modules` 集成用例仍使用编译时新树 `target/debug/examples/lsp_fixture.exe`，具体身份见下方复验记录 |

当时已提交的 `eace014` 仍使用 Kit 组件默认功能、`tree-sitter-languages` 及 `component::Theme`，本地 UI/主题/高亮实现只在外部快照中，因此当时不能作为 #121 独立提交。这个前置现已由用户的 `b59f5ac` 解决；S1 不将其重做或并作自身成果。

S1 复用 `gpui_base::Root`、`ui::theme`、本地 `ui::controls` 和 `language::highlighter`；未恢复 `gpui_kit::component` 调用。UI、Git、窗口、消息、停靠以及其他原有 WIP 均按冻结摘要排除出本轮新增成果。

本轮保留既有 `workspace.files 1.3`，未迁入 S2 的 `Plan.conditions`、跨文档事务或自动导入接受动作，也未迁入 S3 的选区格式化动作。历史检查点中的 `workspace.files 1.4` 属于旧树保存内容，不代表新 S1 树已启用。

[首次相对冻结基线差异账本](vue-typescript-s1-diff-2026-10-19.json)列出生成时点的新增、修改及删除和两端 SHA-256。它是历史证据，不是正式基线上的暂存清单；归档中的原字节不变。新集成树以用户正式提交为父提交，只迁入核对后的 S1 差异，不包含外部覆盖层。

## 后续复核结果（覆盖下方早期断点的待跑描述）

以下是同一次 S1 实施继续推进的实际结果；下方早期断点保留失败和修复的来历，不能将其尚未执行的描述当作最新状态。

| 检查 | 实际结果 | 身份与限制 |
| --- | --- | --- |
| `cargo test --workspace --exclude editor-app -j1` | PASS，exit 0 | `workspace-non-ui-first.log`。普通 workspace 门禁完成；ignored 仍为跳过，不计入实际 WASM 验收。后续审查修复后仍需复跑受影响检查 |
| editor-app 测试程序复编 | PASS，第二次 3m43s、第三次 2m14s、共享文档 RED 候选 2m20s | `editor-app-second-build.log`、`editor-app-third-build.log`、`shared-close-red-build.log`。不是全部 app 测试或桌面交互通过 |
| 宿主 strict control ACK | 真实 fixture RED → GREEN，1/1，0.25s | `control-handshake-host-red.log` 与 `host-control-source-green.log`；缺 marker、错误版本、缺方法、重复方法和未知字段均在自定义请求前拒绝；永久协议拒绝不自动重试三次 |
| 原生控制和关联文档 | 两项 GREEN，分别 0.12s、0.06s | `host-control-source-green.log`；真实 native revision 37 与 wire version 1 分离、伪造版本拒绝、70 KiB 正文单独同步、业务拒绝/取消复用连接、关联源变化撤销结果 |
| Unicode Base 定义坐标 | RED → GREEN，1/1，0.25s | `definition-offset-{red,green}.log`；修复真实 Base UTF-8 byte offset 到 LSP UTF-16 的转换，保持 `中😀` 后位置 |
| 当时 SDK 原生模块多消费者 | GREEN，1/1，153.29s | `native-modules-consumers-current-sdk.log`；真实独立 WASM consumer A/B、健康 framed 往返、exporter/recipient 双向物理退出、无模块 sibling 保持；此结果早于后续 pending 账本修复及新增 guard 断言，不能替代当前源码复放。该 fixture 不执行 JS 模块，实际 TS/Vue 执行另验 |
| 当前源码原生模块多消费者 | GREEN，1/1，148.44s | `native-modules-consumers-final-guards.log`；使用真实 .2 SDK guest 与当前 pending/committed 实现。双 pending 中释放一票不抹另一票、重复请求不抹既有 committed、原调用者撤销前后的 dispatch 校验及 recipient 撤销前 reservation 均有实际断言；同一 pipe、exporter/recipient 物理 EOF、无模块 sibling 继续、来源重启与磁盘不变通过 |
| language_contexts 两项 ignored | GREEN，2/2，82.03s | `language-contexts-real-ignored.log`；当前 SDK project-context .1；warm 性能仍用冻结 capability-example .19.0 原摘要，实测 101ms/95ms，未放宽原 5s 预算 |
| 当前 SDK TS .9 + Vue .9 三版本原生混合 | 语义断言走完，整体 FAIL，105.99s | `actual-mixed-current-sdk-first.log`。真实三根、模板补全/hover/定义/引用/诊断原源坐标、唯一诊断和磁盘不变断言通过；最终 Shutdown 因 TS 未返回声明快照失败，不能称整个联验通过 |
| TS .10 源 Node 完整套 | GREEN，43/43，0 skipped，47.619s | TS 树 `target/s1-010-source-tests.log`；其中 paused SDK 只证明真实 stdio 背压，不替代实 TypeScript 语义 |
| TS .10 实际 ZIP 密封 Node 完整套 | GREEN，43/43，0 skipped，47.379s | TS 树 `target/s1-sealed-0.1.10/sealed-node-tests.log`；使用 `7497e4…2282` 包内 Node、服务与 TypeScript，仅复制测试；服务 ZIP 摘要已核对 |
| 内层编译器背压 | 真实 public stdio RED → GREEN，0.36s → 0.72s | TS 树 `target/s1-engine-pressure-{red,green}.log`。测试独立 SDK 在初始化后确认暂停 stdin；8 MiB 文档之后的无编译器请求也必须等待，恢复后验证 open/change/close/reopen 顺序。生产采用有界队列、管道写入完成和 30s 输入期限 |
| 模块准入失败释放回执 | 真实 TS 进程 RED → GREEN，1/1，1.36s | TS 树 `target/s1-import-receipt-{red,green}.log`。严格 error.data `{version:1,imports:"released"}`；前置拒绝或等待候选 child 物理关闭后才发，既有完整 composition 保持健康。宿主匹配回执后释放本次导入的实际结果见下方 `module-imports` 行 |
| 共享关闭覆盖层 | 目标 RED → GREEN，24.75s → 25.61s | `shared-close-{red-with-fixture,green}.log`；关闭共享源后，另两个 context 原来仍返回 discarded draft，修复后恢复 saved；open/close/reopen 全 wire 生命周期与新 incarnation 内容均通过。只广播关闭路由，复用原有 lifetime 校验 |
| 原调用者 workspace.read | 目标 RED → GREEN，68.16s → 84.86s | `language-context-permissions-{red,green}.log`；相同真实 .2 SDK guest，snapshot=false 对照成功，snapshot=true 无 EditorRequest 且结构化 PermissionDenied。只补 owner 与原调用者门禁，不靠提供者权限代授 |
| 最终 SDK TS .10 + Vue .10 原生混合 | GREEN，1/1，98.33s | `actual-mixed-final-010.log`；三种固定 Vue 与 TS 5.9.3 的五项模板语义、原始位置、唯一诊断、磁盘不变、三独立 TS 进程及 checked Shutdown 全部通过。测试关闭接收改为不跨 OS 线程唤醒 GPUI 测试调度器的 `try_recv`，没有放宽期限或忽略错误 |
| TS 独立七项能力、无 TS 结构化拒绝 | GREEN，2 项 | `typescript-final-010-package-suite.log` 的第 1、4 项；有配置 TS 与无配置 JS、停用 Vue 后补全/诊断/悬浮/定义/引用/重命名/格式化及磁盘不变均通过。同一轮另两项因旧 consumer .1 缺 Snapshot 失败，完整套件不是 GREEN |
| 未知消费者 SDK 错误、公开协作 | GREEN，2/2，230.12s / 56.50s | `typescript-final-consumer-012.log`；consumer .2 仅补声明的空 Snapshot 后，缺失/错误显式路径/不兼容 SDK 的三种拒绝，以及 resolve/describe/analyze/release、引擎/七项能力、原生身份与释放后失效全部通过 checked Shutdown |
| 显式拒绝模块只释放本次导入 | 真实 SDK → Manager → app RED → GREEN，1/1，110.44s → 116.45s | `module-imports-{red,green}.log`；严格匹配 JSON-RPC `error.data` 释放回执，不影响既有授权、健康原 Arc 或后续 barrier；`initialize` 仍一次 |
| 最后一个 pending peer 的授权回收 | 目标 RED → GREEN，1/1，162.22s → 168.81s | `native-modules-last-peer-{red,green}.log`；同一 .2 SDK guest、来源、原生响应和断言。A 已接受、B 待确认，A 退役后再引入/释放 C，旧逻辑会丢失 A 的 committed 证据；修复保留所有 committed/pending 记录，最后 B 释放后先封闭启动，再锁外物理回收，同一 stdout 已观察到 EOF，B 与 exporter 仍存活 |
| 过大原生错误响应的失败历史 | 真实 RED 115.87s；修复首复放仍 FAIL 108.77s | `control-error-budget-red.log` 首次为 `OperationFailed` 而非 `LimitExceeded`；修复后原请求的 typed limit 和序列化字节断言通过，但 `final-app-package-regressions.log` 在 SDK 回执观察失败。夹具只发布 contributions，warm 路径未驱动 `Manager::poll`；随后仅修观察循环，保持原请求 15s 与原观察界限 20s，未降低断言；最终 GREEN 另列 |
| 最终非 UI workspace 门禁 | GREEN，318 passed、322 ignored，82 suites，exit 0 | `workspace-non-ui-final2.log`；包含最后 pending peer 修复后的当前 runtime。编译 7m08s，ignored 没有算为通过；第一次 final 尝试的 E0583 是新测试 mod path 缺失，已修正并实际复放 |
| 最终 app / 普通 bin / workspace check | GREEN，1m47s / 1m19s / 14.68s | `control-error-budget-green-build.log`、`editor-final-bin-build.log`、`workspace-check-final.log`；包含生产错误预算修复。随后仅调整测试观察与夹具，app 再编译 GREEN 2m15s，`final-observer-harness-build.log` |
| 当前宿主受影响普通测试 | GREEN，44 passed、24 ignored | `app-affected-final.log` 的 `language::`；另两个过滤分别 0 tests / 0 passed 14 ignored，不计为通过。适用 ignored 单独执行 |
| 最终宿主 strict ACK / control / Unicode / 关联源 | GREEN，4/4，0.38s / 0.13s / 0.28s / 0.08s | `host-control-final.log`；最终生产修复后重复针对性检查，通过真实 framed 子进程与原生 Base 坐标 |
| 无释放回执的业务错误 | GREEN，1/1，109.21s | `final-app-package-regressions.log`；错误和 barrier 保持同 Arc / 一次 initialize；无回执不能推断 native 已释放模块，来源禁用后原 plan 退休 |
| 陌生语言子项目原生标签 | GREEN，1/1，24.32s | 同日志；当前 SDK project-context .1，经公开包/Manager 分离两根、进程与未保存输入，不向其他项目同步文本 |
| 最终 Vue grammar 原生 loader | GREEN，1/1，0.52s | 同日志；最终 Vue .10 ZIP 的 grammar/query 经生产 `WasmStore` 和 readonly parser，普通 SFC 无 parse error |
| Windows 实际调度第一轮 | FAIL，131.47s；生产 1000ms 超时实测 1.0097467s | `native-control-windows-first.log`；第一轮不算全闭环通过。测试原来只声明包权限，模块 fixture 两个 control 方法缺 `native.modules` 方法权限；随后仅补准确声明与阶段/真实 SDK 返回诊断，所有期限和断言不变，最终 GREEN 另列 |
| 过大原生错误最终复放 | GREEN，1/1，108.90s | `control-budget-native-final.log`；原请求与真实独立 SDK 回执均为 `LimitExceeded` 且序列化 UTF-8 Failure <=64 KiB；禁用被拒绝的来源后，原 Arc / 健康 barrier / initialize=1 保持。生产 SHA `6f21ef324cf4bf0adec42a9279990fd29237de25c93ec2aed00d711dd39abaf1`，观察测试 SHA `92462152e875b355f503c444f3c5b12a0954b46623e2184c0861282be415cb07` |
| Windows 实际调度最终复放 | GREEN，parent 1/1 115.79s，child 1/1 115.71s，exit 0 | 同日志；普通与带导入的原始 1000ms 请求分别实测 1.0129059s / 1.0238475s，实际 wire 对应取消、同 Arc / 一次 initialize / 后续 barrier、关闭 pending 原身份得到 stale、同路径重开新文档/新进程，以及未知 native 导入保留撤销权全部通过；checked Shutdown 与仅自己创建的 HWND 实际销毁通过。父/子不是两个独立场景，不重复计数 |
| 普通原生桌面 / 最终 .10/.10 | 本轮相关可见交互通过，正常退出 0 | 私有 `target/s1-native-ui/editor-app.exe`，SHA-256 `c76e2851e9b3ea6947414a17728b27d7836ee6dd99fd1db63e19d677ca3ef701`，`--profile` 指向本轮成功联验留存的 `s1-verified-T7uGKa`；Sky 仅操作该窗口。三个 Vue 标签实际 F12 均跳到各自原源，Vue 3 引用列表为 `App.vue:3:7` 与 `:5:26`；Vue 2 中文/表情后的模板跳转正确。中文/表情临时注释输入和 Undo 的可见文本通过，按不保存关闭，三个磁盘摘要前后一致。没有把该输入当作中文 IME 组合态验收 |

TS .10 首份当前 SDK 候选：`C:/Projects/RustProjects/TypeScript-for-Nanobug/target/s1-statefix-010/typescript-0.1.10.zip`，SHA-256 `7497e4f932904690608107834110a8839c3120565e71b956152ad19435792282`；内嵌服务摘要 `33764ce30d0b2101df7c1c07f4edb96411939ac928c723cc6d595bdb449179aa`。公开宿主打包前已核对源 workspace 和私有短 target。包含空 Snapshot 修复，但尚待实际混合 Shutdown 复验；后续最终 SDK 封包不得冒用此摘要。

全量 `cargo fmt --check` 首轮实际失败，只有外部冻结基线的 `tests/project_windows/geometry.rs` 与 `ui/controls.rs` 格式差异，记录于 `fmt-check-first.log`。没有将针对性 rustfmt 通过表述为全量通过，也没有顺手修改这些他人改动。

最终 `cargo fmt --check` 仍仅这两个外部文件失败，`fmt-check-final.log`。相对冻结基线的全部 117 个 S1 Rust 文件 `rustfmt --check --edition 2024 --config skip_children=true` 通过，`fmt-s1-final.log`；TS/Vue 两个独立仓库的 `cargo fmt --check`、`git diff --check` 也通过。全工作树 `git diff --check` 的唯一空白错误仍为外部 `ui/controls.rs:54`，`diff-check-final.log`；LF/CRLF 警告不当作源码变更。全量检查不是 GREEN。

## 早期断点结果（历史保留）

下表仅适用于新集成树和对应日志，不替代 workspace 门禁或真实 Vue/TS 原生联合验收。

| 检查 | 实际结果 | 证据与限制 |
| --- | --- | --- |
| `cargo check -p plugin-runtime --tests -j1` | PASS，3m47s | `target/s1-resume-2026-10-19/runtime-check-first.log`；不包含 editor-app 编译 |
| runtime 五组定向测试 | 18 passed、12 ignored，0 failed | `runtime-targeted-tests.log`；language_contexts 11+2 ignored、language_declarative 3、language_services 1、native_modules 2+1 ignored、plugin_services 1+9 ignored |
| `cargo test -p plugin-protocol -j1` | 64 passed；doc 0 | `protocol-tests.log`；58 unit + cloud/commands/interaction 各 2 |
| `cargo build -p plugin-runtime --example lsp_fixture -j1` | PASS，14.55s | `lsp-fixture-build.log`；与当前测试 profile 一致 |
| `independent_guest_delegates_a_native_plan_and_retires_loaded_modules --ignored --nocapture` | 1 passed，80.25s | `native-modules-lifecycle-first.log`；使用新源码通过公开 SDK 打包的 `fixtures/language-delegate-fixture-0.1.0.zip`。只证明真实 SDK 模块授权、租约、错误 context 拒绝和撤销后的 process resources=0。该 fixture 启动参数缺 wire log，child 可能已自行退出；`process_count` 只计归属租约，不能证明健康 stdio 往返或物理进程终止。更强的多消费者/实际 framed 往返验证待运行 |
| TS Node 源套件 | 41 passed、0 skipped，50.402s | 独立 TS 树 `target/s1-control-full-node.log`；之后新增了一项测试，不能用该 41 项结果覆盖新增项 |
| 新增只读控制结果溢出后的 transport 文件复验 | 4 passed，2.358s | 根代理工具回执；尚无独立保存日志。包含新增用例，不能与上一行相加当成完整 42 项复跑 |
| TS 0.1.9 实际 ZIP 解包后的密封套件 | 42 passed、0 failed、0 skipped，46.803s | TS 树 `target/s1-sealed-0.1.9/sealed-node-tests.log`；使用包内 Node 和 `native/**/node_modules/TypeScript`，仅复制公开测试，不从源码 `native/` 借实现。它是当前 42 项整套证据，仍不等于宿主联验 |
| TS 控制握手、版本拒绝 | 两项分别 RED → GREEN；无 marker 的普通 LSP 另 1 passed | TS 树 `target/s1-control-handshake-{red,green}.log`、`target/s1-control-version-{red,green}.log`；宿主 strict ACK 回归尚未执行 |
| 实际 Vue 兼容策略查询的首次复现 | RED：0 passed、1 failed，130.85s | 新树 `vue-policy-red.log`；`Unsupported Vue analysis feature`。该失败保留；修复复放见下一行，不把进程启动当分析通过 |
| 上述策略查询的 step1 修复复放 | GREEN：1 passed，127.68s | 新树 `vue-policy-step1-green.log`；使用 Vue `target/policy-step1/vue-0.1.9.zip`、同一测试程序和两个策略 fixture。只证明正常 Vue 2.7 策略报告及 attach=0，不是最终 Vue ZIP、完整 AC4 或真实 TS 语义验收 |
| editor-app S1 定向静态检查 | rustfmt 解析/格式检查、`git diff --check` 通过；组件调用检索无命中 | 只检查本轮文件；S1 app 测试尚未编译通过或实际运行，不能据此标记 AC 完成 |
| 首轮 editor-app 测试程序编译 | RED：7 个编译错误 | `editor-app-first-build.log`；helper 误把可见 `DiagnosticEntry.range` 的 byte range 当成 line/column 导致 6 项，既有 scene probe 未穷尽两个新 `EditorValue` 变体导致 1 项。均已写修复，尚未据本记录复编通过；没有改生产诊断范围以迁就测试 |

整个新工作树的 `git diff --check` 已实际失败：外部基线 `crates/editor-app/src/ui/controls.rs:54` 的 `diagnostic_popup` 重导出行有尾部空格。该文件在冻结账本、新集成树和只读原 main 的摘要相同；本轮限定文件检查通过不能表述为全工作树通过。本轮保留这项基线限制，没有顺手修改他人 UI WIP。

runtime 定向原始命令：

```powershell
cargo test -p plugin-runtime --test language_contexts --test native_modules --test language_declarative --test language_services --test plugin_services -j1 -- --nocapture
```

TS Node 41 项完整测试与新增 transport 文件复验的原始入口如下；Node 使用现有绝对路径，不依赖浮动 PATH，也未安装新工具链。

```powershell
$env:NANOBUG_VUE_FIXTURES = 'C:/Projects/RustProjects/Vue-for-Nanobug'
& 'C:/Users/Tmiracle/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe' --test --test-concurrency=1 test/*.test.mjs
& 'C:/Users/Tmiracle/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe' --test --test-concurrency=1 test/transport.test.mjs
```

首次嵌套 SDK cache target 的 LINK1181 已改用宿主公开短 cache 打包入口完成 fixture 包构建；没有为该环境路径故障修改宿主功能。它不证明其他候选或宿主 UI 已构建通过。

## 原调用者权限与模块回执的复验入口

以下命令均在新 S1 集成树运行。执行前 `cargo metadata --no-deps --format-version 1` 已确认 workspace 与 target 均在新树；原 main 与旧任务树没有参与构建。命令中的测试程序来自对应 `--no-run`，不借其他工作树产物。

```powershell
cargo test -p plugin-runtime --test language_contexts --no-run -j1
$env:NANOBUG_NATIVE_CONSUMERS_PACKAGE = (Resolve-Path -LiteralPath target/s1-resume-2026-10-19/fixtures/native-consumers-fixture-0.1.2.zip).Path
& ./target/debug/deps/language_contexts-a9c5dc70b3a1185c.exe related_snapshot_requires_the_original_callers_workspace_read --ignored --nocapture

cargo test -p plugin-runtime --test native_modules --no-run -j1
& ./target/debug/deps/native_modules-9cb1557223d3c540.exe releasing_the_last_pending_peer_reaps_code_from_a_retired_committed_consumer --ignored --nocapture

# 冻结当前测试程序，避免与另一个串行 Cargo 门禁的后续链接竞争。
New-Item -ItemType Directory -Path target/s1-native-module-final/deps -Force | Out-Null
Copy-Item -LiteralPath target/debug/deps/native_modules-9cb1557223d3c540.exe -Destination target/s1-native-module-final/deps/native_modules.exe
Get-ChildItem -LiteralPath target/debug/deps -Filter '*.dll' -File | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination target/s1-native-module-final/deps }
& ./target/s1-native-module-final/deps/native_modules.exe original_consumers_revoke_independently_and_both_module_ends_reap_actual_workers --ignored --nocapture
```

权限用例的相同 guest 先验证不请求 snapshot 的对照确实能完成，再验证 snapshot=true 在原调用链没有 `workspace.read` 时不能产生任何 EditorRequest；测试宿主不伪造权限错误。RED/GREEN 分别保存在 `language-context-permissions-{red,green}.log`。独立 .2 guest 的 SHA-256 为 `12b7387b0757bf010e741edaffcc3c0133cad70f58285eace13d18bc1b195c4c`，包含真实声明的 schema 1 Snapshot 分支；不以 Snapshot 缺失掩盖关闭失败。

最后 peer 用例的首次编译因测试子模块路径 E0583 失败，随后用显式 `#[path = "consumers/release.rs"]` 修正；该编译失败不是目标 RED。定向构建日志为 `native-modules-last-peer-red-build.log`（6.16s）和 `native-modules-last-peer-green-build.log`（18.37s）。目标 RED 在收到两个实际、匹配请求 ID 的 `fixture/refuse` 严格回执后，因原 plan 仍 active 失败；GREEN 除 inactive 外还要求 `process_count == 0` 及原 stdout 实际 EOF，最后才执行 Manager shutdown。夹具不执行 CJS 代码；这一回归证明真实 SDK 授权、pending/committed 生命周期与 native stdio 退出，模块业务执行仍由真实 TS/Vue 进程验收。

多消费者当前源码复放的私有测试程序 SHA-256 为 `84ce62047932b85729325fcce891715782cdcd35e7058d6de4a820312ec38e43`。其 fixture 路径由编译时 `CARGO_MANIFEST_DIR` 指向新树默认 `target/debug/examples/lsp_fixture.exe`，不会因复制测试程序或 `examples` 目录而改变；该实际 fixture 在运行前后均为 `e5549018bf119c11bace17bd1f2c2106177e5f90a3f5e8582449175731cbdc4b`。运行期间没有重新链接默认 fixture，私有测试程序不阻塞 Cargo 链接。新日志 `native-modules-consumers-final-guards.log` 记录 1/1 PASS、148.44s；旧 153.29s 与旧 lifecycle 缺 argv 日志的历史局限保留，不以本次结果重写旧用例身份。

`NativeModuleAttachment` 的 reserve 阶段不注册进程依赖；dispatch 重新验证原始 owner/consumer/context。匹配的严格 `error.data={"version":1,"imports":"released"}` 仅释放本次票据，已有成功使用及其他 pending 仍保留。没有回执的错误、取消和超时保守保留可能已加载代码的授权。回执仅表示本次新增导入没有被任何仍活的受管 worker 保留，不能解释为代码从未执行或外部副作用已回滚。64 条模块配额继续明确拒绝新增，不以删除仍有 committed/pending 义务的记录腾出配额。

GPUI 的确定性 TestScheduler 不能用于证明独立线程上的同步 IO 与 1000ms 取消并行；已移除原 GPUI timeout 分支，普通业务错误保留 GPUI 验证。超时不确定导入使用 `extensions/worker/native_language_control_tests/module_timeout.rs` 中真实 Windows OS 调度的父/子测试，等待该用例实际结果后再更新状态；不能把该用例的编译通过算作 GREEN。

## 候选身份

- TypeScript 独立树 `C:/Projects/RustProjects/TypeScript-for-Nanobug`，origin 为 `https://github.com/Nanobug-Editor/TypeScript.git`。最终 SDK 公开打包的 `target/s1-final-010/typescript-0.1.10.zip` 与首次生成的 `dist/typescript-0.1.10.zip` 相同，40,917,920 bytes、22 entries，SHA-256 `fbc363cb36e67f5583449628b428ed77f2dbcb0fe9afe759ae0c18b15743cff5`；SDK 为 `76a7a14a0bc46d4dc9573cc5342edfda352c805ba4f780e1f9b1b493a2ccdd12`，服务 ZIP 为 `33764ce30d0b2101df7c1c07f4edb96411939ac928c723cc6d595bdb449179aa`。与已通过密封 43 项套件的首份 .10 包相比，22 entries 中只有 `typescript.wasm` 因最终 SDK 变化；全部 native 服务字节相同。最终包已通过上述混合、独立 TS 和 consumer .2 实际宿主用例。旧 .9 包及其密封 42 项证据原样保留。
- Vue 独立树 `C:/Projects/RustProjects/Vue-for-Nanobug`，origin 为 `https://github.com/Nanobug-Editor/Vue.git`。最终 SDK 公开构建的 `target/s1-final-020/vue-0.1.10.zip` 与首次 `dist/vue-0.1.10.zip` 一致，4,108,507 bytes、39 entries，SHA-256 `59cc1e3ae71c5e0897df096eeec88269ff8f5a6433af0e1448c931dcc8f39ee4`；SDK 缓存 `76a7a14a0bc46d4dc9573cc5342edfda352c805ba4f780e1f9b1b493a2ccdd12`。该新 ZIP 与同 SDK 的 TS `.10` 通过真实三版本混合项目原生联验，1/1 GREEN、98.33s，日志 `actual-mixed-final-010.log`。保留的 `.9` 包 `target/s1-final-metadata/vue-0.1.9.zip` 为 4,105,763 bytes、39 entries、`2b409709294d7c89da16862a4811b63da130a8d386f4aef3f8a1378a94a0c667`，与其 dist 副本完全相同；其源码/封印模块各 84 项与实际 Rust 策略矩阵通过。`.10` 的三个 Rust 策略源文件、兼容 records/ABI 和 11 个模块条目与 `.9` 相同，仅版本/验证文档元数据及 SDK 编译 WASM 变化；`.9` 的完整策略矩阵仍按原包身份保留。
- 保留 0.1.8 等历史 ZIP；不覆盖旧包，不以源码版本号代替实际产物身份。两个独立仓库尚未按本轮 S1 完成提交推送或发布。

新树公开 SDK 夹具分别为 `target/s1-resume-2026-10-19/fixtures/project-context-fixture-0.1.0.zip`（SHA-256 `1C63DD815C860F2B1500280FF323240E8F29FADB96C45564EC3F82CD648AF9F3`）与 `language-delegate-fixture-0.1.0.zip`（`896B136CBFC5E5FB6FD02E1EA054B3BCC9B00FACBF48D481906820A1D5CCEE36`）。夹具包身份不等于尚未执行的 app 测试通过。

多消费者 fixture 的旧 .0 包及其 `native-consumers-package.log` 保留。当前 .2 为 `fixtures/native-consumers-fixture-0.1.2.zip`，SHA-256 `12b7387b0757bf010e741edaffcc3c0133cad70f58285eace13d18bc1b195c4c`；当前 SDK scope guest 为 `fixtures/current-sdk/capability-example-0.19.2.zip`，`e30b4ab33ff124157232b8c8429d265615001b269325767b0aaeab3ef5e650e3`。独立消费者授权、真实 framed 往返与双方物理退出已通过上表用例；fixture 不执行适配模块代码，真实执行由 TS/Vue 混合联验补足。未知 TS consumer .2 是独立仓库 fixture，`target/s1-final-consumer/unknown-project-consumer-0.1.2.zip`，380,919 bytes，`63f4ad7c7e78c7c5e33f929441f17e62c4c16ab5d97c321d80581635ae159d6c`。

## Vue 生产兼容策略（S1）

Vue 0.1.9 的生产 `src/evidence.rs` 通过授权工作区句柄收集声明、实际安装、npm lock、pnpm 所属 importer 与显式目标，返回有界来源、后端和 TS 上下文/引擎元数据。`vue.targetVersion` 是宿主确认的用户/项目配置，不接受调用方伪造；冲突不被覆盖。公开 `compatibility` 查询在附加原生模块前完成，严格分析拒绝未知、未验证、缺依赖或冲突组合。配置重启需要的快照只含空逻辑标记，不保存句柄或待处理调用。

0.1.9 使用新集成宿主公开 `--plugin-package` 和 SDK 缓存 `ad5871af74fcdc33ebd0274c28459444e20ee5594661eec045173b79dd75a3a8` 构建。`workspace.files` 最低声明修正为实际所需的 `>=1.3,<2`，没有为错误的 1.4 下限迁入 S2 事务。独立重打到 `target/s1-final-metadata/` 的 39 entries 与冻结 dist 逐项相同；对比 `.8` 时仅 WASM、manifest、plugin.toml、compatibility 账本和双语 README 共六项变化，11 个 `modules/**` 条目及所有 grammar/query/license 原样保留。账本位于 Vue 树 `target/s1-final-metadata-entry-comparison.json`、`target/policy-018-to-019-entry-comparison.json`。

本轮对固定 0.1.9 输入实际完成以下验证：

- 源码全集：`node --test --test-concurrency=1 tests/*.test.cjs`，84 passed、0 failed/skipped/cancelled，89.35s；Vue 树 `target/source-019.log`。
- 封印模块全集：同一套 84 项，设置 `NANOBUG_TEST_EXPORTS=1` 和真实 TypeScript 插件 `native/engine-bootstrap.cjs`，84 passed、0 failed/skipped/cancelled，84.64s；`target/sealed-019.log`。运行前对测试读取的全部 11 个模块文件与最终 ZIP 逐 entry 对比，全部相同，见 `target/sealed-019-input-comparison.json`。这两套不是 168 个独立场景，其中 Node resolver 探针不充当 Rust 策略证据。
- 新宿主 `vue_compatibility` 集成用例：真实 Vue Package → Manager → 公开 SDK 消费者调用，最终 1 passed、0 failed/ignored，206.43s；`target/s1-resume-2026-10-19/vue-policy-final-019-fixture-fixed-matrix.log`。覆盖正常选择、显式来源、已安装/声明/lock 冲突、无确切证据、未验收历史版本不回退、scoped alias、非法设置回滚、不兼容 TS 元数据、规范根、合法内部 junction、拒绝越界 junction、零 attach 及原文件/元数据不变。

该集成矩阵的受控提供者只返回 TS 策略元数据并拒绝 attach，不运行真实 TS 5.9.3。两个独立 SDK 0.2.0 固定夹具为 `tests/fixtures/compatibility/dist/controlled-typescript-context-0.1.0.zip`（`f1cd35f69cea9911e550b1b82c39f688342c4505609652a15452f82aa57773c9`）及 `unknown-vue-policy-consumer-0.1.0.zip`（`0783054083629f19009a041ef55a8f2460ef8febc21da15b7c9c7d5f1e09ea32`）。它证明生产 Rust 选择与公开契约，不代替真实 TS 项目发现、三版本混合语义、原生 UI 或 S2–S6 验收。

RED/GREEN 原始日志保留。`.8` 的首次策略 RED 使用仅更正 `workspace.files` 下限的测试副本，原 WASM 与其余条目不变，实际失败为 `Unsupported Vue analysis feature`（130.85s），不是准入失败。正常报告 step1 GREEN 为 127.68s；加入显式设置后先出现 `Unknown setting`，补实现后又暴露 `Plugin did not return its declared state`，修复逻辑快照后 step3 GREEN 为 161.13s。完整最终矩阵首次 186.63s 失败来自测试 junction：PowerShell 接受 `\\?\` 目标但生成原生不可读链接；独立原生读取对照确认根因后只修夹具构造，生产 ZIP 不变，完整复放 206.43s GREEN。诊断和各冻结输入详见 Vue 树 `target/policy-junction-diagnosis.md`、`target/policy-019-handoff.md`。

0.1.10 的双语 README 和 `compatibility.json` 将上述证据明确归属 `.9`。三个 Rust 源文件、兼容 records/ABI 及全部模块条目已与 `.9` 复核相同，见 Vue 树 `target/policy-020-source-inheritance.json`。新宿主的 SDK `76a7a14a0bc46d4dc9573cc5342edfda352c805ba4f780e1f9b1b493a2ccdd12` 经公开入口完成 release 打包（33.83s），随后首次生成 dist 副本；两份新包均为上述 `59cc1e...`。对比 `.9`，39 entries 中只有 WASM、manifest/plugin 版本、compatibility 验证账本和双语 README 共六项变化，11 个模块条目不变，账本见 `target/policy-020-package-comparison.json`。全部旧 ZIP 保留。

最终 `.10` 新包的真实冒烟由 `actual-mixed-final-010.log` 单独记录：上述 Vue `59cc1e...` 与 TypeScript `target/s1-final-010/typescript-0.1.10.zip`（SHA-256 `fbc363cb36e67f5583449628b428ed77f2dbcb0fe9afe759ae0c18b15743cff5`）在同一最终 SDK 宿主中完成真实三版本项目、原生语义与 Shutdown 全闭环，1 passed、0 failed，98.33s。该结果解决早期 TS 缺少逻辑 Snapshot 的整体验证失败；它不把 `.9` 的策略矩阵改记成 `.10` 重跑，也不豁免宿主门禁与 S1 双轴审查。

## S1 验收与测试索引

以下按首次冻结基线上的实际执行区分已通过行为和当时剩余门禁；正式基线上的复验与交付结果由最新记录补充，不改写旧日志。

| 验收项 | 当前证据 / 测试入口 | 本轮待验内容 |
| --- | --- | --- |
| AC1 身份、revision、元数据与初始化 | 当前宿主真实 control/关联快照回归、最终三版本 Worker 与 consumer .2 已通过；原生版本 37 不猜 wire 1，伪造元数据拒绝、source vector 失效和 resolve/attach 自身固定点有实际断言；真实 Windows 关闭/重开已通过 | 独立 Git 基线与交付门禁仍待解决 |
| AC2 TS 独立使用 | 最终 .10 实际包的有配置 TS、推断 JS、停用 Vue 后七项能力、引擎/配置记录和磁盘不变通过 | 无新增行为缺口；整单交付仍受下述前置限制 |
| AC3 三种 Vue 混合项目 | 最终 .10/.10 production Worker 联验 98.33s GREEN；固定 Vue 2.6.14/2.7.16/3.5.22 + TS 5.9.3，模板五项语义及真实源位置、无重复/虚拟/跨项目诊断、三独立进程和 Shutdown；最终 grammar 0.52s 与普通桌面原生交互通过 | 无新增行为缺口；整单交付仍受下述前置限制 |
| AC4 选择、项目根与绑定 | Vue .9 生产 Rust 完整策略矩阵 206.43s GREEN，.10 对应策略源/records 不变；提供者候选与显式选择回归通过；真实三根绑定隔离、当前陌生语言标签路由与原生三标签跳转均通过 | 独立 Git 基线与交付门禁仍待解决 |
| AC5 公开协作及错误 | 最终 TS .10 + consumer .2 两项 GREEN；缺失 TS 对照通过；原生身份封印、七项能力与 release 后失效均从公开契约观察；独立双轴源码核对通过 | 独立 Git 基线与交付门禁仍待解决 |
| AC6 模块权限与隔离 | 当前源码/.2 guest 多消费者 148.44s GREEN；真实 TS 执行 Vue 模块及不兼容版本拒绝；原调用者权限、严格回执、最后 pending peer 实际 EOF 均 RED→GREEN；无回执业务错误、真实 OS 超时未知导入及有界错误回执均通过 | 独立 Git 基线与交付门禁仍待解决 |
| AC7 通用性及不写项目 | 未知语言、消费者、模块 fixture 已实际使用；最终 TS/Vue/consumer 与桌面三文件磁盘不变通过；Standards/Spec 未见专属宿主分支、权限转授、全局安装或项目缓存写入；原 main 和旧任务树由本任务保持只读 | 独立 Git 基线与交付门禁仍待解决 |
| AC8 控制协议及预算 | strict ACK、业务错误/取消复用、关联源变化、大正文分离、真实 stdio 背压、错误 64 KiB 上限及真实 OS 超时/关闭/未知导入全部 GREEN | 独立 Git 基线与交付门禁仍待解决 |

`definition_provider_preserves_base_utf8_offset_after_unicode` 已完成 RED→GREEN，见上表。GPUI 确定性调度器不能代表阻塞 stdio 时的 OS 线程：新增的超时/关闭/未知导入验收使用独立受控 child 创建真实 WindowsPlatform/D3D11 和私有原生窗口，没有调整生产期限或测试调度器来迁就结果。

## 首轮交付检查与后续衔接

完整 app 测试程序、普通 bin、最终非 UI workspace 和 workspace check 已通过；最终混合、独立 TS、公开消费者、策略、本轮相关桌面交互及最后的生命周期/预算回归均通过。双轴审查的问题均已修复，预算实现另外经独立 Spec 与 runtime Standards 复核；最终观察循环也经独立增量审读。只读报告分别在 `target/s1-standards-audit/standards-review.md`、`target/s1-standards-audit/budget-independent-review.md`、`target/s1-spec-audit/final-review.md`。报告时点尚待执行的事项由上表实际后续日志覆盖，不能倒改历史为早已通过。

本轮最终测试副本 `target/s1-acceptance-finish/deps/editor_app.exe` SHA-256 为 `ebbb4c98bbf73aa98fb0a7d333b7fc76b5c7f661a068167263906ba37832f2ff`，匹配的 `examples/lsp_fixture.exe` 为 `e5549018bf119c11bace17bd1f2c2106177e5f90a3f5e8582449175731cbdc4b`。两份固定副本保留，后续 Cargo 链接不覆盖这些验收输入。

首轮因 main 的 `eace014` 尚不含本地 UI/主题/highlighter 接口而停止交付，冻结副本用于保留已完成实现与验收，没有把其他任务的 256 条 WIP 整批并作 S1。用户随后确认前置已提交；实际读回本地与远程 main 均为 `b59f5ac`，仅余一个与 S1 无关的 Excalidraw 改动。新集成树保留这个正式父提交，并复跑门禁；此前两处格式错误已在用户提交内修正，不作为 S1 的修复成果。

最终只提交已验证且属于 #121 的差异。普通提交、推送与关闭授权继续有效；先核对正式基线上的验收及双轴审查，再推送并读回 #121 状态。本轮不关闭其他工单，不创建 Release。
