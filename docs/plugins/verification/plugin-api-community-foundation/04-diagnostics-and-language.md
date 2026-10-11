# 04：多来源诊断与共享语言结果验收

日期：2026-10-11。议题：[#96](https://github.com/T-miracle/Nanobug/issues/96)。状态：联合实现及两个独立审查轴已收口，最终 SDK 和六个独立包已构建；物理验收发现的 Problems 键盘缺陷已补真实回归并修复，新宿主、增量审查及剩余验收进行中，尚未交付，不关闭议题。

依据：[已批准工单](../../tickets/plugin-api-community-foundation/04-diagnostics-and-language.md)、[共同完成条件](../../tickets/plugin-api-community-foundation/README.md)、[社区基础能力方案](../../specs/plugin-api-community-foundation.md)与[平台规格](../../specs/plugin-api-platform.md)。Windows 物理桌面验收由根代理独占；本文件区分实际包/GPUI 自动测试与桌面观察。

## 输入与边界

- 独立工作树 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-language/Editor`，分支 `codex/plugin-api-language`，起点 `3aa9177a20b428dbac5ebffbd0aa78e34a464eda`。原项目目录已有改动不计为本单产物。
- Rust/Cargo `1.98.1`，现有 MSVC x64 与 `wasm32-wasip2`；VS Community 18 临时开发环境，`CARGO_TARGET_DIR=../target04`、`CARGO_BUILD_JOBS=2`。不安装额外工具、不调用归档脚本。
- 新的稳定能力 `editor.diagnostics 1.0`；实验能力 `language.direct 0.1`；标准 LSP 新结果属于 `language.lsp 1.4`。实验提供者按语言/特征沿既有选择机制运行，不改变有效单 LSP 选择。
- 只读修复前提 `workspace::Plan.origin` 属于通用 `workspace.files 1.3`。旧无前提消息可以反序列化；发送前提必须协商最低 1.3。它只检查原生文档 incarnation/revision，不授予源或目标读写权限；目标仍经过原有权限、预览、冲突与撤销路径。
- 声明式 LSP 不要求空 WASM。其当前文档动作以真实服务 seal、原 grants 和实际协商的 LSP 1.4 / editor.edit 1.1 / workspace.files 1.3，经同一公开 `Manager.take_editor_requests`、事务预览和 Native Undo。原生发行者限制为当前源的完整 `EditDocument`，不增加文件或跨文档授权；最多 8 个保留批次、32 个待处理请求，最后一个 Native 回执持有者释放对应槽。
- 诊断保存在已有 `DocumentDiagnostics`，原生 Problems 是即时投影；不建立另一份诊断模型、Rope 或撤销栈。两个独立来源的相同位置/消息仍分别显示，旧语法与 LSP 的成对去重保持原行为。
- 独立消费者为 `spell-check`、`config-check`、`expression-language`、`keyvalue-language`，各自首发 0.1.0。前两者没有 LSP；后两者分别使用纯直接回调和受控标准 stdio LSP。四包及受影响的两个事务包已按冻结联合 SDK 实际重建，最终指纹与下文阶段输入分开保存。

## 已取得的逐切片证据

| 检查 | 实际结果 | 说明 |
| --- | --- | --- |
| 公开诊断能力准入 | Red 1 失败 → Green 1/0/0，13.80 秒 | 元数据/权限边界测试；不是实际 WASM 构建证据 |
| `native_lsp_references_preserve_unsaved_utf16_and_single_connection --ignored` | Red 0/1/0，23.30 秒 → Green 1/0/0，22.80 秒 | 真实公开 Manager、组件与受控 stdio；两次引用使用同一个 initialize/didOpen，未保存中文/非 BMP 坐标 |
| 原生引用列表键盘与焦点 | Red 0/1/0，0.01 秒 → Green 1/0/0，0.02 秒 | 真正绘制的 Base 行、Down 焦点与完整 KeyDown/KeyUp Enter 选择；只发 KeyDown 的旧测试不能模拟 Base KeyboardClick |
| 标准共享引用/参数/动作 | Signature Red 0/1/0，23.13 秒；Action Red 0/1/0，28.78 秒 → Green 1/0/0，29.85 秒 | 同一个公开 Manager、组件、真实 stdio、未保存中文/非 BMP 文档；两 overload 与 active override、UTF-16 修改转换和原生源 revision |
| 同连接用户取消 | Green 1/0/0，29.70 秒 | 首请求抵达 wire 后取消，typed Cancelled 与对应 `$/cancelRequest`；原进程继续两次引用、参数、动作，只 initialize/didOpen 一次，退役后进程归零 |
| pending 关联输入期间的新语言意图 | 真正行为 Red 0/1/0，4.26 秒 → Green 1/0/0，9.65 秒 | 真实慢 stdio / `has_pending` 准入；显式取消、A→B→A、真正关闭重开后任何 heading 均不迟到重现，后续 fresh 请求与原输入/Undo 仍有效 |
| 无 WASM 的当前文档修复与原 seal | 行为 Red 0/1/0，0.74 秒；末尾 shutdown Red 0/1/0，1.42 秒 → Green 1/0/0，1.41 秒 | 正式 KV ZIP 公开重检为声明式包；实际 Ctrl+.、预览取消、dirty Apply、原生回执 Undo、引用、参数；同 ID disable/enable 不能复活旧来源；缺 write 为 PermissionDenied，真实未协商 files 为 CapabilityUnavailable；shutdown 即刻 process/seal 清理 |
| 原生服务请求与 Batch 归属 | Green 1/0/0，0.04 秒 | 公开 Manager，无 fake WASM；一次交付、8 批次配额、取消后 transient 回收、最后回执 holder 释放、disable/uninstall/shutdown 撤销已经发表的 Request/Batch；该测试不启动 stdio，不冒充 UI 证据 |
| 三项原 Rename 回归 | 各 1/0/0 | 共享 `document_edits` 解码后的 forms/rejection/versioned overlap；没有重写 Rename 或另建文本/Undo |
| 原生代码动作列表 | Red 0/1/0，0.02 秒 → Green 1/0/0 | Base 名称/焦点/完整键盘选择只返回一次原 Fix 完整计划，不直接写文本 |
| 原生参数卡片 | Red 0/1/0，0.02 秒 → Green 1/0/0，0.02 秒 | 两 overload、signature-local active parameter、惰性纯文本文档、Close/Escape；公开 `test_support()` 只登记实际绘制 role/name，正常构建无测试状态 |
| 原生 Problems 名称与 Escape | Red 0/1/0，0.14 秒 → Green 1/0/0，0.15 秒 | 真正的 Heading、命名本地 Base 按钮与编辑区焦点恢复 |
| 正式新 WASM 生命周期 | Red 0/2/0，25.86 秒 → Green 2/0/0，57.77 秒 | 三个新 guest 最初漏回空逻辑 Snapshot，在 Manager.install 即被严格拒绝；只补 guest `Some(Default)`，未放宽宿主。新包真实 rebuild 后 Direct 3 features/cancel/retire 与诊断 foreign-clear/quota/queued revocation 都通过 |
| 正式诊断 Native 多来源与清理 | Green 1/0/0，54.60 秒 | 两 owner 同位置/消息分别显示，外来集合清理被拒绝，自己的清理与禁用清理有效 |
| 正式修复预览/源前提/Undo | Green 1/0/0，65.16 秒 | 本文档修改与整组 Undo；只改另文件的取消/确认/Undo；预览期间改源、真正关闭重开源均 StaleRevision，无目标副作用 |
| 正式两语言完整 Native 流程 | Red 0/1/0，55.38 秒 → 阶段 Green 1/0/0，66.92 秒 | `.nbexpr` 与 `.nbkv` 都完成实际 Shift+F12 导航、Ctrl+. 动作/分组 Undo、参数卡、provider disable/replacement、A→B→A、真正关闭重开；新文档 ID 强断言保留。仍须最终联合源码/SDK重建后重验，不能沿用阶段 ZIP 充当最终输入 |
| 共享语言 UI / 版本门禁连接 | 最新 UI 3/0/0，0.04 秒；同 stdio 1/0/0，22.72 秒 | 当前服务 seal 与 LSP 1.4 准入后的引用、动作、纯参数卡与同连接取消；没有用旧包版本声明伪造低版本 host 协商 |
| source SDK 出口 | 最新 3/0/0，0.19 秒 | 完整 source-only、缓存修复与并发出口；不等于最终联合 SDK 与四包 WASM/UI 通过 |
| app 编译记录 | 多轮 exit 101 后最新 `--bin editor-app --no-run` exit 0，18.72 秒 | 接口编译错误不计行为 Red；修正 Stateful 身份、公开像素转换、Style trait 和选择器生命周期。保留实际 linker/import 警告，未改变冻结依赖 |

上述 LSP 先例使用只读旧组件包 `C:/Projects/RustProjects/Editor/target/plugin-api-test/capability-example.zip`，协议 7、版本 0.17.1，SHA-256 `EE67E56F0A5186265EAD02CD634155EA4DF938DEB6FC12CAB2D6B9F5F70420CB`，只复制到本树测试夹具目录，没有修改原包。它仅证明宿主接缝，不代替本单四个正式消费者的当前 SDK 构建。

受控 stdio 夹具通过 `cargo build -p plugin-runtime --example lsp_fixture` 直接构建；当前引用/参数/动作及延迟一次引用场景的 EXE SHA-256 为 `1E29C5C8A8229D1B55B13294F2F488BADD7B0747E157CF5E02A80F64538DFD44`。响应与真实取消由上表的实际宿主测试验证。

## 构建阶段输入与失败诊断

初次 source SDK 为 `f9d19b04fcec1cec84726a51d8496102d7664d779229f7b744f89c532a358bbc`，42 个源码/协议入口，Markdown 为零，逐源 hash 与宿主出口一致。第一宿主 EXE SHA-256 为 `F53737D02F52105D3FDBC9DFF3EC6B3AC4F1AA10A321F17A133D3673034C06D9`。它们属于首次切片输入；后续源码注释、原生路由与取消修正需要新的最终宿主和 SDK，不把旧 hash 延用为冻结产物。

Snapshot 修复后的阶段包实际 ZIP SHA-256：

| 独立包 | 版本 | 阶段 ZIP SHA-256 |
| --- | --- | --- |
| spell-check | 0.1.0 | `458EDE2F968C20500DDEB3668EFFC78960BE26778D3E8D19DBE8DC1B871303B0` |
| config-check | 0.1.0 | `BD97EFD58BBC17C5F5EB4AE01C71E7EB0EC2054B44C2947911682B9ADA308D07` |
| expression-language | 0.1.0 | `B4D1DF0998C30C2EC75CF3CDD4DFBEE930B1A2DB7B2A31E37B9CFF585FC74A5C` |
| keyvalue-language | 0.1.0 | `4896A2A3A2020FEE7A78ECEE206AF0F96506D6E551ACFFD4E280D6B137059B51` |

原始失败与重建后的输入分别保存在本树外 `../target04/formal04-before-snapshot-inputs.json` 和 `formal04-inputs.json`，日志保留实际命令/结果。最初 0/2 发生在候选快照检查之前的安装，不是 Direct 回调或集合归属错误。宿主要求每个候选显式返回逻辑状态，纯空状态也必须为 `Some(Default)`；三个新消费者修复后通过，handle/root/request/text 不写入 Snapshot。

两语言旧 close/reopen 测试曾用 tempfile 原始路径，Windows 原生 tab 保存 canonical 路径，导致 `discard_tab` 没命中、重开其实仍是同一 entity。测试改用实际 canonical 路径，必须断言旧 tab 确实移除、新 Document.id 与旧值不同，再观察迟到结果；没有据此错误地改生产 incarnation 逻辑。

后续前置失败与真正缺陷分开记录：慢输入用例第一次无法取得 fresh 卡，声明式包第一次被旧 executable-permission 规则拒绝，均尚未到目标行为。Windows 语言租约使用既有 `navigation::file_uri` 的规范卷标大小写；改用该公开内部接缝后能命中真实 lease。仅给已声明语言服务的无 WASM 包保留必要的 `editor.read/write`、`workspace.read` 授权准入，未开放面板、命令或不受控进程权限。随后取得真正 pending cancel 与无 WASM fix Red，再修 reservation/原文档 ID/原 provider generation、服务发行队列。最后 shutdown Red 在全部 UI/权限流程之后暴露缓存未退役，补入与禁用相同的 Prepared 退役入口后原断言通过。

源码已普通合并已交付的 03/05/06 联合资源基线，并在独立审查修复候选冻结后，用联合 Host/SDK 构建四个正式包及受 `Plan.origin` 影响的 `workspace-scaffold`、`workspace-converter`。合并前的阶段构建与源码出口不充当联合候选的最终输入。

T20 的旧语言回归只读复制既有实际包，不修改原工作区：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/Editor/dist/plugins/javascript.zip`，0.3.0/protocol7，SHA-256 `6B0A98C6397ECF4034C9ACC2AA35069DD7443F8978DAEB0305008C466CCCDE11`；`C:/Projects/RustProjects/Editor/dist/plugins/xml.zip`，0.2.1/protocol7，SHA-256 `96B4E933259FFE3155D1967256DA2D2882C4104EBE0535CF8297F7599D083AB9`。它们是既有语言的回归输入，不冒充本单新 SDK 重建的四个交付包。

## 联合候选与独立审查

03/05/06 已交付基线 `cb1bdb290555cdfc03654e7b01fe9af6a5826293` 已普通合并到 04，固定联合候选为 `5ce40208cfbbd7b114cd38fc7baba3965fc330de`（父提交为 `d792daf` 与 `cb1bdb2`）。联合格式、编译检查、Host 构建和 app 测试 EXE 构建均退出 0；此时 SDK/正式包仍是阶段输入，不能称最终交付。合并保留 live Snapshot 先于 stop、失败后的 fresh WASM/activation 恢复，以及诊断、树/装饰、HTTP/timer/private 的联合能力。

两个未参与 04 实现的独立审查轴及原始报告见[独立审查记录](04-diagnostics-and-language-review.md)。Standards 初轮硬性违反 1（P3 Fix 标题控制字符）、坏味道 0；Spec 初轮缺失或部分 0、范围外 0、行为错误 3（P2 可选 Direct 幽灵角色、跨来源诊断误清除、配置裁剪 Unicode 坐标）。两轴分别保存。固定修复候选 `cc17293e2b3611f2e639472447d0c3606f71f592`（tree `7a2d11454da7ff1a606cd919c243d7f3fc9122f7`）复查已完成：Standards 硬性违反 0、坏味道 0；Spec 缺失或部分 0、范围外 0、行为错误 0。独立报告另以纯文档提交 `d3f04590699138705b0982dbe412fedd67f2f5a2` 保存，不改变生产源码或 SDK 输入。

公开接缝的真实反例命令/日志位于本树外 `../target04/`：

| 审查发现 | 精确命令过滤词 | 真实 Red |
| --- | --- | --- |
| 控制字符动作标题 | `cargo test -p plugin-protocol direct_actions_reject_control_labels_without_accepting_a_valid_prefix -- --nocapture` | 0/1/0，0.00 秒；有效前缀加含换行标题的完整 proposal 仍被接受；`review04-fix-title-red.log` |
| 可选 Direct 角色遮蔽 LSP | app EXE `native_community_language_omitted_optional_direct_keeps_selected_lsp_features --ignored --nocapture --test-threads=1` | 0/1/0，43.58 秒；合法当前包真实省略未来可选能力，原 LSP 已准备且保留有效选择，仍没有引用结果；`review04-optional-direct-red.log` |
| 禁用 A 清除未变 B | app EXE `native_community_language_diagnostics_preserve_independent_owners_and_cleanup --ignored --nocapture --test-threads=1` | 0/1/0，42.99 秒；原同源清理正确后，禁用 A/贡献刷新把同 DocumentVersion 的 B 从 1 清成 0；`review04-independent-owners-red.log` |
| Unicode 路径范围失效 | app EXE `native_community_language_config_trimmed_unicode_path_keeps_exact_diagnostic_span --ignored --nocapture --test-threads=1` | 0/1/0，21.44 秒；`include= 中 ` 的缺文件诊断从期望 1 变成 0；`review04-config-unicode-red.log` |

修复只省略未协商的 Direct 角色、保留仍存活且精确版本匹配的独立来源、在插件内计算裁剪路径的真实起点，并让诊断/LSP/Direct 共用 `Fix::valid`。标题边界 Green 1/0/0，0.00 秒；可选角色的三个实际 LSP 入口 Green 1/0/0，22.02 秒；配置真实包 Unicode/空格/tab 范围 Green 1/0/0，26.03 秒。双来源与权限完整 Green 1/0/0，111.39 秒（`review04-independent-owners-permission-green.log`）：禁用 A、B 原 seal 存活、同版本选择变更、编辑撤销/重发及最终禁用清理，随后实际权限拒绝与无副作用均通过。新权限 probe 首次曾在停用 guest 前置条件失败（0/1/0，90.08 秒）；重装保留先前 disabled 选择是正常行为，夹具补上公共 `enable` 后窄重验通过，没有放宽生产准入。

C02 扩展同一真实包场景：required future Direct 和缺必需 editor.read 的 Package 准入拒绝、合法 Direct 包缺确认 grants 的 Manager 拒绝，均已在可选角色 Green 中实际执行，不伪造协商版本或权限。已准入 checker 缺 editor.diagnostics 的 Create 操作实际返回 typed PermissionDenied，没有新原生请求、诊断或文本副作用，随后 shutdown。四个新包在阶段修复期间从未正式交付，首发保留清单/Cargo 的 0.1.0；两个已交付事务消费者随公开 Plan 源契约适配升为 0.1.1，最终均按新 SDK 实际重建。

为验证已修改的插件领域代码，仅配置检查器先通过当时联合阶段 Host 的公开 `--plugin-package plugins/config-check --output target/community-language` 构建（退出 0，release 23.46 秒；`review04-config-trim-package.log`）。阶段 SDK 为 `d8159187830e6f18e4d4f3b831adba33cb30f148eda348c494729978ad46dc7e`；该包 SHA-256 为 `238942D2793AF1CB2174545CC2F461876D7A9B5E8A9FC45CFD1DEDA59E978698`，旧包保存为 `../target04/config-check-before-trim.zip`。它不充当最终 Fix 源码/联合 SDK 的输入；修复候选冻结后仍统一重建全部四个新包和两个受影响事务包。

## 冻结联合产物

六包与 SDK 的生产输入固定为 `cc17293e2b3611f2e639472447d0c3606f71f592`，记录提交为 `d3f04590699138705b0982dbe412fedd67f2f5a2`。首次联合 Host `C:/Users/Tmiracle/.codex/worktrees/plugin-api-language/target04/debug/editor-app.exe` SHA-256 为 `51800CA225B3ADF6E9AF7F26A1698ACB48EE5331A839594C6D3A9DBDDCBFAD6E`。`cargo build -p editor-app --bin editor-app` 退出 0，35.00 秒；没有为本单升级依赖。后述 Problems 修复只改变宿主 UI，公开 SDK 源和六个包输入均未变化；该 Host 留作首次物理验收的准确输入，不冒充修复后的宿主。

最终 SDK address 为 `3f39ebeff8b29cea5577d7c6407c922c0f0edbc664f95686dfc3ebad514331d6`。公开 `--export-plugin-sdk ../target04/sdk04-final` 退出 0，0.18 秒；49 个源码、Cargo、WIT 与 JSON 文件逐一与 `crates/plugin-protocol` 匹配，Markdown 为零。六个独立项目经公开 `--plugin-package` 实际触发 `--plugin-cargo` 构建，构建日志只出现此唯一 SDK address；未给访客增加仓库 `crates/` path 依赖或 SDK 源码副本。

| 正式包 | 版本 | 最终 ZIP SHA-256 |
| --- | --- | --- |
| spell-check | 0.1.0 | `4A341014D25CB220A0CA4D9B2F4C283825CE054BD6E8BEDDDCC816C6DB0F6568` |
| config-check | 0.1.0 | `4E1706A1F5B9FF6BDE2F17AC70F5E3E795A1E452EA5436502C19D0A79BC34F08` |
| expression-language | 0.1.0 | `6DA3E320240AD32EDCD554962A1283FD63C91DA8C487C75E30C586CA58221024` |
| keyvalue-language | 0.1.0 | `D4C94C7DB6DF4CDBF76A7ABE07D8BA83E772FAACC188EE867578E26A756E1536` |
| workspace-scaffold | 0.1.1 | `6CD6CF38A06519003B7215F9E04492CEFAD59328281AF5961B5E977446C5DAED` |
| workspace-converter | 0.1.1 | `300C1291BA3830F00803EC226AC39EC291819208E1A03C7F3A4FD564AB4B117F` |

四个语言包在 `target/community-language/`，两个事务包在 `target/community-transactions/`。公开命令为 Host `--plugin-package plugins/spell-check plugins/config-check plugins/expression-language plugins/keyvalue-language --output target/community-language`（退出 0，81.42 秒），以及 `--plugin-package plugins/workspace-scaffold plugins/workspace-converter --output target/community-transactions`（退出 0，53.21 秒）。逐文件源 SDK、ZIP、WASM、服务 EXE 与已重检 manifest 的 hash 保存于本树外 `../target04/joint04-final-inputs.json`，实际命令和耗时保存于 `joint04-final-build-results.json`，不把源 manifest 的服务 hash 占位值当作正式包 hash。

物理验收使用仓库外隔离 `../native-qa-04/`。注释齐全的一次性 Native 安装程序只调用公开 `Package`、`Manager.install` 和各包完整声明的授权，不增加仓库 crate、宿主测试 API 或访客 SDK 副本。复制当前 `Cargo.lock` 后，以同一 `target04`、jobs 2 运行 `cargo run --offline --manifest-path ../native-qa-04/setup/Cargo.toml -- <profile绝对路径> <workspace绝对路径> <target/community-language绝对路径>`，退出 0，107.09 秒（编译 34.56 秒）；四个最终 0.1.0 包均 admitted，进程退出时 Manager Drop 已完成。日志为 `joint04-native-qa-setup.log`；此安装步骤不代替物理界面行为验收。

## 冻结版本门禁与自动验收

以下结果均使用上述冻结源码及当前产物；计数顺序为通过/失败/ignored。非 UI 门禁未执行的 ignored 单独列出，后续显式运行的真实包测试不冒充全部 231 项已执行。

| 命令或 app EXE 过滤词 | 实际结果 | 日志（本树外 `../target04/`） |
| --- | --- | --- |
| `cargo fmt --check` | exit 0，2.74 秒 | `joint04-final-fmt.log` |
| `cargo test --workspace --exclude editor-app` | 253/0/231，71 个 result 批次；exit 0，222.37 秒 | `joint04-final-non-ui.log` |
| `cargo check --workspace` | exit 0，15.30 秒 | `joint04-final-check.log` |
| `native_language_features_` | 3/0/0；真实绘制引用/动作按钮、纯文本参数卡与键盘选择 | `joint04-final-language-ui.log` |
| `native_problems_heading_controls_and_escape_restore_focus` | 1/0/0；命名本地 Base 控件与 Escape 焦点恢复 | `joint04-final-problems-ui.log` |
| `sdk_export::tests::` | 4/0/0；联合 source SDK 完整出口、缓存修复与并发发布 | `joint04-final-sdk-source-tests.log` |
| `language::completion::tests::` | 3/0/0；补全排序、替换范围与匹配高亮 | `joint04-final-completion-pure.log` |
| `language::navigation::editing::tests::` | 3/0/0；完整 Rename、过期/外来/混合/注解拒绝、版本重叠拒绝 | `joint04-final-rename-pure.log` |
| `cargo test -p plugin-runtime --test direct_language -- --ignored --nocapture --test-threads=1` | 2/0/0；exit 0，116.81 秒；最终包真实 Direct 与独立诊断归属、取消和退役 | `joint04-final-language-runtime.log` |
| runtime `workspace_transaction_roots_reject_escape_private_peer_and_revoked_authority --ignored` | 1/0/0；exit 0，91.21 秒；当前公开包的 root、私有/同伴边界与撤权 | `joint04-final-workspace-roots.log` |
| runtime `workspace_transaction_missing_write_grants_and_unsupported_required_version_have_no_effects --ignored` | 1/0/0；exit 0，30.38 秒；缺写权限和未来必要版本均无副作用 | `joint04-final-workspace-grants.log` |

应用行为使用同一冻结源码实际编译的 `../target04/debug/deps/editor_app-bf8b72a9198c679b.exe`（SHA-256 `A2F8040A6D9D8D5A4D935F0B2E488D464F8101D442BF25F94673EAAB536CCCF4`），直接逐过滤词执行 `--nocapture --test-threads=1`；明确标注的真实包测试另加 `--ignored`。没有因纯文档提交重复链接应用。门禁构建后的受控 stdio 夹具当前 SHA-256 为 `0E9192D1750B351D96FA8DA68F1D84CF7A6E4FB2B48EA78D40E677D57A67D05C`；前文 `1E29…` 是切片阶段的实际输入。完整计数和命令分别保存在 `joint04-final-gate-audit.json` 与 `joint04-final-small-app-results.json`。

## 物理反馈后的 Problems 键盘修复

首次联合 Host 的真实桌面中，Ctrl+Shift+M 已显示两来源诊断，但初始选中 Config 后 Down 不切换到 Spell。[物理阶段记录](04-native-desktop.md)由根代理独立保存。没有用既有 Escape 通过推断行导航有效：在正式两个检查器的公开 Package/Manager 场景补完整回归，快捷键打开后不点击任何子控件，依次检查 Down/Up/Home/End 的真实修复按钮、完整 KeyDown/KeyUp Enter 的原生定位，并继续原来的单文档与跨文件预览、取消、源 revision/关闭重开拒绝和分组 Undo。

精确命令为 `cargo test -p editor-app --bin editor-app native_community_language_diagnostics_fix_preview_source_revision_and_undo -- --ignored --nocapture --test-threads=1`。真实 Red 为 0/1/0，行为 68.10 秒，断言初始 Dialog 焦点下 Down 后仍没有另一个来源的修复按钮；日志 `joint04-problems-keyboard-red.log`。核对锁定的 `gpui-base 0.7.0` 公开 Dialog 行为后确认初始焦点在列表监听器的祖先上。修复仅为现有原生行建立焦点句柄，把初始焦点送入选中行，方向键在同一投影中移动焦点；Base 行仍拥有 Enter 点击，Dialog 仍拥有 Escape，修复按钮的 Enter 不被拦截。句柄没有保存诊断载荷或另建模型。

第一次 Green 编译仅因 ClickEvent 参数类型推断失败退出 101（6.48 秒），没有行为结果，不计 Red。明确公开参数类型后，同一完整测试实际 1/0/0，行为 68.43 秒、编译及执行 153.93 秒；日志 `joint04-problems-keyboard-green-final.log`。受影响的空列表命名控件/Escape 焦点测试另实际 1/0/0，行为 0.16 秒（`joint04-problems-keyboard-empty-green.log`）。新测试 EXE SHA-256 为 `9C0C819F27D68D0658AFDE295206F3EC5C67F805F3B01BBB1961124CA424AC3E`；上述 SDK 与六个 ZIP 不需因这项 UI 修复重建。修复后的普通候选、宿主 hash、三门禁、独立增量审查和物理按键复验仍需准确补记。

## 剩余验收

`cc17293` 的三项联合门禁、两个独立审查轴和最终六包的四项 runtime 连接回归已通过。Problems 增量修复的门禁、双轴审查和新 Host 物理按键复验仍待完成；其余最终 GPUI 连接回归继续执行。Ctrl+. 在旧 Host 的明确英文编辑输入模式下仍没有弹窗，右键动作有效，尚不能认定原因为 IME；由根代理在新窗口先复验，并核对完整按键路由及弹窗生命周期。C01–C06 不提前标记完整交付。每次仅因源码/输入变化或未决失败补充受影响测试；不重复未修改的 03 全磁盘负面矩阵。源前提修改仍检查单文件和跨文件修复连接、预览后改源与关闭重开、取消和会话 Undo，以及相关 03 回归。

最终交付由根代理完成整合、推送与 issue 状态读回。本记录不会把正在运行、ignored 未执行或编译检查误写为功能通过。
