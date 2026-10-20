# Vue/TypeScript 剩余场景实施

日期：2026-10-19。状态：实施中，尚未将剩余场景标记为完成。

用户通过 `execute-plan` 要求执行剩余所有工单。沿用[已批准规格](../specs/vue-multi-backend.md)、[六场景拆分](../tickets/vue-typescript/replan-proposal.md)及 Package → Manager/SDK → 受管服务、原生 DocumentSession/编辑事务测试接缝。#121 已完成；本轮执行 #122–#126，旧 #108–#118 与父 #107 不改动、不关闭，版本发布不在范围。

## 工作树与执行路线

- 起点：Editor `fc9dc531ef991315d73e21c8a092f12a0f1ff78b`，TypeScript `873e8c3d3ebe3a86a8bb1e028a3c4bcd1c557b94`，Vue `f8e5880bf89161f16723ae7be1c76458d50a676c`。
- 实施工作树：`C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor`，分支 `codex/vue-typescript-remaining`。已用 `cargo metadata --no-deps --format-version 1` 核对 workspace_root 与专属 target。
- 主工作区已有 `docs/窗口布局拖放规则.excalidraw` 改动保留；旧 `vue-typescript` 脏工作树只读，按行为选择迁入候选，不恢复旧 S1 缺陷或其他任务 UI 改动。
- 复用规格、工单，不重新拆分或询问已批准决策。共享宿主接缝集中，逐单 `implement`，独立部分由子代理核查；每单完成实现、门禁和双轴审查后提交、推送、核对关闭，再继续。
- GitHub 已读回 #122–#126 均 open；#122–#125 仅阻塞于已完成 #121，#126 阻塞于 #122–#125。普通提交、推送与关闭沿用既有授权。

## S2 / #122

先迁入真实 TypeScript 包经原生重命名、预览、取消、分组 Undo 和补全 resolve 的回归，在当前基线观察失败。旧候选和日志只保留历史身份；最终结果以本轮实际命令、源码和包为准。

- `cargo test -p editor-app real_typescript_rename_opens_unknown_targets_previews_cancels_and_group_undo --no-run -j 1` 编译通过（2m00s）。
- 直接运行所生成的 app 测试程序，显式 `--ignored --test-threads=1`，使用 S1 的真实 TypeScript 0.1.10 ZIP：**RED**，62.76s，未达到只读跨文件目标的完整拒绝行为。日志 `target/s2-typescript-red.log`；源码随后迁入完整编辑计划，尚待 GREEN。
- 迁入 `Plan.conditions` / `workspace.files 1.4`、跨文件提案、统一预览、原子文本提交和选择时补全 resolve；保留 S1 来源和生命周期保护。新增真实三版本 Vue prop 重命名、三项目共享脏 TS 与通知丢失后的水位恢复用例。此段为实现状态，不是验收结论。

### 定向执行与断点

当前已冻结的观测程序为 `target/s2-pre-context/deps/editor_app.exe`（SHA-256 `F8705D3F9B654C8A01446B03CCA098C8427FA4EEED7A4E040401AF99283D133B`），配对 `examples/lsp_fixture.exe` 为 `7B1664C343ED58C8ADF2C7921B46BE2F8BC245CB53DD98A484656422D78C0909`。它覆盖 S2 事务候选与通知恢复，尚不包含后续上下文注册接线；不能用它验证新接口。

| 用例 / 命令入口 | 实际结果 |
| --- | --- |
| `mixed_vue_projects_share_one_unsaved_typescript_dependency`，真实 TS/Vue 0.1.10、production Worker | 1/1，129.22s；三版本读取同一个脏 TS 并产生准确源码诊断，主 TS 提供者及所有磁盘原文保持；`target/s2-mixed-shared-first.log` |
| `notification_overflow_resynchronizes_unchanged_live_sources_before_diagnostics` | RED 0.23s → GREEN 0.16s；丢失最后推送后，即使字节未变也发送完整新版本，不保存源文件；`target/s2-overflow-{red,green}.log` |
| `runtime_log_tests::notifications --ignored --test-threads=1` | 4/4，1.50s；溢出后的日志、ready、请求和阻塞预算回归保留；`target/s2-notification-regressions.log` |
| `paged_related_graph_synchronizes_every_large_native_source_without_truncation --ignored` | 1/1，6.52s；260 份各大于 64 KiB 的源完整同步并有界分页；`target/s2-large-graph.log`。它验证标准 LSP 主路径，不扩大独立控制 RPC/编辑计划预算 |
| `editor::language_edits::completion::tests` | 4/4，0.00s；首 token 自动导入、冲突/非法尾部/代理对、命令和 snippet 整组拒绝；`target/s2-completion-boundaries.log`。不是原生来源失效联验 |
| `language::navigation::editing::tests` 普通测试 | 3 通过、1 ignored；`target/s2-editing-unit.log`。真实包非法尾部测试首次在夹具缺 `language.editing` 声明处拒绝，已修夹具，待重编重跑，不记作语义通过 |
| 真实 TS 完整事务第二次 / 带定向观测 | FAIL，54.62s / 56.25s；预览、只读拒绝、辅助冲突和跨文件应用已触达，分组 Undo 失败。观测证明 `capability_revision` 实际增加 2，而收据假定增加 1，错误留在旧 revision；按钮与 owner 均有效。日志 `target/s2-typescript-{green,undo-observation}.log`，正在修复，不把文件名 green 当结果 |

### 共享文件展示上下文

现有最近配置发现不足以表达共享文件的来源继承、歧义选择和本机偏好。补齐通用受管上下文引用、交互准备和原生选择刷新；领域候选与 TS 引擎策略仍在 TypeScript 插件。沿用公开 Package/SDK 测试接缝，不增加宿主 Vue/TS 专属分支。

首条注册回归已通过公开宿主 SDK 9187 构建独立 `context-selection-fixture-0.1.0.zip`，运行 `cargo test -p plugin-runtime --test context_selection -j 1 -- --ignored --nocapture --test-threads=1`，**RED** 25.23s：首个调用返回 `UnsupportedOperation: Unknown operation: language_contexts`。日志 `target/s2-context-registry-red.log`。注册接线的 GREEN 及完整选择/撤销仍待验证，不能用这一条测试宣称共享上下文交互完成。

### 后续实际结果

- 注册第一步经能力分发补齐后第三轮 **GREEN**：1/1、0 ignored、24.73s，`target/s2-context-registry-green-third.log`。中间缺少枚举处理造成编译失败；随后方法白名单遗漏仍返回 Unknown operation，均保留失败记录。完整交互准备、选择及撤销仍在实施。
- 文本提交收据改为校验原生 DocumentSession 因果 revision 及同一 Rope 后像，再读取实际公开 revision，避免假定通知次数。app no-run **PASS** 3m51s；冻结 `target/s2-receipt-context/deps/editor_app.exe` SHA-256 `25CE843FCFD0BFEEA278325AB9F176B6089DACCB18076CA6C167DB0F89006F5D`，fixture SHA 与上段相同。
- 此冻结程序运行真实 TS 0.1.10 完整重命名 **GREEN**：1/1、71.52s；预览、取消、辅助冲突、跨文件应用、分组 Undo 及实际选择补全 resolve 均经过原生文档。`target/s2-typescript-receipt-green.log`。
- 真实 stdio `packaged_rename_rejects_illegal_tail_without_exposing_a_valid_prefix` 修正夹具权限后 **GREEN**：1/1、1.07s；`target/s2-full-edit-decoder-fixed.log`。
- 同程序三版本 Vue prop 重命名 **FAIL**：153.30s。2.6 / 2.7 的应用与 Undo 已触达并通过，Vue 3 在完整提交后等待分组 Undo 超时；`target/s2-vue-prop-rename-three-versions.log`。整条用例不记为通过，正在区分已消费分析条件与持续 owner 授权。

## S3 工具层复验

使用 Node 24.19.0、Vue `f8e5880` 的现有源码及密封模块，TypeScript `873e8c3` 引擎启动器；未修改生产包。

| 命令 | 实际结果 |
| --- | --- |
| `node --test --test-concurrency=1 tests/*.test.cjs` | 84/84，0 跳过，88.024s；Vue `target/remaining-s3-source-tests.log` |
| 同命令，设置 `NANOBUG_TEST_EXPORTS=1`、`NANOBUG_TS_BOOTSTRAP=<TypeScript>/native/engine-bootstrap.cjs` | 84/84，0 跳过，83.784s；Vue `target/remaining-s3-sealed-tests.log` |

这两组是同一批测试的两种加载模式，不是 168 个不同场景；它们不替代 S3 的宿主实际包、原生高亮/格式化或 S6 的安全事务验收。

### Vue 0.1.11 当前工具证据

外部脚本视图限定到当前 SFC 静态 `src` 归属，支持对象及有界 `const` 导出（含没有 Vue import 的 JS/TS），项目间派生版本分离。真实双配置项目共享 JS、普通模块负例及双向关联变更均有回归。Vue 2.6 的显式 setup 拒绝与 scriptless 隐式块不误报分别保留。

首次完整新源码测试 98/102：四个 Vue 2.6 样式用例暴露内部零长度 setup 块误报 90001；`target/remaining-s3-source-011.log`。修复后样式/语法 18/18（20.23s），随后重建两个模块闭包：

| 加载模式 | 实际结果 |
| --- | --- |
| source，全 `tests/*.test.cjs` | 102/102、0 skipped，114.898s；Vue `target/remaining-s3-source-011-final.log` |
| sealed，同一套用例及 TS-owned loader | 102/102、0 skipped，105.307s；Vue `target/remaining-s3-sealed-011-final.log` |

模块 SHA-256：legacy `f021e8dd903fc37d956451c6faf7392847eba45a57a49529436b0ea44bc10c1e`，modern `a468cf03e3da4b77f326884661e847d63a0ff16f59491dffa0e7342ad738644b`；完整文件闭包及许可随 `module-exports.json` 生成。当前仅工具证据；真实 `.11` WASM/宿主和 S6 事务组合未通过前不标为正式支持。

## 待执行

S3 的历史写法与 SFC 能力、S4 未知生态贡献、S5 故障恢复，以及 S6 最终跨场景联验均未在本轮完成。各项验收要求按[场景工单](../tickets/vue-typescript/README.md)保留，不将 ignored、未运行或失败记作通过。

## 上下文、格式化和恢复的后续断点

以下均为本轮实际执行结果；候选测试程序和包不作为最终交付的替代品。

| 场景 | 实际结果与边界 |
| --- | --- |
| TypeScript 0.1.11 Node 全套，source / 解包后的 native archive | 分别 44/44、0 skipped，51.11s / 49.69s；source 为实际工具返回，未另存日志；sealed 日志 `TypeScript/target/s2-context-typescript-node-sealed.log`。归档内 16 个 native 文件与源码逐 SHA 一致；服务 ZIP SHA-256 `1d89ed0c86f7e9b98b0bca55441e1b80db5c0b8b8029200da46a1620796770ad`。最终源码套须另存日志，后续恢复修复若改变源码须重新生成和验证 |
| 原生选区格式化入口 | 首次 RED 6.39s：尚无 `FormatSelection`。接入标准 `rangeFormatting` 后，准确 Unicode/CRLF 选区及 single Undo、缺少 range 能力时不回退 whole document 两项通过。第三项 IME 用例在“响应尚未返回”的测试前置失败，未测到 IME；日志 `target/s3-native-range-green.log` 实际为 2 PASS / 1 FAIL，不能按文件名记为全部通过，改用真实 Windows 异步驱动复验 |
| 待确认提案与已提交恢复的分析条件 | `target/s2-consumed-analysis-guard-final-red.log`，0.38s，实际跨文档提交后因分析条件失效而错误撤销 Undo。正在将一次性准备条件与持续 owner/caller 授权分离；尚未记 GREEN |
| 真实连接替换后的旧 capture | 实际 RED 1.16s → GREEN 1.16s；连接 incarnation 退出后，即便新连接重复 wire/graph generation，旧 capture 仍失效。`target/s5-connection-capture-{red,green}.log` |
| 未知生态插件 Snapshot 生命周期 | 历史 0.1.0 包 RED 23.92s：未返回所声明状态。独立 0.1.1 包 GREEN 47.78s，checkpoint → disable → demand/enable → schema 1 空状态 → shutdown_checked，工作区磁盘不变。`target/s4-ecosystem-lifecycle-{red,green}.log`；新 SDK 重建及最终角色组合仍待验证 |
| 真实 Windows Vue 单项目后端退出与恢复 | 修正初始 string 方法诊断应为 TS2551 的用例预期后，实际恢复 RED 115.76s。退出前 Vue 2.6 / 3 的具体类型语义通过；只停止 A 后 B 保持可用，但 A 普通有限恢复返回 completionInfo “No content available”。清理完成；`target/s5-native-vue-recovery-initial-corrected.log`。正在补通用 delegate prepare 的重新授权与适配恢复，不把进程重启视为语言能力恢复 |
| 独立 Vue 当前公开 SDK 构建 | 正常宿主候选编译通过 2m30s，SHA-256 `0F0DA399D56FC9378739D327BD1E009F0A638986DFB4FF129E988AC4BA3DBF80`。公开 `--plugin-cargo Cargo.toml check --target wasm32-wasip2 --release -j 1` 实际 RED：SDK `5e66d563cce2d46a41a0ce91307a23006392439870d3486ea2ffa3bfe076b77f` 缺 `src/language_context/registry.rs`，E0583；Vue `target/remaining-011-public-sdk-check.log`。宿主导出表和资源回归已补，待重建宿主后通过公开入口验证；不手工修缓存 |

注册上下文接口为通用 `language.lsp 1.6`；引用绑定 owner、配置和来源代次，不授予路径访问权限。共享文件选择策略、候选项目及本机偏好由 TypeScript 插件提供。精确来源的刷新仅退休其绑定，待实际新包的交互测试证明；原生格式化、事务及恢复仍沿用公开协议和 DocumentSession。

### 跨日继续执行的实际结果

本轮工作继续至 2026-10-20（Asia/Shanghai）。[覆盖映射](vue-typescript-remaining-coverage-2026-10-19.md)单列 35 项 AC 与 V01–V14；新测试的存在不计作行为通过。尚未关闭任何剩余工单。

- 提案阶段回归 **GREEN**：`target/s2-consumed-analysis-guard-green.log`，1/1、5.63s。真实 global/source/connection 准备条件在待确认时分别撤销两目标；实际文本效果发生后全部过期仍可 group Undo；无文本效果不消费条件，owner 禁用仍撤销授权。旧连接 capture 与 fix origin 同步失效 **GREEN** 1.09s，`target/s5-connection-origin-phase-green.log`。
- 三 Vue 版本 prop 重命名/预览/取消/group Undo **GREEN**：`target/s2-vue-prop-rename-phase-green.log`，1/1、114.80s；TS/Vue 均为 `.10`，宿主冻结程序 `target/s2-phase-native-green/deps/editor_app.exe` SHA-256 `CC127F2FC039357D7E5C0693C62BAAB1489635E04FAD61D559F6F47EB73D8240`。它证明 phase 修复，不替代最终 `.11` 联验。
- 新真实 Windows range IME parent/child **GREEN**：`target/s3-native-inflight-range-first.log`，parent 7.96s / child 1.54s。实际 wire request 尚未回复时在 1.603ms 内预编辑/取消，原生 revision 0→2、最终字节还原；迟到结果拒绝，新请求同 Arc / initialize 1 / range 2、whole 0、single Undo、磁盘不变及 checked HWND 退出均通过。这里测试原生 IME bridge，不是 OS 候选窗口。冻结程序 `target/s5-range-locale-red/deps/editor_app.exe` SHA-256 `D4F43596BB3A59EFD92C50BFAE7CCD2268303CE7AA2C804313057FEB6EFFD273`。
- 生产 Package grammar/缺失 grammar/撤销预检 **GREEN** 2/2、3.07s；同包原生主题绘制、焦点、键盘、IME bridge **GREEN** 1/1、48.17s。`target/s3-native-grammar-preflight-010.log`、`target/s3-native-grammar-theme-preflight-010.log`，TS/Vue `.10`。最终 `.11` 需与本轮 grammar 资源比较并重放对应入口。
- 真实 Vue 故障恢复 **GREEN**：`target/s5-native-vue-alias-recovery-first.log`，parent 113.45s / child 113.27s，实际 `.10/.10`。Vue 2.6/2.7/3 初始具体类型及源码诊断通过；仅 A 退出后 B/C 原 Arc 和旧 capture 保持健康；正常 delegate prepare 在同一有界恢复预算下恢复 A 类型语义，旧 A 结果失效，未保存新类型仍被观察；撤信任、进程归零、actor ACK、HWND 销毁和磁盘不变通过。宿主冻结 SHA-256 `EA9F7A2FBEE5C39BC9A46D4983B25149A2F92F7385646BD10B4FE3AF2ED743C8`。更新候选拒绝另待运行，不从此结论外推。
- 状态本地化实际 **RED 0.11s → GREEN 0.28s**：英文 UI 原显示中文恢复状态，改用现有中英文资源及有剩余毫秒的 backoff 文本。真实启动失败、重连、预算耗尽同时检查 owner/项目、1/3→3/3 与单调失败代次；`target/s5-native-recovery-locale-red.log`、`target/s5-recovery-locale-green.log`。
- 新 SDK 修正导出遗漏后，Vue `.11` 公开 `--plugin-cargo check` **PASS** 4.31s，`--plugin-package` **PASS** 15.47s。SDK `20d009f643dbc88ac70667c8ed5c9fb709ca9a4b5bb378396c5e49901a0958e3` 的候选 ZIP SHA-256 `4a8ff5ec67cace5689fbaf3d10dddc787c8fb58dc2cc37ad4141d3c96dfa26d5`，4,120,834 B、39 entries，WASM `85767047f4d86690c93813b3124ce9913a07ae664375184f6f651dbe1711aaf6`。输入及逐文件收据在 Vue `target/remaining-011-{package-metadata,package-receipt}.json`；不能把后续新 SDK 重打的归档当成此 SHA。
- 同 SDK 的 TS `.11` guest check **PASS** 32.79s；guest build **PASS** 41.61s 后实际 Package admission **RED**：`Unsupported delegated service permission`。通用方法校验遗漏 canonical interactive prepare 的私有 storage 声明；精确 canonical profile 正例及改变参数/增加 editor.write/普通 delegated storage 负例完成 **RED→GREEN 1/1**。`target/s2-context-interactive-profile-{red,green}.log`。源码与 SDK 格式已统一冻结，新 SDK 输入 digest `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`，仍待普通 CLI/两包及消费者构建闭合。
- TS 最终 source Node 套实际落盘 **44/44、0 skipped、51.079s**，`TypeScript/target/s2-context-typescript-node-source-final.log`；与 49.689s 的 sealed 套对应同一 `1d89…70ad` native 归档，后续 Rust 偏好键稳定化不改变该 native 闭包。
- S4 四角色初次实际宿主预检 **FAIL** 113.97s，`target/s4-four-roles-first-native.log`；未到角色语义。测试将临时目录作为授权 workspace，却把依赖 junction 指向外部 Vue fixture，WASM 元数据读取正确拒绝 `Path escapes capability root`。已将临时项目放入授权 Vue fixture workspace 的 `target` 下，保留严格链接边界；待重编重跑。待确认映射提案跨实际替换/卸载的原生撤销测试亦已注册，未记通过。

### SDK 4af 与后续候选的实际断点

- 正常 CLI **PASS** 1m36s，冻结 `target/s6-sdk-4af-host/editor-app.exe` SHA-256 `D587836C7A97BE4B22EEC9FD95BA3778226DA8CA877B906D8426D5F819264412`；公开导出并核对 54 项 SDK 输入，digest 为 `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`。这闭合前述导出及 canonical prepare 修复，不从 guest 编译成功外推交互成功。
- Vue `.11` 经此 CLI 的公开 check **PASS** 4.30s、package **PASS** 34.04s。新 ZIP `8e665285ac0d2701ba513d2d11ddd63f61771a27e21843e6869f000a7ee16252`，4,120,843 B、39 entries；`vue.wasm` 为 `5bcef489647b9611575722bda6569f29e7bdb26e0240b549323b443b653b4dea`。逐文件收据为 Vue `target/remaining-011-sdk4af-package-receipt.json`，旧 SDK 20d 候选及收据另存保留。
- TS `.11` 同 SDK 公开包准入通过，ZIP `d1c582b64262cd56bd4c09c22868f8d88f697b83791d9006027b24ce6185b427`，22 entries。但是实际共享项目交互 **FAIL** 203.26s：`project.v1.` 加 64 位摘要共 75 字节，超过公开 `PreferenceKey.name` 的 64 字节限制；`target/s2-shared-context-first-011.log`。此包仅保留失败证据，不能交付为已验收。
- 偏好键加真实公开 `.validate()` 回归后 **RED 0.93s → GREEN 0.76s**：版本域放进 SHA-256 输入，输出保留完整 64 位小写 hex，不截断、不放宽宿主契约。TS 候选提升 `.12` 并经同一公开 SDK package **PASS** 13.82s，ZIP `f869529d1906d0a06c8a6c90225dfa09377092c1ae6654f20273a5ebb4a1c3a5`。native 服务字节没变，仍真实标注 `.11` / `1d89…70ad`。实际 `.12` 共享联验 **FAIL** 63.45s：取消、双项目选择、精确 A 撤销而 B 保持、非空偏好及 API 不兼容包拒绝均已触达；最后 F12 来源继承后的 hover 断言失败，`target/s2-shared-context-second-012.log`，仍待定位并完整重跑。
- app 当前 no-run **PASS** 2m59s，冻结 `target/s5-updates-unified/deps/editor_app.exe` SHA-256 `C535EA71DE04E1CFE9B9162172585800C7899A1606C3011B1E4CBE421EF9EEE7`。此程序的未知语言补全实际 **GREEN**：`target/s2-unknown-completion-resolve-first.log`，parent 19.11s / child 5.51s。普通键入和选中后才 resolve；主编辑与自动导入一起应用并可一次撤销；源 ABA、关联脏源变化、合法前缀加非法尾部及 owner 禁用均整组拒绝。同一私有 Windows harness 重跑选区 IME ABA **GREEN**：parent 7.52s / child 1.57s，`target/s3-native-inflight-range-reused-harness.log`。两项均检查磁盘不变和 actor/HWND 退出。
- S3 三版本 × 四样式 whole/range 用例所有语义断言已触达，但整体 **FAIL** 160.76s，`target/s3-style-range-matrix-first.log`：真实 actor 的异步唤醒与 GPUI TestScheduler 不兼容，在 `end_test` 拒绝跨线程活动。保留失败，改用有界实际 Windows 驱动；不放宽调度器，不把触达断言记为用例通过。
- 12 个生态场景都已通过公开 CLI/SDK 4af 的独立打包与 Package 准入；逐包 SHA、版本与路径为 `target/s4-ecosystem-sdk4af/packages.json`。四角色组合及缺 style 替换包共享同一插件身份、分别 `.1.1` / `.1.2`；循环、配置冲突和非法映射是独立可安装包。该步骤不等于实际宿主组合验收通过。
- 新增真实 TypeScript 项目引用 / paths 别名 / 工作区内 junction 用例，source 与解包后服务各 **PASS** 1/1、约 1s，原始脏源码 URI、具体类型、诊断范围、完整关联图与磁盘不变均通过；`TypeScript/target/s2-project-references-{source,sealed}-first.log`。包含此用例的全套 source **45/45、0 skipped、52.636s**，sealed **45/45、0 skipped、51.267s**，日志 `target/s2-project-context-node-{source,sealed}-012-final.log`（sealed 在 target 目录）。首次遗漏 Vue 环境变量的运行是 30 PASS / 15 skipped，单独留作环境失败记录，不计入完整通过。
- `.11/.11` 真实更新与恢复联验 **FAIL** 161.05s，`target/s5-native-vue-candidates-and-recovery-011-first.log`：三版本初始类型语义及 malformed 准备拒绝、旧实例保留已触达；第二个完整 WASM 候选尚未在测试的 30s 观察期限内完成编译准备，没有取得最终安装回执。只修正该观察期限以符合冷 component 准备预算，并补阶段及兼容失败诊断；生产尚未据此修改，也不把这次超时误报为契约回滚缺陷。

### 真实 Windows 格式化、进度与回滚的后续结果

格式化四项均改用普通 libtest parent → 独立 WindowsPlatform child，保留原阶段观察预算与全部断言，不关闭 TestScheduler 检查。输入固定为 TS `.12` `f869…c3a5`、Vue `.11` `8e665…6252`；宿主冻结 `target/s6-native-format-context-locale-red/deps/editor_app.exe` SHA-256 `8A5BF17237C3716FFF31488F8AB106A5A9DC0667B6B4A0FA8C93186A812FB6E3`。每项都经过 checked actor shutdown、进程归零、磁盘原文检查及所属 HWND 退出。

| 实际入口 | 结果（parent / child） | 日志和范围 |
| --- | --- | --- |
| `installed_vue_whole_and_selected_style_formatting_preserve_source_and_single_undo` | **PASS** 101.79s / 101.63s | `target/s3-native-formatting-matrix-first.log`；Vue 2.6/2.7/3 × CSS/SCSS/Sass/Less 共 12 行，whole/range、单次 Undo、Unicode/CRLF、标签/属性/外部 src 保留 |
| `installed_vue_rejects_empty_cross_block_attribute_and_external_src_format_ranges` | **PASS** 93.09s / 92.95s | `target/s3-native-formatting-boundaries-first.log`；空/跨块/属性/src 选区明确拒绝且不退回全文；合法外部 script/style 的 whole 格式化保留属性和外部磁盘 |
| `installing_vue_preserves_the_existing_typescript_formatter_and_primary_analysis` | **PASS** 70.28s / 70.13s | `target/s3-native-formatting-provider-first.log`；安装前实际 TS 分析与显式 formatter 选择，安装后 primary 原 Arc、grammar/formatter 选择及真实格式化/Undo 保持 |
| `installed_vue_format_rounds_cannot_overwrite_same_turn_native_input_or_ime_aba` | **PASS** 93.14s / 92.99s | `target/s3-native-formatting-aba-first.log`；range/whole × native/IME 四种 same-turn ABA，原生 revision 增加而身份不变、迟到结果拒绝、fresh 格式化及 Undo。不是 OS 候选窗或 wire 已 pending 的额外断言 |

- TS `.12` 共享用例第三轮真实 **FAIL** 93.87s，`target/s2-shared-context-third-012-observation.log`。普通 F12 已到正确源码，但完整等待后仍是旧 right 分析 Arc；实际 source left 与其注册引用正确，排除单纯观察过早。`main.rs::request_definition` 原先直接打开目标、遗漏已有来源继承路径；现统一 F12/show-document，在原 request/path/revision 与源服务活性检查后，对 canonical 文件借用一次来源提示。待新冻结程序完整重跑，不以接线源码存在计通过。
- 通用回滚实际 **RED** 162.62s，`target/s5-hot-update-actual-red.log`：候选失败后旧 pending/lease 已退休，但恢复实例在后续角色 demand 中被当作冷记录，语言计划丢失。`activation` 现在仅收养已实际 active owner 的 `materialized` 状态，并增加角色准备前后 owner 恒等断言；新 runtime 八目标 no-run **PASS** 39.82s，尚待该实际回归 GREEN。
- 当前源码的轻量依赖/准入测试 **14 PASS / 13 ignored**：managed 8/8（2.82s，另 1 ignored）、optional 3/3（0.13s）、native-modules 普通 2/2、plugin-services 普通 1/1。`target/s5-runtime-final/inputs.json` 锁定八份程序，日志 `target/s5-{managed,optional,native-modules-ordinary,plugin-services-ordinary}-ordinary-final.log`；尚未运行的 WASM 回归不计通过。当前 SDK 的 delegate/双消费者/service-set fixture 已分别经公开 CLI 构建及单独 metadata 核对，交接入口 `target/s5-runtime-handoff-2026-10-20.md`。
- 安装进度实际队列发布 **RED**：英文显示 `准备安装…`，`target/s5-queued-install-locale-red.log`。测试在持 mutex 时断言还触发了 drop poison 的二次 panic，已改为先 clone 发布值再断言。修复把本地和市场依赖阶段统一到现有 i18n 资源，终态失败/成功和操作标签也使用同一语言；新冻结宿主 `target/s6-f12-locale-green/deps/editor_app.exe` SHA-256 `FE5919D3A1449BDB9C5C0526FB6A1F2BB9DF25DD70D7270DAEA7AE04A075242F`，no-run **PASS** 2m36s。对应回归 **GREEN** 1/1、0.17s，`target/s5-queued-install-locale-green.log`：实际 queued 发布、十个阶段 renderer 的文案/取消边界，以及实际后台检查失败发布同时验证 EN/ZH；不把 renderer 检查冒称实际下载操作。
- S4 第二轮实际 **FAIL** 119.04s，`target/s4-four-roles-sdk4af-second-native.log`：生态 guest 读取自有声明资产，但仅声明权限、未协商 `package.assets`，正常边界拒绝。只修生成清单与 fixture 版本为 `.1.2` / 同身份缺 style `.1.3`；12 个独立新包再经 SDK 4af 公开 Package 准入通过，逐包记录 `target/s4-ecosystem-assets-sdk4af/packages.json`。原 `.1.1/.1.2` 包完整保留，不放宽宿主能力或方法权限，实际四角色语义仍待复跑。
- 全 workspace 首次 `cargo fmt --check` 发现本轮 19 个文件需要格式统一；只将 rustfmt stdout 形成的最小补丁经 `apply_patch` 应用，并核对全部位于本任务文件清单。重跑 **PASS**，`target/s6-workspace-fmt-green.log`。没有公共 SDK 源输入改变，SDK 仍为 4af。独立 Standards / Spec 审查已开始，固定基线及原始候选 SHA 清单在 `target/s6-review-inputs-2026-10-20.json`（SHA-256 `c0551a646f33a96afe007fe50f32a1d1dcac7b4ff61020f953739de3db76a68f`）；清单记录格式化前原字节，审查按格式化后行号定位，尚无最终审查结论。

### F12 通过、真实更新失败和内层编译器复审

- 共享 TS 文件完整真实 Windows 联验 **GREEN** 70.95s，`target/s2-shared-context-f12-green-012.log`；宿主 FE5919、TS `.12`。取消、双项目选择、仅 A 绑定撤销而 B 保持、非空私有偏好持久化、API 不兼容候选拒绝、关闭重开及实际 F12 来源继承全部通过。F12 后旧 right Arc 确实退休，新分析选择 canonical left 项目且 hover 对应；下次直接重开恢复显式 right 偏好。此 API 不兼容负例不是 TS/Vue 领域契约更新测试。
- Vue `.11` 有效 WASM 但要求 `typescript.adapter ^99` 的实际候选联验 **RED** 170.01s，`target/s5-native-vue-candidates-and-recovery-012-second.log`。准备失败候选保持旧实例，但此候选到 Migrating 后才报告不兼容，旧 Vue 语言绑定已经丢失。现把当前活动旧实例的必要服务依赖只读检查放到旧实例停止之前，仍在实际 cutover 时复核；未激活的冷记录不提前启动。后续反向 TS provider 领域 major 变更另补验证，不能由 API 版本负例外推。
- 通用更新/回滚完整回归 **GREEN** 1/1、251.34s，`target/s5-hot-update-preflight-and-rollback-green.log`。必要服务不兼容候选在旧 owner 退出前拒绝，原 Arc/计划/进程保留；实际 guest 激活失败恢复旧数据与 owner，后续角色 demand 不再二次重建回滚实例。固定 capability-example `.19` 包未重建。故障隔离显式 WASM 回归 **GREEN** 1/1、188.40s，`target/s5-fault-recovery-preflight-final.log`。
- 独立 Spec 复审发现只结束内层 TS/Vue compiler 时外层 stdio 留存，导致有限恢复无法启动；真实 Node **RED** 6.61s，`TypeScript/target/s5-inner-exit-red.log`。TS `.13` 仅在已准入引擎致命退出时退休所属外层服务，显式 close、取消、合法业务错误及候选准入失败仍保留现有服务。定向三项 **GREEN** 5.15s；当前完整 source **47/47、0 skipped、55.739s**，sealed **47/47、0 skipped、54.751s**，日志 `TypeScript/target/s5-typescript-013-{source,sealed}.log`。原生内层故障仍待跑，未由 Node 结果代替。
- TS `.13` 继续由公开 SDK 4af 构建：guest check **PASS** 9.99s、package **PASS** 15.22s，包 SHA-256 `66d60b57913a317920cf9f0b0d3492e4661dc776b8b6c9714cf3057a6d5766c7`（37,548,409 B、22 entries）；native archive `9bd5b8b90a616bb7267c0d79f786997cefa735f5bf606074e71293a8ffa8f34e`（37,141,699 B）。源码/stage/sealed 的 16 个 native 文件逐 SHA 相符，收据为 `TypeScript/target/s5-native-013-closure-proof.json` 和 `s5-context-package-013-proof.json`。
- S4 新 `.1.2/.1.3` 归档第三次预检 **FAIL** 0.24s，`target/s4-four-roles-assets-sdk4af-third-native.log`：旧冻结测试程序仍断言 `.1.1/.1.2`，尚未进入角色执行；源码 guard 已同步并重编，不算产品行为失败或通过。
- 强制 `cargo test --workspace --exclude editor-app -j1` 首轮 **FAIL**：完整编译后 `language_declarative` 2 PASS / 1 FAIL，测试期望 `language.lsp 1.5`，现行公开上下文能力为 `1.6`。已同步期望，日志 `target/s6-workspace-non-ui-first.log`；最终全套仍待重跑。不能把已完成编译记为全套测试通过。
- 增量 Standards 复审确认生产增量未见新规范缺陷，另报两项测试进程防护 P2：PowerShell helper 缺少自身超时回收，Node 故障注入未在 kill 前核对创建身份。正在修复；详见[双轴审查记录](vue-typescript-remaining-review-2026-10-20.md)。

### `.14` 真实加载修复与内层故障通过

- 两项 P2 helper 修复经独立 Standards 复核解决；测试进程有自身 10 秒期限、kill/wait 与有界输出，故障目标采用创建身份和持有句柄复核。TS `.13` 最终新 helper source **47/47、60.403s**、sealed **47/47、59.840s**，0 skipped；`target/s5-typescript-013-{source,sealed}-final-helpers.log`。类型及格式检查通过。没有把后续 `.14` 产物沿用成 `.13` SHA。
- 历史八行第二轮 **FAIL** 126.30s，`target/s3-historical-native-013-second.log`；Vue 2.6 class、2.7 class、2.6 JSX 三行完整通过，第四行 TSX 在 native source postimage 等待失败。相同三图真实 sealed Session 探针 **PASS** 1.57s、提案完整，不能替代宿主验收。保留前置/等值断言，补阶段源诊断和现有 220px receipt 区真实滚轮及 pointer-down 命中检查；未删除历史回执、不调用内部 Undo，待完整复跑。
- S4 四角色第四轮真实 **FAIL** 118.55s，`target/s4-four-roles-assets-sdk4af-fourth-native.log`：已进入实际 adapter admission，返回 `Adapter failed to load`。独立 probe 及真实公开 stdio **RED** 0.899s，确认 Node 24 对贡献的 verbatim Windows 路径解析失败，底层 `EISDIR lstat C:`；普通路径成功且 descriptor 仅约 4.8KiB，不是凭猜测归因命令长度。
- TS `.14` 只在 bootstrap 使用各自已 `verifyClosure` 的规范入口，不裁剪授权路径；新 stdio 用例含实际 >260 字符贡献入口。定向 **GREEN** 2/2、2.430s（后续完整套另有 2.371s 定向日志），source **48/48、69.088s**、sealed **48/48、63.826s**，0 skipped；`TypeScript/target/s4-typescript-014-{source,sealed}-final.log`。类型/格式/bootstrap 语法检查通过。独立 Spec 增量复核无新发现。
- `.14` 公开 SDK 4af Package **PASS** 23.842s，ZIP `92c29c83c8fa4e72fea83cfc11f3917773e43845c54a57d24e1f68d8dae5e9c9`，37,548,822 B、22 entries；native service `71c2794f0833bb548d365255878362b4637750bdac05e7ff7e8392df73884f83`，37,141,873 B。156 archive entries 与 stage、16 native source/stage/sealed 逐 SHA 一致；相较 `.13` 仅 bootstrap 修复与 serverInfo 版本改变。汇总 `TypeScript/target/s4-typescript-014-final-tools-proof.json` SHA-256 `d0a32dbaa0fcc092dfd538dd17a5d0acce62d0ecc9483b1240d29ee9396dc904`。
- S4 `.14` 第五轮 **FAIL** 242.78s，`target/s4-four-roles-native-014-fifth.log`：四角色真实类型/模板/项目/样式诊断及精确源码范围均已返回，但测试只接受 widened `number` 错误，实际 TS 保留更精确的字面量 `42`。只修测试接受这两种数值类型表示，仍要求唯一精确位置与两角色完整集合；未将本轮算成映射事务通过。修复已编入宿主冻结 `e18fe31f27f2f7f9e72f1bdd45afd345ca5eb6bf0b5d0b8315f3d776d63877fe`，no-run **PASS** 1m57s，待重放。
- 内层 adapter 真实 Windows **GREEN** 122.54s / child 122.38s，`target/s5-native-inner-adapter-014-second.log`；宿主 `0b8e4ac4499afa05e134eedb9e85b7bc97c71806a4f25fd9d8217148d26e8c39`，TS `.14`、Vue `.11`。A 首先单独建立具体语义，再加入 B/C；只终止 A 内层 adapter，外层自然 EOF，普通有限恢复、脏类型变化、旧 capture 撤销及 B/C 原 Arc 和 capture 保持均通过，最后撤信任/进程归零/actor ACK/HWND 退出/磁盘不变。此前 `.12/.13` 两次只在初始 UI 投影就绪前置超时（226.40s / 224.35s，checked cleanup OK），没注入故障或候选，不能充作对应 RED；改为实际 plan/server 就绪后，原语义和故障断言完整保留。
- 通用恢复当前完整实际 **GREEN**：hot update 1/1、251.34s；fault 1/1、188.40s；data migration 4/4、847.05s（内部 crash child 仅 filtered，不冒称单独通过，父中断恢复实际执行）；native modules 显式 3/3、402.97s；managed receipts 显式 1/1、231.52s。`target/s5-{hot-update-preflight-and-rollback-green,fault-recovery-preflight-final,data-migration-preflight-final,native-modules-sdk4af-preflight-final,managed-receipts-preflight-final}.log`。
- Service sets 首批 **2 PASS / 2 FAIL**、595.69s，两例 `Probe TimedOut` 保留；不扩大原 30 秒预算。同入口借用/选中单项重放 **GREEN** 144.28s，观测实际冷 member Call 为 28.139s、真实握手和 EOF，其他失败仍待重放。该 fixture 末尾是 unchecked shutdown，不将其当成完整 Snapshot/checked checkpoint 证据。Plugin services 首批 **2 PASS / 7 FAIL**、499.15s，七例均在默认空 workspace 的安装前置被正确拒绝；测试改用 temp/workspace 与 temp/plugins 兄弟目录、同作用域重开，不改产品信任规则，待新九例实际运行。

## 2026-10-20 01:56 后续验证

- 取消委托的公开 SDK 回归实际 **RED 66.17s → GREEN 68.78s**，普通准入另有 2/2 通过。独立 Spec 增量审查无可行动项。详见[取消后的独立委托](vue-typescript-deferred-control-cancel-2026-10-20.md)，不扩大为物理进程或 Snapshot 证明。
- Service sets 另一失败用例单独重放 **GREEN 221.64s**，`target/s5-service-sets-open-observation.log`，保留相同 30 秒预算，实际冷 member Call 28.170s。四个不同用例目前各有通过证据，但此前整组 2/4 失败不改写为一次整组通过。
- 当前 SDK 的上下文选择实际 **1/1 GREEN 92.15s**，`target/s5-context-selection-sdk4af-final.log`；生态生命周期实际 **1/1 GREEN 53.76s**，`target/s5-ecosystem-lifecycle-sdk4af-final.log`，后者覆盖公开 checkpoint、disable/enable 和 checked shutdown。Plugin services 修正工作区前置后，deferred 3/3、387.30s 与 lifetime 3/3、386.95s 已通过；routing 尚未完整结束，不提前计入整组通过。
- 历史八行第三次 `.14` 实际 **FAIL 90.22s**，`target/s3-historical-native-014-third.log`。首行实际重构后像通过，测试诊断在非绘制阶段读取 `content_mask` 导致 panic；已去掉该绘制专用读取，保留滚轮、真实点击命中、完整文本及原预算，待重放。
- `.14` 外层更新第二次实际 **FAIL 126.66s**，`target/s5-native-reverse-provider-014-second.log`。三后端先建立真实类型，两个 Vue 候选均拒绝，但测试读取的一次性状态已被 UI 消费，未取得归属失败。改为有 watermark 的每插件持久 host.operation 日志，并增加公开请求代次保持断言；反向 provider major 尚未触及。期间对短命测试窗进行 UIA 检查后，AccessKit 在 HWND 移除时发生上游回调 panic；保留实际失败，后续自动化运行不连接短命窗口，普通可见验收使用专属持久窗口。

## 2026-10-20 02:11 收敛记录

- Plugin services 新三组全部 **GREEN**，deferred 3/3、387.30s；lifetime 3/3、386.95s；routing 3/3、465.65s。共九个不同的显式 ignored 用例，filtered 用例未计入通过；三份 `target/s5-plugin-services-workspace-*.log` 保留，旧工作区前置失败不覆盖。
- 原生 grammar 资源闭合：直接读取两个历史 TS `.10` ZIP 与 `.14`、Vue `.10` 与 `.11`，TS 的 13 个 grammar/queries 条目、Vue 的 18 个条目，名称/长度/SHA-256 全部逐项一致。`target/s6-grammar-resource-closure-014-011.json` SHA-256 `b39eadb99bcd5328d417a83f53290595a8e9eeb2b9039ceb8fbc4cf1d89ba3d2`；该身份复用此前实际 native loader/highlight 结果，不扩大为新桌面输入验收。
- 宿主 `704679343f8e87a8f4d0b7e042b83d3b48c9d012aa5a3d9552d1dec6f0b83dbf` 下两个更新用例分别 **FAIL 225.15s / 224.93s**，仅在三个图冷启动准备前置超时，checked cleanup OK，未执行任何候选；不能称作 epoch 或反向更新 RED。准备改为先建立第一个冷源的真实语义，再打开另两图，保留全部生产请求期限；该策略原已用于通过的内层故障场景。
- 同冻结宿主与 TS `.14` 的 public consumer 三项为 **2 PASS / 1 FAIL、176.09s**，`target/s6-typescript-public-negative-and-native-014.log`。实际公共 resolve/describe/脏文本分析/release 及无 TS 提供者两项通过；坏显式 SDK 已取得预期 phase 0 和结构化错误，但旧测试强求一个已发布 plan，实际为零。修为允许先拒绝、或唯一已有 plan 必须保留 exact SDK，不允许替代 bundled engine；尚待三种坏 SDK 完整重放。
- 通用反向 provider 回归真正 **RED 58.81s**（compile 21.80s），`target/s5-reverse-provider-lib-red.log`。真实固定 Package/Instance/Broker 已将 mandatory consumer 解析到旧 1.x owner，公开 prepare/commit 却接受 2.x 候选，证实只查候选 requires 的缺口。该 lib 测试不启动 Node、不写 Vue fixture，也不代替后续实际 Vue/TS 组合验收；正在补通用预检。
- 独立真实 Windows 反向候选同样取得目标 **RED 147.52s**，`target/s5-native-provider-major-staged-red.log`，冻结宿主 `7f594f3f9bc4b998dfafd0f6363af6ae37e93273b4e10565664cdb0bf0796651`、TS `.14` / Vue `.11`。先用三个真实 Vue 上下文建立具体补全与源码诊断，再准备仅变更 offered contract major 的实际 TS 包；27.53s 后提交并 `installed:true`，三源从 Ready 转 Loading，失败为 `incompatible TypeScript provider candidate replaced the usable historical combination`。checked cleanup OK，无 UIA teardown 异常。本轮确实达到更新检查，区别于前两次 setup-only 超时。

## 2026-10-20 03:32 当前收敛结果

- WASM 与纯资源候选的反向依赖预检现已共用有效契约快照，并在 Actor 退休公开 epochs 前检查，提交时复核。Spec 增量发现的纯资源绕过取得 **RED 27.90s → GREEN 28.06s**；原边界 **GREEN 57.98s**，当前运行时 lib **104 PASS / 3 ignored、13.58s**。两独立审查轴均已复核该 P1 解决，见[审查记录](vue-typescript-remaining-review-2026-10-20.md)。真实三后端外层更新联合回归还在执行，不以 lib 代替。
- TS `.14` 的三个坏显式 SDK 场景实际 **GREEN 251.19s**，`target/s6-typescript-explicit-sdk-final-014.log`；拒绝发生在编译器启用之前，保持结构化错误、精确 SDK、不静默改用 bundled engine 和 checked cleanup。此前 2 PASS / 1 FAIL 的旧观察断言不覆盖。
- Vue 当前 `.11` 的真实包策略用例 **GREEN 188.12s**，`target/s6-vue-policy-current-final.log`；使用受控公开 TS provider / policy consumer 夹具，覆盖证据不足、冲突、历史未知、pnpm、显式非法版本及工具不兼容等准入政策，不把它称为实际 TS 编译器语义验收。
- 最终 `.occlude()` 修复后的宿主 `703c8ba44d5b6927ef8b5a748e734ffe8b8f99d5d7ced2a2f79bc5a516d4c04a`，配合 TS `.14` / Vue `.11`，实际历史八行 **GREEN 144.24s**、外部 `script src` 两组合 **GREEN 104.93s**、生态四角色跨文件查询/编辑/替换/卸载 **GREEN 152.95s**。均保留原生指针操作、会话完整原文/磁盘检查、checked shutdown 和进程归零；详情与旧失败日志见[原生验收记录](vue-typescript-s3-s4-native-acceptance-2026-10-20.md)。四冲突移除恢复第二轮仍 **FAIL 154.55s**，正在区分界面贡献同步边界与失败上下文缓存，不提前计完成。
- 当前生产源码强制门禁全部通过：`cargo fmt --check`，`cargo test --workspace --exclude editor-app -j1` **319 PASS / 0 FAIL / 327 ignored**，`cargo check --workspace -j1`。日志为 `target/s6-workspace-{fmt-post-occlusion-final,non-ui-final-third,check-final}.log`。前两轮过时测试文案期望的失败保留，修正只涉及测试的现行英文技术指导断言。
- 正常宿主构建 **PASS 1m54s**，`target/s6-final-native-host/editor-app.exe` SHA-256 `ff92755696cb68a3b00aac8111077b90908f0b50019beaeb6bdf56b8545c2776`。由它公开 `--plugin-cargo` 检查 TS/Vue 均 **PASS**，SDK 仍是 `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`；未因仅宿主实现变化重新标注另一 SDK。最终候选包、持久桌面交互、实际更新恢复与交付/工单关闭仍待完成。

### 03:36 前置失败与观察边界

真实外层更新恢复在 `703c8b...04a`、TS `.14` / Vue `.11` 下再次 **FAIL 226.46s / child 226.26s**，`target/s5-native-outer-final-014-011.log`。首次源由 Loading 转为 `TimedOut: Vue resolve context ... Vue dependency call cancelled`，仍未执行任何更新或故障注入，checked cleanup 完成。20 秒采样的 worker/native editor request 队列为空，只覆盖界面控制队列，不观察 Broker 的冷激活等待，因此不能据此断言请求丢失。该轮与测试程序编译并发；后续诊断须区分冷编译预算与真正未继续的请求，不扩大生产期限来掩盖失败。

生态卸载恢复观察器补用了正常 render 同步入口；新测试程序 `target/s6-contribution-recovery-adoption/deps/editor_app.exe` SHA-256 `064129a3f58120c13b543fdc6059a1affe95198103dd051155f558acbb24fa48`，no-run **PASS 2m33s**。原 Spec 与 Standards 代理分别只读复核源码 `085c3f1e708acaa93bfe43dad8d3ba7c14d86b623d1b7435a252cd929e6be508`，均无新发现；下一轮保留原 180 秒与严格失败检查，尚无终态，不记为通过。

### 03:40 冲突恢复的真实产品 RED

上述原生同步边界第三轮实际 **FAIL 162.39s**，`target/s4-conflicts-native-014-third.log`。exclusive 冲突被正确拒绝；卸载已发布 configuration `14 → 16`，但 Vue 的 Availability（manager 2 / generation 3 / configuration 3）未改变，显式正常同步后的 `ServiceLoadState` 仍为同一旧错误。已排除只少等一帧的解释。失败的准备没有成功服务 lease，原来的按失活 Arc 清理路径不能撤回 Manager `Err` 缓存及 App 的完成失败记录。

正在实施通用修复：在文档准备预留时保存它声明的 collection 输入快照，真实输入变化仅撤回已终止的失败结果；成功服务及 pending 请求仍沿既有生命周期。App 只因该完成错误的公开结果被撤回而重新 demand，不将准入失败变成逐帧重试。没有新增语言/插件 ID 分支或公开 SDK 能力。修复后的完整四冲突、健康组合及最终增量审查尚待，不提前标记通过。

### 04:28 发布证据修复与冷准备前置

失败输入捕获已覆盖 Vue → TS 的传递 collection 声明。独立审查随后分别发现准入拒绝误重试、已接受取消误重试两个 P2，实际取得 **RED 1.15s** 与 **RED 1.50s**，均保留日志。最终由原始文档 reservation 保存实际终态 Err 发布的历史 Arc，Worker 只转送 CompletedErr 对应的证据；取消、超时和准入拒绝不置位，缓存先撤回后送达仍可识别。最终三回归 **3 PASS / 0 FAIL / 0 ignored、1.66s**，双轴增量复核均无新增发现，详情见[审查记录](vue-typescript-remaining-review-2026-10-20.md)。

当前冻结测试程序 `target/s6-publication-provenance-green/deps/editor_app.exe` SHA-256 `918351a79c4cbf0055d283b2c68c717747df49925feeef6afb95d74bfcead5a1`，no-run **PASS 3m28s**。真实完整生态冲突在该程序及 TS `.14` / Vue `.11`、五项 SDK 4af 生态包下独占执行，实际 **FAIL 127.22s**：初始四角色准备 `TimedOut: The plugin did not become available in time`，尚未安装任何冲突候选。这是冷准备前置失败，不计失败缓存修复的目标 RED 或 GREEN，也不冒称 checked cleanup。源码、预算和严格断言未改；无匹配本轮程序及后端残留。

开始使用同一冻结程序运行带有安全编译耗时／原始请求阶段／epochs／plans 观测的真实 Windows 外层场景，日志 `target/s5-native-outer-provenance-solo.log`。此轮不并发 Cargo、工具后端或 GUI 检查，用于区分编译预算、依赖继续和上下文提前撤销；无结果前不宣称故障恢复通过。

### 04:32 冷准备诊断的对照前置

该轮实际 **FAIL parent 222.60s / child 222.46s**，日志 SHA-256 `61bd813a227dcc321d30ed9b3ce33e329875a29121396782795275cd092f99e7`，仍为首次 Vue 2.6 来源的依赖调用取消；未更新或故障注入，cleanup `Ok(())`。原生队列为空不代表 Broker 丢帧。只读进程采样限定冻结程序及其本轮后代：2.021s 内子进程消耗 28.234s CPU，尚无 Node 后代。原 epoch 只从无变为首次 Vue `1`，无提前撤销证据。20 秒日志窗口被每秒约 86 条 grammar 记录挤出，不能从缺失的 compile 记录推断没有编译。

现场与代码共同指出待证伪的测试差异：`Harness.frame` 每 10ms 无条件调用贡献同步，反复增加 grammar generation 并重新准备尚未完成的 grammar；正常 `EditorApp.render` 只因 provider generation 或待同步标记变化才同步。下一轮只将测试驱动改为 `window.refresh()` 与正常绘制，由真实 render 处理这些边界；不改生产、请求期限、20 秒观测或任何业务断言。只有取得新实际结果后才判断该调度差异的影响，不提前宣称根因或 GREEN。

### 04:38 外层候选与故障全链 GREEN

只改变测试帧驱动的对照实际 **GREEN parent 164.27s / child 164.09s**，`target/s5-native-outer-normal-frame-solo.log`。程序 `target/s6-native-normal-frame/deps/editor_app.exe` SHA-256 `706332eba442bf2ccb266fbe23b33b7f49c84d2cb43be487a25e0c2ebcabb399`，仍为 TS `.14` / Vue `.11`，所有生产源码、原期限和断言与上一轮一致。测试源码 `4ea09524a20e0730a85bd20d6411ad1175d73d8cb3ebd8d51315395086ac0e2d`，逆转唯一 frame hunk 后精确恢复 `9b63a5312ccfbe759b1e4db23f74d9179f67cd13667c49f86229676fa835487a`；两轴独立复核无新增问题。

首 20 秒日志从约 1,778 条降至 10 条，没有记录遗漏；保留 Vue 主组件编译 26,127ms、TS 24,849ms 和后续 Store 复用。真实三份 Vue 上下文完成具体类型补全和诊断；Preparation／Contract 候选拒绝、反向 TS provider 候选拒绝均保留旧包、三份原分析及私有记录；实际 EOF、正常有界重连和重新适配、脏类型变化、旧结果拒绝、健康其他上下文保持、撤销信任、checked shutdown 与原 HWND 清理全部通过，scenario 与 cleanup 均 `Ok(())`，完整验收收据存在。

该单变量结果确认强制同步的测试驱动会制造重复语法准备并干扰初始场景；不据此声称修改了产品冷启动预算，也不外推独立 S4 初始超时的唯一原因。当前使用该冻结程序，在上述 checked cleanup 后独占补验尚未完成的完整四冲突场景，原初始 127.22s 失败保留。

### 04:44 四冲突完整 GREEN 与最终门禁

完整 S4 场景实际 **1 PASS / 0 FAIL / 0 ignored、280.28s**，`target/s4-conflicts-native-after-native-cleanup.log` SHA-256 `670ebf654e48a2ebf410a2f3e71d80fb423975b91d1cf0b55d36301cc96c61eb`。程序仍为 `706332…b399`，TS `.14` / Vue `.11` 及五项 SDK 4af 生态输入均按原摘要；没有修改此场景、生产、原 180s 恢复预算或严格错误断言。四类分别为 exclusive、ordering cycle、configuration conflict 和不保留原始源块的 mapping；每次卸载后均恢复四角色 effects 和原文 F12。Vue Availability 保持原 3/3，卸载 revision 为 `14→18 / 34→38 / 52→56 / 70→74`，已证明本次失败缓存失效和重新准备链，而非依靠重装 Vue。

结尾实际 checked shutdown ACK 通过、原测试进程已物理退出；只读归属样本未发现该私有 profile 与项目的进程残留。原 127.22s 初始前置失败保留，不伪称已经定位其唯一成因。详情和实际证明边界见[单一验收记录](vue-typescript-s3-s4-native-acceptance-2026-10-20.md)。

最终源码开始依次执行格式、非 UI workspace、workspace check 与正常宿主构建，日志 `target/s6-final-delivery-{fmt,non-ui,check,build}.log`。此前两次被停止的门禁仍不计完整通过。普通桌面所用候选与最终支持元数据／包将分别保留身份，桌面未通过前不提前提升正式支持或交付状态。

### 05:04 最终宿主门禁与 Worker 文案回归

最终 `cargo fmt --check`、`cargo test --workspace --exclude editor-app -j1`、`cargo check --workspace -j1` 均实际通过；非 UI 为 **85 个成功 suite、319 PASS / 0 FAIL / 327 ignored**，编译 8m59s，ignored 不计通过。check 8.77s，正常宿主 `cargo build -p editor-app --bin editor-app -j1` **PASS 2m19s**。冻结 `target/s6-final-delivery-native-host/editor-app.exe`，SHA-256 `996bf4fdbb8fcf79a4949c6ae7fe3035c1cfe1b53e3de6b1d48badf52f498448`。这些日志未覆盖尚未完成的最终插件包与桌面验收。

补充 Worker 普通过滤发现旧英文断言：实际 **5 PASS / 1 FAIL / 8 ignored、2.24s**，单独运行该例亦 **FAIL 0.04s**。当前实现使用 `plugins.command_owner_changed`，catalog 为当前的“replaced or disabled”，旧测试仍找 `Plugin command owner changed`。只修正测试为当前 catalog 加 `StaleRevision`，插件归属、Error 日志、manager-wide inspect 错误和 checked shutdown 保留。第一次修正后普通 **6 PASS / 0 FAIL / 8 ignored、1.68s**。

Standards 又找到同一文件的取消准备 ignored 屏障仍用旧 literal。其原入口实际 **FAIL 126.70s**（`target/s6-worker-trust-catalog-red.log`），仅报告通用 `wait_for` 超时，没有 backtrace，故不虚称已精确证明最后屏障阶段。统一修正后源码 `worker_tests.rs` 为 `ff6902558fd918d86c8953a740dc251cc445bd4bac069ed2a0df950d641e460f`，两轴静态复核无新增问题；格式重排已按 rustfmt 修正，最终 `cargo fmt --check` 再通过。普通全过滤再次 **6 PASS / 0 FAIL / 8 ignored、1.62s**（`target/s6-final-worker-catalog-all-green.log`）。安装进度中英文 ignored 用例已显式 **1 PASS / 0 ignored、0.14s**（`target/s6-worker-progress-locale-final.log`）。

两处仅为 `#[cfg(test)]` 期望更新：非 UI 门禁排除 editor-app，normal/check 不编译该测试模块，未改它们冻结的生产源码。当前完整测试程序 `target/s6-final-delivery-tests/deps/editor_app.exe` SHA-256 `84b78e9e054d8e2af3d0f8365ac95cd24f108ee5aff5c447f51082488c8d0a6c`；随后显式运行取消准备 ignored 原例实际 **FAIL 129.11s**，`target/s6-worker-trust-catalog-green.log`。文件名 green 不代表通过。设 `RUST_BACKTRACE=1` 后仍只取得 `wait_for:212` 的通用 publication 超时，未取得调用者帧，具体失败阶段未知，不能宣称最后屏障已通过或根因已确定。

暂停前更正输入定位：此例使用裸 ID `archive("capability-example")`，`tests/support/packages.rs::input` 取账本首个同 ID，实际输入为 **`.17.1`**，SHA-256 `20f5d815b109f46ab54e22bf56ccd89497c83fd4b2619250294e1a02cad249d8`。此前读到的 `.19.1` / `e9313adf…b46b` 不是此例输入，不能据它诊断本次失败；未更换夹具或账本。

### 05:08 暂停并保存

按最新暂停要求，已停止所有工单实施、诊断和代理；测试进程已退出，未开始最终打包或普通桌面验收。本轮没有提交、推送或关闭新 #122–#126。三仓修改和既有失败／成功日志继续保留；实际门禁、剩余失败、冻结程序及恢复顺序见[暂停检查点](vue-typescript-paused-2026-10-20-0508.md)。恢复须收到新的继续指令。
