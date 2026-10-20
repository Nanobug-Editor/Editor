# Vue / TypeScript 剩余场景覆盖映射

> **2026-10-20恢复更新：** 用户已恢复新#122–#126，详见[恢复记录](vue-typescript-resumed-2026-10-20.md)及[普通桌面记录](vue-typescript-desktop-2026-10-20.md)。S4整项280.28s、S5 outer164.27s、普通七项操作/独立整组Undo/真实OS中文IME/滚动字号/TS自动导入/B选择、配色及同符号Escape→Ctrl+I均已实证。最终normal与三门禁/SDK通过，T4/V5精确范围supported功能元数据完成独立双轴0项发现。E78–E82进一步闭合最终精确双包静态资源、实际mixed pair 1/0/0（97.62s）、public policy 1/0/0（179.74s）、版本策略5/0/0；当前行为和技术门禁收口，尚待三仓commit/push/远端与新issue回执，不提前称全部工单交付。旧RED/partial及既有AccessKit窗口退役abort限制保留，不声称普通退出exit0。

> **05:08 历史暂停保留：** [暂停检查点](vue-typescript-paused-2026-10-20-0508.md)保存了当时附加 ignored 入口仍 FAIL 129.11s、尚未打包及尚未交付的状态。恢复后的结果追加记证，不删除旧失败，也不将旧输入、静态审查或文件名 green 改称行为通过。

**最新读回：2026-10-20恢复批次（Asia/Shanghai，终态截至E82）。** E01–E77证据行及旧FAIL逐行保留；新增E78–E82与末尾35AC/V表及剩余清单为当前盘点。普通桌面结果来自根代理实际操作和记录，维护者只读文件/日志/收据、本轮只改本文，不运行Cargo/Node/UI。B1/B2/B4及B3最终归档/策略、B5独立审查等技术门槛均已闭合；首轮旧待验按历史保留，以末表为准。新#122–#126的Git交付及issue回执仍待实际完成，旧#108–#118和父#107不变，不创建Release。

日期：2026-10-19，Asia/Shanghai。**本表是只读源码、已落盘日志及包输入的覆盖盘点，不是验收完成声明。** 主体观察截止到本机 23:40；23:45 另读回新增 E20。后续编译和运行中的任务不得由本表推定结果。盘点者没有运行 Cargo、Node 或原生窗口；没有修改既有验收记录。

依据为 [S2](../tickets/vue-typescript/scenarios/02-edit-and-refactor.md)、[S3](../tickets/vue-typescript/scenarios/03-sfc-language.md)、[S4](../tickets/vue-typescript/scenarios/04-ecosystem.md)、[S5](../tickets/vue-typescript/scenarios/05-recovery.md)、[S6](../tickets/vue-typescript/scenarios/06-joint-delivery.md)，以及[规格第 7–13 节及 V01–V14](../specs/vue-multi-backend.md)。当前实施另见[剩余场景记录](vue-typescript-remaining-2026-10-19.md)。#121 已交付；只推进新 #122–#126，旧 #108–#118 和父 #107 不变，不创建 Release。

## 固定身份和记法

本节P0/P1/P2保留10-19首轮输入身份；后续P3/P4和E53–E82各按实际包/程序记证。37fd普通语言/编辑、7ce52d卡片测试、43c1inline测试、a45普通深浅配色、beed键盘/最终pair和1cae普通键盘复验分别记录；E60候选与E79最终pair恰同97.62s但归档、程序和日志独立，不互相套用身份。

- 宿主根为 `C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor`，分支 `codex/vue-typescript-remaining`，HEAD `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 加本轮未提交源码；不是该 HEAD 本身的已提交实现。下文 `target/...` 均相对这个根。
- `TS/` 为 `C:/Projects/RustProjects/TypeScript-for-Nanobug`，HEAD `873e8c3d3ebe3a86a8bb1e028a3c4bcd1c557b94` 加未提交候选；`Vue/` 为 `C:/Projects/RustProjects/Vue-for-Nanobug`，HEAD `f8e5880bf89161f16723ae7be1c76458d50a676c` 加未提交候选。
- **P0**：已发布 S1 的实际 TypeScript `.10` / Vue `.10` 包；SHA-256 分别为 `fbc363cb36e67f5583449628b428ed77f2dbcb0fe9afe759ae0c18b15743cff5` / `59cc1e3ae71c5e0897df096eeec88269ff8f5a6433af0e1448c931dcc8f39ee4`。本轮已有宿主回归仍使用这些包；不能把新宿主的通过写成 `.11` 包通过。
- **P1**：两个独立仓当前 `.11` 源码。TS 曾封印 16 个 native 文件到服务 ZIP `1d89ed0c86f7e9b98b0bca55441e1b80db5c0b8b8029200da46a1620796770ad`；后续恢复修复必须重新比较并封印。Vue 当前模块 legacy `f021e8dd903fc37d956451c6faf7392847eba45a57a49529436b0ea44bc10c1e`、modern `a468cf03e3da4b77f326884661e847d63a0ff16f59491dffa0e7342ad738644b`。截止时没有找到最终 `.11` ZIP；工具层通过不等于 WASM/Package/宿主通过。
- **P2**：独立 ecosystem `.1` / 同 ID without-style `.2` 夹具，由公开 CLI、旧 SDK `9187f5a734591977dc2f6bce15402d3bae6a99a26d63eebd41dcc2d1ba4a38b1` 构建；详情在 `target/s4-ecosystem-packages-9187.json`。combined `.1` SHA `ecf9fd0db42295a19532dc1c441c34516ea18227fd41836e35a8e1e134106499`。修正重复贡献 ID 后的 bad-mapping 包另在 `target/s4-ecosystem-invalid-mapping-unique/`，SHA `a023e4f9c91c4ce72eb9004e2f02cb6d95a943a5532e941bb1a7a84f7c123e08`；不能误用账本中先前重复 ID 的包冒充不可逆映射测试。历史 `.0` ZIP 保留，SHA `72c4a403c662f58e28d11a0fb148ef081e73327c41b8b9dd9a7151c1cc5bc429`。
- **PASS** 仅表示对应日志、代码时点及输入实际通过；**FAIL** 区分产品失败与未成立的测试前置；**待跑** 包含源码已写但无运行终态。全部 AC 仍需按最终源码、SDK 和归档身份补齐。文件名包含 `green` 不构成结果。
- 本人编写的 Vue 外部脚本/历史语义、ecosystem 角色和格式化/IME 测试在本表只说明接缝、断言与执行日志，**不计为对自己实现的独立规范/规格审查**。2026-10-15 paused 记录及 S1 旧候选证据只保留其原身份。

## 已读回的实际证据

| 代号 | 入口、结果和输入 | 日志与证明边界 |
| --- | --- | --- |
| E01 | `real_typescript_rename_opens_unknown_targets_previews_cancels_and_group_undo`，PASS 1/1，71.52s，P0 TS | `target/s2-typescript-receipt-green.log`。公开 Package/Manager、原生文档、未打开目标、只读全拒绝、预览取消、辅助源冲突、提交、分组 Undo、真实选中补全的 resolve 和附加 import。不是 Vue 历史写法、未知语言或延迟 resolve 失效测试 |
| E02 | `mixed_vue_projects_share_one_unsaved_typescript_dependency`，PASS 1/1，129.22s，P0 双包 | `target/s2-mixed-shared-first.log`。三版本共同读取脏 TS、准确诊断、主 TS 提供者和磁盘不变；不含歧义选择/来源继承 UI |
| E03 | `notification_overflow_resynchronizes_unchanged_live_sources_before_diagnostics`，RED→PASS 0.16s；`runtime_log_tests::notifications` 4/4 | `target/s2-overflow-{red,green}.log`、`target/s2-notification-regressions.log`。最后事件丢失时重建完整水位，不把四项与该单项简单相加 |
| E04 | `paged_related_graph_synchronizes_every_large_native_source_without_truncation`，PASS 1/1，6.52s | `target/s2-large-graph.log`。260 份各大于 64 KiB 的原生源、分页完整；不是独立控制 RPC 的额度扩大 |
| E05 | `editor::language_edits::completion::tests`，PASS 4/4；`packaged_rename_rejects_illegal_tail_without_exposing_a_valid_prefix`，PASS 1/1，1.07s | `target/s2-completion-boundaries.log`、`target/s2-full-edit-decoder-fixed.log`。前者是编辑解码纯函数；后者是真实未知 Package/stdio 的非法尾部全拒绝，不能倒推未知语言实际补全选择已验证 |
| E06 | `independent_package_registers_owned_context_references_and_rejects_stale_generations`，RED→PASS 1/1，24.73s | `target/s2-context-registry-green-third.log`，`context-selection-fixture-0.1.0.zip` 经 SDK 9187 构建。只证明引用注册/代次/归属，不证明 QuickPick、偏好落盘和真实 F12 来源继承 |
| E07 | `vue_prop_rename_discovers_parent_previews_cancels_and_group_undo`，原 FAIL 153.30s→新 phase 宿主 PASS 1/1，114.80s，P0 双包 | `target/s2-vue-prop-rename-three-versions.log`、`target/s2-vue-prop-rename-phase-green.log`。普通三版本 Child prop 的安全事务；未包含 class/JSX/TSX 八行历史矩阵 |
| E08 | `committed_native_recovery_survives_analysis_expiry_but_not_owner_retirement`，RED→PASS 1/1，5.63s | `target/s2-consumed-analysis-guard-green.log`。待确认条件失效应拒绝；已提交 Undo 仅依赖持续来源授权。真实 stdio/DocumentSession，不含实际 Vue 生态扩展更新 |
| E09 | Vue `node --test --test-concurrency=1 tests/*.test.cjs`，source 102/102、sealed 102/102、均 0 skipped | `Vue/target/remaining-s3-source-011-final.log`（114.898s）、`remaining-s3-sealed-011-final.log`（105.307s），P1 Vue 模块 + TS 5.9.3 / Node 24.19.0。两种加载模式是同一套 102 项，不是 204 个独立场景；不是正式 WASM/宿主/事务应用 |
| E10 | TS 工具套 source 44/44，sealed 44/44、0 skipped；后者 49.69s | source 仅既有工具返回，未另存日志；sealed `TS/target/s2-context-typescript-node-sealed.log`。对应 P1 当时封印；当前恢复修复后须重新执行，不把源码改动自动继承为 PASS |
| E11 | ecosystem `node --test --test-concurrency=1 test/fixtures/ecosystem/ecosystem.test.mjs`，12/12、0 skipped，21.538s | `TS/target/s4-ecosystem-source-12.log`；前后 16 个 native 文件摘要未变。真实 TS-owned 引擎和封印 Vue factory 的角色/组合/冲突效果；不是 WASM 服务发现和 Package 权限证据。README 中较早 11 项是旧时点，不覆盖新增 missing-style 一项 |
| E12 | `independently_packaged_ecosystem_checkpoints_disables_and_reenables`，历史 `.0` FAIL 23.92s→P2 `.1` PASS 1/1，47.78s | `target/s4-ecosystem-lifecycle-{red,green}.log`。Snapshot/schema1、disable/enable、checked shutdown、磁盘不变；不含四角色实际宿主语义，也不是最终 SDK 重建结果 |
| E13 | `selection_formatting`，实际 2 PASS / 1 FAIL，34.79s | `target/s3-native-range-green.log`。准确 Unicode/CRLF 范围+single Undo、无 range 支持不回退全文已通过；IME 项失败在“回复仍未返回”前置，**不是生产 RED**。已换真实 Windows child，同测试名尚待编译/运行；350ms 延迟没改 |
| E14 | `vue_package_native_block_grammars_and_captures`、`vue_package_native_queries_missing_blocks_and_withdrawal`，PASS 2/2，3.07s | `target/s3-native-grammar-preflight-010.log`，P0 双包，生产 native Tree-sitter loader/查询、8 种块语法及缺失/撤销。P1 终包须比较资源摘要或复放 |
| E15 | `vue_editor_native_highlights_themes_keyboard_and_ime`，PASS 1/1，48.17s | `target/s3-native-grammar-theme-preflight-010.log`，P0。真实 EditorState 绘制、深浅色解析、程序化键盘/IME bridge；源码未见滚动/缩放场景，不声称 OS 候选窗或普通桌面完整验收 |
| E16 | `old_context_capture_is_not_live_after_native_pipe_recovery`，RED→PASS，最新 phase 1/1，1.09s | `target/s5-connection-{capture-green,origin-phase-green}.log`。真正 pipe incarnation 即使重复 graph/wire 代次也不复活旧 capture；不证明 Vue 模块恢复 |
| E17 | `real_vue_backends_recover_without_reviving_old_results`，最近终态 **FAIL** 1/1，115.76s，P0 | `target/s5-native-vue-recovery-initial-corrected.log`。退出前 A/B 具体语义、B 独立保持已触达；A 恢复后真实补全 “No content available”，没有通过后续信任撤销。首次 2339/2551 预期修正另留日志。源码正修复，未拿进程重启当语义恢复 |
| E18 | 两插件公开当前 SDK、最终包闭合 | 最近 Vue 公开 `--plugin-cargo ... check` 为 FAIL E0583，SDK `5e66d563...` 缺 `language_context/registry.rs`，`Vue/target/remaining-011-public-sdk-check.log`。导出表已修；`sdk_export::tests` 6/6 PASS，`target/s2-phase-sdk-export-tests.log`。重建宿主后的同公开入口、两个 `.11` 实包及最终 SDK 尚待，不手工补缓存 |
| E19 | S1 生命周期/缺失提供者/策略/普通桌面 | [S1 交付记录](vue-typescript-s1-delivery-2026-10-19.md)保留 P0 与旧 SDK 身份。原始 A/B、last-peer、1000ms native 控制、策略矩阵等有历史实际 PASS；不是本轮改过引用/事务/恢复后的最终回归 |
| E20 | `in_flight_range_formatting_rejects_ime_preedit_even_when_bytes_return`，真正 Windows parent PASS 7.96s / child PASS 1.54s；23:45 补读 | `target/s3-native-inflight-range-first.log`。同一场景不重复计数；实际 wire request 2 仍 pending，1.603ms 内原生 preedit/cancel，revision 0→2、字节恢复；晚结果不应用、fresh exact UTF16、same Arc / initialize1 / range2 / whole0 / Undo / disk / checked shutdown / HWND 退出均通过。350ms 原 delay 保留。不是 Vue `.11` 实包 style 格式化或 OS 候选窗证据 |

## 首轮逐场景 AC（历史时点，当前见末表）

以下“尚缺”是证据或测试覆盖缺口，不能仅凭未运行推断产品实现错误。

| AC | 当前对应测试/实际结果 | 足以证明要求的部分与尚缺 |
| --- | --- | --- |
| S2-AC1 | E02 PASS；E06 PASS；`shared_typescript_choice_is_native_scoped_and_retains_navigation_provenance` 已写、待跑 | 脏共享源有实际证据。后者覆盖 QuickPick、取消、本机偏好、A/B 精准撤销；其中来源继承是显式调用生产 `inherit_source_language_context`+`open_definition_uri`，仍需普通 F12 从来源项目进入共享文件的实际输入，不能把 helper 调用当完整导航入口 |
| S2-AC2 | E03/E08/E16 PASS；TS `control.test.mjs` 的配置代次/精确 revision，E09/E10 旧时点 PASS | 新选择 UI、关闭重开与 late completion/preview 的最终包联验待跑；配置/owner/映射变化发生在真实 resolve 或 preview 等待期间时须整组拒绝 |
| S2-AC3 | E03/E04 PASS | 对应大图及水位恢复断言直接覆盖要求；当前最终同步/恢复代码改过，需同入口最后复放。控制 RPC 独立额度保持 |
| S2-AC4 | E09 外部 src/ownership/static exports/CRLF/UTF16；E14/E15、P0 S1 路径策略；E20 原生 pending IME PASS | 工具样例有精确原始 spans。多 tsconfig/paths 不等于 project references 或 pnpm 链接布局验收；后两项需实际解析导航样例。外部 src 的 P1 实际包未验 |
| S2-AC5 | E01 真实 TS 正例 PASS；E05 4 项纯函数 PASS；E09 历史 auto-import 8 行为提案 | 仍缺真实 Vue 的选中 resolve→主编辑+import→Undo，及未知语言经公开链的非法/过期附加编辑全拒绝；真实 resolve 期间来源/附加目标变化不能由纯函数测试代替 |
| S2-AC6 | E01/E07 PASS，均 P0 | 普通 TS/三版本 prop 已经过预览取消/未打开目标/提交/分组 Undo。八行历史 class/JSX/TSX 和转换映射事务需 S6 组合；P1 最终包复放待跑 |
| S2-AC7 | E01 只读/辅助冲突；E05 非法尾部全拒绝；E08 owner/revision phase PASS | 通用 `workspace_transaction_preview_rechecks_disk_native_version_and_closed_target`、`workspace_transaction_later_typing_and_local_undo_refuse_all_batch_restoration` 等现有 SDK 回归待当前契约复放；缺必要映射/扩展更新后的旧 confirm 由 S2+S4/S5 联验，不能仅凭前置 map 校验通过 |
| S2-AC8 | E01/E07/E08、E13 两个原生文档用例 | 源码沿唯一 EditorState/DocumentSession 和公开事务。未知 Package 的 SDK 多范围 Undo、旧单文档/提供者选择回归须最终显式执行；源码结构核对与行为验收分别记录 |
| S3-AC1 | E09 `compatibility.test.cjs`、`syntax.test.cjs`、`legacy-setup.test.cjs` | 声明的 26 类固定组合、JS/TS、Options/Composition、2.6 扩展 1.7.2、2.7/3 setup 及 2.6 setup 拒绝有工具层证据；P1 受管 Package 和最终支持清单仍待。不得把 2.6 的 script setup 算合法 |
| S3-AC2 | E09 class-decorators×2、JSX×3、TSX×3；`auto-import.test.cjs` 8 行 | 真实 tsserver 的各自 script/render 有效/非法语义、props/slots、源跨度 refs/rename、格式化/自动导入提案已测；宿主八行安全应用、预览/取消/Undo 和 Vue 实际补全选择待新增/执行。此测试为本人实现，仍需独立审查 |
| S3-AC3 | E09 `external-src.test.cjs`、`external-ownership.test.cjs` | 仅 SFC admitted `script.src` 所属外部 JS/TS 的 literal/const/已知 wrapper 有界映射；普通模块及跨项目共享源负例、脏 src acquire/release已测。任意动态工厂/继承未承诺；P1 密封受管实包、路径别名/链接布局联验仍待 |
| S3-AC4 | E14/E15 P0 PASS | 生产 loader、独立子 grammar、缺失/停用和主题绘制有证据。P1 grammar/查询闭包需与 P0 逐项比较并复放必要入口；未知块不借其他 grammar |
| S3-AC5 | E09 format/styles 工具 PASS；E13 两项通用原生范围 PASS；E20 pending IME PASS；`vue_formatting_tests::{matrix,boundaries,selection}` 四项已写待跑 | 三 Vue×CSS/SCSS/Sass/Less、whole/range、不越块/属性/src、无保存+Undo、独立 TS formatter 未覆盖、旧结果拒绝都已有测试入口，尚无实际 P1 宿主运行终态。E20已关闭通用range异步IME时序缺口，不能外推Vue样式矩阵已通过 |
| S3-AC6 | E11 missing-style 工具 PASS；E14 missing grammar、E15 主题/焦点/键盘/IME PASS | 缺失角色的原生准确状态仍待 S4。滚动/缩放、原生提示中英文需补可见证据；现 test 无相应动作，不能由主题测试外推 |
| S3-AC7 | E09 封印 loader PASS；E18 当前 SDK 缺文件 RED 后源码已修 | 最终两插件公开 SDK 构建/Package admitted、全闭包来源许可摘要、无开发依赖的检查待完成；双轴审查须覆盖最终候选，不审自己结论 |
| S4-AC1 | E10 composition/role 工具套；E11 PASS；角色 SDK/Worker 测试已写 | 四角色元数据、collection、单选/组合接缝已存在；异步 prepare 不在只读 hook 调服务需源码审查+最终实际包发现证明，不能用普通 service Open 或 A/B 消费者代替 |
| S4-AC2 | E11 12/12、P2 public admission；E12 Snapshot PASS | 真正 Quill 转换、Ink 源绑定、项目布尔检查和 TS 2345 类型约束有工具证据；`four_ecosystem_roles_map_native_queries_edits_replacement_and_removal` 尚待生产 Worker 实包运行，配置/项目磁盘不变有断言 |
| S4-AC3 | E11 源位置和非法 map PASS；roles test 已写 F12/refs/rename preview/cancel/apply/Undo | P1/P2 Package 实际组合待跑。**拿到有效映射提案后改变源/映射代次，或更新/撤销扩展后确认旧提案**尚需联验；预先拒绝 invalid-map 不能替代这个时序 |
| S4-AC4 | E11 四类 conflict+有效重试 PASS；TS composition/adapter ABI sealed 测试 E10 | 当前 host conflict test 仅 exclusive 与 invalid-map；ordering cycle/config conflict/错误配置/ABI/引擎冲突已有 Node seam，最终 Package 角色链至少补其准入/恢复观察，且不得按安装次序覆盖 |
| S4-AC5 | roles replacement/removal 源码待跑；E19 原 A/B/last-peer/多跳仅原身份 | 旧 server、lease、plan process0、其他角色继续均有断言。原始 A/B 及 `delegated_resources_and_async_continuations_follow_the_original_source_lifetime` 必须按最终公共契约显式复放；不能拿 Node retirement 或 resource count 单独证明物理 EOF |
| S4-AC6 | E11 without-style PASS；roles replacement `.2` 待跑 | 工具已证明仅 Ink 缺失而 type/template/project继续；实际原生状态/其他功能及缺失 grammar 的独立性待最终包。Quill/Ink 是测试语法，不宣布 Pug/Stylus/Nuxt 正式支持 |
| S5-AC1 | `managed_dependencies::checksum_network_and_cancel_failures_leave_the_old_version_available`、`data_migration::{failed_migration_activation_and_commit_preserve_old_data,interrupted_commits_recover_before_any_guest_is_activated}`、`hot_update::mixed_update_rollback_replaces_all_owners_and_preserves_latest_data` 均已有、当前未运行 | 这些是可复用真实 Package/Manager 事务入口，不能因普通 workspace 默认忽略 WASM 就算通过；本轮实际 TS/Vue 组合候选失败后旧语义及私有偏好保留尚缺 |
| S5-AC2 | Vue compatibility 账本仍 candidate；E17 仅退出恢复 | 缺真实不兼容 TS/Vue 更新或拒绝切换后，旧 Vue2.6/2.7/3 能力保留/恢复的矩阵。未知未来版本的限制不能代替已承诺历史组合的升级回归 |
| S5-AC3 | `managed_dependencies` 的 offline/cache/checksum/cancel/另 workspace lease、`waiting_for_another_installation_is_cancellable`；`optional_language_dependencies`；当前未运行 | 通用依赖故障/去重/收据已有直接实际下载接缝，可最小复用；需当前源码显式运行并保留状态、缓存/项目不改证据。包内 artifact 离线读不等于网络下载取消已测 |
| S5-AC4 | E16 PASS；E17 最近 FAIL | 必须重跑普通生产恢复并重新观察具体 Vue 类型能力，A退出时B原 server/plan不变。普通业务错误/取消同连接可用的 E19 回归亦需 final复放；不能把 startup ACK 或新进程存在当语义成功 |
| S5-AC5 | E12 Snapshot PASS；P2 roles 更新/卸载待跑；native recovery 源码后段含 trust+process0但 E17 未走到 | 真正 exporter/recipient 更新/卸载/崩溃、撤信任和 workspace close 的旧 channel/request/map/diagnostic/引用封闭及缓存/租约清理仍需 final联验。S2/S4 的 pending proposal/resolve跨该边界须另测 |
| S5-AC6 | E06/E08/E16 PASS；`real_sdk_hook_routes_subprojects_and_retires_only_changed_context`、`language_contexts` late/cancel 回归；共享选择UI待跑 | 当前代次/连接封印已加强；最终包的 A重选不退B、关闭重开新ID、乱序prepare不回写及通知水位需串起实测，源码存在不计通过 |
| S5-AC7 | 新配置、状态/locale 源码；E15 仅 grammar bridge | 中英 prepare/start/index/failure/recovery、版本证据/项目根/恢复入口、一次授权而非正常请求重复授权，缺本轮完整原生可见验收。纯 locale key检查不足 |
| S6-AC1 | 本表 V01–V14 对照；基础单场景有上述部分 PASS | 三种明确联合要求仍未全部执行：历史写法重构、转换映射事务、更新/撤销中的旧结果全拒绝 |
| S6-AC2 | E18 SDK6项PASS；当前若干 no-run/check已有，不是全部门禁 | 最终 `fmt`、非UI workspace test、workspace check、app定向、显式ignored实包和最终Node套须在源码冻结后实际执行；S1 318 passed/322 ignored不可追记为本轮通过 |
| S6-AC3 | E01/E07 原生文档事务，E15 程序化绘制输入；S1普通桌面只读F12/refs | 本轮普通 Windows 桌面的补全选中、诊断/hover、F12/refs、preview/cancel/Undo、format、共享上下文选择、状态恢复及相关IME/滚动/缩放仍需可见结果证据；不把 no-run或tsserver 输出当UI验收 |
| S6-AC4 | E18 SDK export修复；P1源版本、P2旧SDK包已记录 | 最终SDK与两个`.11`归档尚未闭合；公开metadata workspace_root/target、Cargo.lock、许可/闭包/摘要/compatibility正式与限制账本、消费夹具重建待完成 |
| S6-AC5 | 本表为覆盖盘点；未完成最终双轴审查 | 新增/未跟踪路径需进入固定点审查，测试作者不自判独立通过；main原Drawing/WIP不得混入，最终SDK/包须与被审源一致 |
| S6-AC6 | 既有历史/未来准入范围及不可恢复进程内存限制仍保留 | 当前 offline/update/rollback实测未齐；最终支持清单和失败边界据实更新，macOS/Linux仍未验收，保留历史夹具及用户私有数据 |
| S6-AC7 | 新 #122–#126 仍 open/实施状态；无本轮普通提交/推送关闭回执 | 测试与独立审查通过后才普通提交、push、核远端、读回对应新issue；父/旧issue不动，无Release。不得由本覆盖文档提前勾选完成 |

## 首轮 V01–V14 到证据与缺口（历史时点）

| 规格项 | 当前覆盖 | 最小未闭合部分 |
| --- | --- | --- |
| V01 | E09 工具102；E07普通prop事务；S1 P0五项混合语义 | 最终P1三版本七项实包+非法语法、唯一诊断；format/refs分别有明确入口，不能一次进程成功外推七项 |
| V02 | E02共享脏TS；E06注册；TS shared-context工具；共享QuickPick回归待跑 | 实际歧义选择/本机持久化/取消、A来源F12继承、B上下文不退役、最终包 |
| V03 | E09 26类固定组合、Vue2扩展1.7.2、2.6setup拒绝 | P1受管包及声明范围逐行账本；不能把父模板title查询外推到任意动态写法 |
| V04 | E09 class2+JSX3+TSX3自身script/render及8行auto-import提案 | 对应8行native事务/真实选中resolve+import；缺Vue3类组件声明组合不凭名称推定支持 |
| V05 | E09 external-src/ownership；TS控制路径/配置；S1规范路径策略 | project references、路径别名到外部src、合法pm链接与越界链接的实际解析/导航联验 |
| V06 | E02/E03/E04、E09脏Vue/外部源 | 依赖变化叠加旧补全/映射提案全拒绝，final源重放 |
| V07 | E04大图；E09/E13/E15 Unicode/CRLF/原生IME bridge；E20实际pending range IME child PASS | 定位/编辑边界final包；E20为通用未知formatter时序，非OS候选窗；无静默截断 |
| V08 | E01 TS、E07三版本普通Vue事务PASS | 历史8行与生态mapped事务、过期preview、最终包统一Undo |
| V09 | E09 styles/format；E14注入；E13通用range；Vue实际包4项待跑 | 3×4 style的whole/range原生结果及formatter偏好；缺角色原生明确状态 |
| V10 | E11角色12项、E12 Snapshot、P2多variant admission | 真实Worker角色组合/竞争/替换/卸载、旧映射提案失效、A/B多跳final回归 |
| V11 | 通用 managed_dependencies/optional_language_dependencies现有直接接缝 | 本轮显式执行真实下载/离线/取消/坏摘要/去重和租约；不能复用paused全过结论 |
| V12 | E08/E16、E17失败；S1历史多消费者 | 真实TS/Vue不兼容更新/回滚+偏好、生态pending edit/resolve跨更新/撤权、进程及缓存清理 |
| V13 | S1实际缺TS/错误SDK/受控Rust Vue策略（E19），E09版本证据工具 | 最终公开SDK/Package复放及中英原生状态；不将Node resolver当Rust策略证据 |
| V14 | E05未知编辑Package、E06未知上下文、E11/P2生态、S1控制/module生命周期 | 同一最终SDK重建显式ignored；未知语言实际补全resolve原子失败仍缺，宿主不得加插件ID分支 |

## 首轮最小后续执行矩阵（历史时点，当前见末尾剩余清单）

这是待执行清单，命令仅描述入口；本盘点没有执行它们。新的测试名未产生前写“待补”，避免一个不存在的过滤词得到 0 tests 却被记录为通过。原生/WASM与大 Node 套按根代理统一排队。

1. **公共契约/产物闭合。** 等最后的注册/事务phase/恢复源码冻结，重建普通宿主；从其公开入口导出完整SDK，两个独立插件分别检查 metadata，再用 `--plugin-cargo ... check --locked --offline --release --target wasm32-wasip2 -j1`、`--plugin-package . --output <独立候选目录>`。TS按其README直接staging/归档，Vue以现有`build.cjs`+`package.cjs`生成密封闭包，禁止历史scripts/手改SDK缓存。保存实际SDK、源码和两包SHA。重建context-selection、ecosystem各必要variant及changed SDK消费者；保留旧`.0`/旧`.10`。
2. **基础S2/S3/S4工具与宿主。** Node完整source+sealed、fixture角色12项0 skipped；当前SDK的 context_selection、语言上下文/原始caller/module生命周期；TS完整事务、三版本脏共享/prop事务、共享QuickPick；三版本format与8种grammar。E03/E04/E05/E08/E16为小回归，和最终实现直接相关，保留全部断言。
3. **S2+S3历史组合（待补/执行）。** class2（2.6/2.7）+JSX3+TSX3共8行，使用已有固定依赖；每行真实Package/production Worker拿到完整prop/跨文件rename→preview→cancel无副作用→fresh apply→single/groupUndo，源文件全不保存。至少实际选择Vue补全，resolve后主编辑和全部import一起应用/一起Undo；再加来源或必需附加目标在resolve等待中变化的整组拒绝及未知语言公开链。普通 TS 正例、八行tsserver提案不能代替这些断言。根已安排本项独立测试准备，不表示已实现/通过。
4. **S2+S4映射事务。** 先跑四角色actualPackage测试的Quill/Ink F12/refs/preview/cancel/apply/Undo及替换/缺style；再专门在有效映射提案等待确认时更新/撤销来源，尝试旧confirm必须零修改、无成功前缀，其他消费者保持可用；恢复后fresh提案成功。另以改动源revision/映射代次验证必要map过期全拒绝。根已安排追加时序联验。
5. **S5及S2/S4+S5。** 修复后先重跑真正Windows backend crash语义恢复/受影响A与健康B/有界retry/trust与checked关闭。再真实更新准备/激活失败、private prefs回滚、不兼容TS/Vue切换拒绝或旧兼容组合保留，重跑三版本历史保留矩阵。通用下载/回滚/服务多跳/多消费者可用现有publicManager测试；至少生态旧preview和选中resolve跨update/revoke的全拒绝须放进同一实际Package/Worker流程。runtime已安排真实不兼容更新回归，不以安排当结果。
6. **最终S6。** 最终源码一次强制门禁；显式ignored并读回0skip/失败；独立两轴审查；普通Windows私有profile可见输入覆盖语言功能、上下文选择、状态中英文、相关滚动/缩放与IME。包身份、兼容表、限制、许可、数据保留和平台范围据实收敛后，再提交/推送/核对应新issue，父和旧issue不动。

### 现有入口与环境

在宿主根使用最终编译产物；复制测试程序时同时锁定配对 `examples/lsp_fixture.exe` 和依赖DLL，不能把不同源码快照共用Cargo输出。先 `cargo metadata --no-deps --format-version 1` 核对 workspace_root/target_directory。精确运行入口示例：

```powershell
$HostRoot = 'C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor'
$TsRoot = 'C:/Projects/RustProjects/TypeScript-for-Nanobug'
$VueRoot = 'C:/Projects/RustProjects/Vue-for-Nanobug'
# 下面两项须指向步骤1实际构建、核摘要的最终包，不在本表预造尚不存在的ZIP。
$env:NANOBUG_TYPESCRIPT_PACKAGE = $FinalTypeScriptZip
$env:NANOBUG_VUE_PACKAGE = $FinalVueZip
$env:NANOBUG_VUE_FIXTURES = $VueRoot
Set-Location $HostRoot
cargo test -p editor-app shared_typescript_choice_is_native_scoped_and_retains_navigation_provenance -j1 -- --ignored --nocapture --test-threads=1
cargo test -p editor-app vue_prop_rename_discovers_parent_previews_cancels_and_group_undo -j1 -- --ignored --nocapture --test-threads=1
cargo test -p editor-app vue_formatting_tests -j1 -- --ignored --nocapture --test-threads=1
cargo test -p editor-app vue_package_tests -j1 -- --ignored --nocapture --test-threads=1
cargo test -p editor-app vue_editor_native_highlights_themes_keyboard_and_ime -j1 -- --ignored --nocapture --test-threads=1
cargo test -p editor-app in_flight_range_formatting_rejects_ime_preedit_even_when_bytes_return -j1 -- --ignored --nocapture --test-threads=1
cargo test -p editor-app real_vue_backends_recover_without_reviving_old_results -j1 -- --ignored --nocapture --test-threads=1
```

最后两个为普通 libtest parent，仅它们各自的精确child创建真正WindowsPlatform窗口；`NANOBUG_NATIVE_RANGE_FORMATTING_ROOT` / `ME_EDITOR_PROFILE_HOME` 由range parent设置。range夹具复用账本中 `javascript-0.3.1.zip`（`a09afb41de50211e5e0a866d2698dde2a6d524b54dc74149eda573f42eabe144`）的固定Node来源，无新增外部env；不能直接运行private child或把旧GPUI前置失败记作产品RED。

S4实际包还需 `NANOBUG_ECOSYSTEM_PACKAGE`、`NANOBUG_ECOSYSTEM_COMBINED_PACKAGE`、`NANOBUG_ECOSYSTEM_WITHOUT_STYLE_PACKAGE`、`NANOBUG_ECOSYSTEM_CONFLICT_PACKAGE`、`NANOBUG_ECOSYSTEM_BAD_MAPPING_PACKAGE`，分别指向最终SDK重建的 types `.1`、combined `.1`、同ID without-style `.2`、exclusive-conflict、**unique invalid-mapping**。现有入口为 `cargo test -p editor-app vue_ecosystem_tests -j1 -- --ignored --nocapture --test-threads=1`、`cargo test -p plugin-runtime --test ecosystem_lifecycle -j1 -- --ignored --nocapture --test-threads=1`。其Node入口从TS根设置 `NANOBUG_VUE_FIXTURES` 后为 `node --test --test-concurrency=1 test/fixtures/ecosystem/ecosystem.test.mjs`。

S5通用最小回归入口是 `cargo test -p plugin-runtime --test managed_dependencies --test optional_language_dependencies -j1`。实际WASM更新/回滚需准备固定 `capability-example-0.19.0.zip`，先构建匹配 `lsp_fixture`；`--test data_migration` 分别过滤 `failed_migration_activation_and_commit_preserve_old_data` 和 `interrupted_commits_recover_before_any_guest_is_activated`，加 `-- --exact --ignored --nocapture --test-threads=1`，不能直接运行该文件的 private `crash_child`。`--test hot_update` 同样精确运行 `mixed_update_rollback_replaces_all_owners_and_preserves_latest_data`。多消费者及服务多跳用 `--test native_modules --test plugin_services -- --ignored --nocapture --test-threads=1`，依各ignore说明设置当前SDK的 `NANOBUG_NATIVE_CONSUMERS_PACKAGE` / `NANOBUG_LANGUAGE_DELEGATE_PACKAGE` 等真实输入。各项只证明各自层级，不自动把它们的成功解释成TS/Vue更新组合已验收。

本轮最终强制门禁仍为 `cargo fmt --check`、`cargo test --workspace --exclude editor-app -j1`、`cargo check --workspace -j1`、受影响app测试与上述显式实包/Windows入口。最终执行者需将实际命令、包摘要、进程/文件收据和结果回填新的验收时点；本表不覆盖或改写旧失败历史。

## 盘点后的测试准备（不计 PASS）

八行历史联合场景已补入口 `extensions::worker::vue_editing_tests::historical::historical_vue_forms_apply_native_rename_and_resolved_auto_import_with_undo`：class×2、JSX×3、TSX×3，使用同三个配置图和各自未打开的 parent；每行依次走 script/render 属性 F2、可见预览、取消、再次申请并提交、全部源位置及父组件属性精确后像、分组 Undo，随后实际选中 `formatHistoricalTitle` 补全使主编辑+import 同时应用并一次本地 Undo。临时源在选定 Vue 工作区 `target/` 私有目录；固定与临时磁盘输入均逐字节检查，依赖 junction 仍在该工作区内，checked shutdown 后服务 inactive/process0。复用既有 Package/production Worker/EditorState/DocumentSession，未加宿主 API；仅 scoped rustfmt/diff 检查已通过。**尚未编译或执行，不将入口存在写成 S2+S3 组合已验收，也不代替独立审查。** 原生执行仍用上述三个 `NANOBUG_*` 双包/夹具变量，加该完整过滤词及 `--ignored --nocapture --test-threads=1`；以实际日志最终读回为准。

## 2026-10-20 02:57 增量覆盖

以下截至本机 **02:57:40** 的文件读回；晚于该时点的运行另追加，不能由正在增长的日志预判终态。依据[主实施记录](vue-typescript-remaining-2026-10-19.md)、[双轴审查](vue-typescript-remaining-review-2026-10-20.md)、[S3/S4 实际验收记录](vue-typescript-s3-s4-native-acceptance-2026-10-20.md)、[上下文选择](vue-typescript-context-selection-2026-10-20.md)、[委托取消回归](vue-typescript-deferred-control-cancel-2026-10-20.md)和 `target/s5-runtime-handoff-2026-10-20.md`；新增终态另外直接读取原日志。原失败仍是失败，工具提案、Fixture policy、Package 准入、原生事务和普通桌面分别计证。

### 本时点输入身份

三仓仍以首轮所列基线加未提交候选为比较对象，未将旧 paused 工作树或本轮候选伪称为已交付。下列摘要均为 SHA-256；`TS/`、`Vue/` 和宿主 `target/` 继续采用首轮绝对根约定。

| 标识 | 实际输入、摘要和边界 |
| --- | --- |
| P3：SDK 与双包 | SDK `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`，公开正常 CLI `target/s6-sdk-4af-host/editor-app.exe` 为 `d587836c7a97be4b22eec9fd95ba3778226da8ca877b906d8426d5f819264412`；54 项导出输入。TS `.14`：`TS/target/s4-context-package-014/typescript-0.1.14.zip`，`92c29c83c8fa4e72fea83cfc11f3917773e43845c54a57d24e1f68d8dae5e9c9`；Vue `.11`：`Vue/dist/vue-0.1.11.zip`，`8e665285ac0d2701ba513d2d11ddd63f61771a27e21843e6869f000a7ee16252`。原 `.10/.11/.12/.13` 归档保留，各条证据只沿用实际版本 |
| P3 的闭包 | TS native ZIP `71c2794f0833bb548d365255878362b4637750bdac05e7ff7e8392df73884f83`，156 entries、16 native 文件逐 source/stage/sealed SHA 一致；`TS/target/s4-typescript-014-final-tools-proof.json` 为 `d0a32dbaa0fcc092dfd538dd17a5d0acce62d0ecc9483b1240d29ee9396dc904`。Vue 当前两模块仍为首轮 `f021…c1e` / `a468…644b`，SDK4af 包逐 entry 收据为 `Vue/target/remaining-011-sdk4af-package-receipt.json`。这证明产物身份，不代替每个行为 |
| P4：生态 Package | `target/s4-ecosystem-assets-sdk4af/packages.json` 为 `bd4c2c166b335e0df63450c921a3809f0432cfccd83ac84303b31a637aa03e9a`，12 个 SDK4af 包均经过公开打包准入。types `.2` 为 `ff5ce38d0b3b2e887675c6151d82131ffb01b9ebb4e3724b011925f5919d4399`；combined `.2` 为 `3e8a249f96ee23e9b27ada05bc5bf216e4133dd22aba092dcd8b90702265fb15`；同 ID without-style `.3` 为 `df56dc420157285af63a3fb7771e198a9b04fede02c6aba5cfe49b65e28caac6`。四种冲突包的完整路径/摘要见 S3/S4 记录；不能误用早期缺 `package.assets` 或重复 contribution ID 的包 |
| P5：runtime 输入 | `target/s5-runtime-preflight/inputs.json` 为 `45ce9ff5efb38bfd04e9022e0cc176eb3242e8e096374a8d52f07a64108d2590`，锁定八份当时程序。固定 capability-example `.19.0` ZIP `bdfc5d3327b5b7a4f3d25ba1ae95e2a6a47b62634fb040e17fb4430c9aa46773` 未重建；SDK4af delegate `.0` 为 `a784dae13e887fa18f225504a5af842728309d751da3b5ae30cc6b2c34d5f3d5`、native-consumers `.2` 为 `cb862aaa22a4e90bd4815da41841a1c88c6d1235110d2abb54546a4650352c5b`、service-set WASM 为 `4a34750ff70e618d5d33f79c9b62efd9771f24bcf8b5bd889bbd6a1cc11b6654`。源、metadata、独立 target 与后续观察版 exe 的身份均在 runtime handoff，不能混为一个冻结程序 |
| H3：当前宿主观察程序 | `target/s6-preflight-external-diagnostics/deps/editor_app.exe`，`5832e020b94d7dcf2cbf273acbf2e351f83b612f94e2fe8e9555ff69cd4b1287`；配对 lsp fixture 仍为 `a7e30fb146622239228ed175937b473c0d9343754674707ea828754eb4a76075`。其 no-run 通过 2m56s；它包含历史/external 共享回执观察及此前预检，不把之后 resource-only 修复自动视为已进入该 exe |

共享 F12、历史和生态场景的源码入口为 `TestAppContext` / `VisualTestContext`：真实 Package、WASM、production Worker 和原生 EditorState/DocumentSession，不等同于 WindowsPlatform 或普通桌面。格式化、延迟未知语言及恢复的独立 parent/child 才实际创建 WindowsPlatform 窗口。普通可见桌面的最终范围仍单列在 S6。

### 新证据 E21–E40

| 代号 | 实际终态、输入与日志 | 已证明及边界 |
| --- | --- | --- |
| E21 | P3 公共构建闭合：Vue check 4.30s、package 34.04s；TS `.14` package 23.842s。SDK/归档/闭包见上表及主记录 | 已消除 E18 导出缺文件和早期 canonical prepare 准入失败；保留旧错误 SDK/包。不是所有语言操作的最终联验 |
| E22 | TS `.14` source **48/48** 69.088s、sealed **48/48** 63.826s、均 0 skipped；`TS/target/s4-typescript-014-{source,sealed}-final.log`，日志摘要分别 `fbfa1dd56effd2b78bbcca7383defd9e88e45b48d6bb58c6ea45f9c8e8ce79ba` / `d56f9f493e14a8c7bcad4e1099159e3dc50538b0a9208b1d5eaea70a97f8d1b7` | 包含内层致命退出传播与 >260 字符规范贡献入口的真实 stdio 语义。Vue `.11` 的 source/sealed **102/102** 继续沿 E09 原时点及模块身份，不能将两加载模式相加或声称整套 Vue 已用每个后续 TS 版本重跑 |
| E23 | 项目引用/paths/合法 workspace junction 定向 source/sealed 各 **1/1 PASS**，约 1s；`TS/target/s2-project-references-{source,sealed}-first.log`，后续 45 项和 E22 的 48 项完整套均包含相关回归 | 真正 TS-owned 引擎、脏原源 URI/类型/诊断/关联图及磁盘不变；不是 Vue 外部 src 全部路径形态的宿主实际导航 |
| E24 | `shared_typescript_choice_is_native_scoped_and_retains_navigation_provenance` **1/1 PASS 70.95s**，FE5919 / TS `.12` `f869…c3a5`；`target/s2-shared-context-f12-green-012.log`，摘要 `e213f503418ccfca29d0d3226031840088b8e30ac8f8fd7e5ea69be4dc1f4016` | 原生 QuickPick 取消/选择、A 撤销而 B 保持、非空私有偏好、关闭重开、真正 F12 来源继承与具体 hover、磁盘/清理全过；旧三次失败保留。最终 `.14` 组合身份仍需收敛，API>=99 负例不冒充领域 major 更新 |
| E25 | 未知语言选中补全 Windows parent/child **19.11/5.51s PASS**，`target/s2-unknown-completion-resolve-first.log`；复用 harness 的 range IME **7.52/1.57s PASS**，`target/s3-native-inflight-range-reused-harness.log`；C535 冻结程序 | 普通键入/选中后 resolve，主编辑+import 单次 Undo；源 ABA、辅助脏源、非法尾部、owner 停用全拒绝。range 保留实际 pending wire、350ms 延迟、原生 IME bridge、same Arc/init1、磁盘及 HWND 退出。不是 Vue 历史八行或 OS 候选窗证明 |
| E26 | 四项 Vue 格式化 Windows parents **4/4 PASS**：matrix 101.79/101.63s、boundaries 93.09/92.95s、provider 70.28/70.13s、ABA 93.14/92.99s；8A5B / TS `.12` / Vue `.11`；`target/s3-native-formatting-{matrix,boundaries,provider,aba}-first.log` | 三 Vue×四样式的 12 行 whole/range、块/属性/src、CRLF/Unicode、single Undo、无保存；安装 Vue 保持 TS primary Arc/grammar/formatter；四种 same-turn input/IME ABA。各项 checked actor/process/HWND 清理均过。完整日志摘要在 S3/S4 记录，旧 GPUI 160.76s scheduler FAIL 不改写 |
| E27 | grammar/queries 逐项闭合：TS `.10`→`.14` 13 项、Vue `.10`→`.11` 18 项名称/长度/摘要全同；`target/s6-grammar-resource-closure-014-011.json` `b39eadb99bcd5328d417a83f53290595a8e9eeb2b9039ceb8fbc4cf1d89ba3d2` | 可精确复用 E14 生产 loader 两项和 E15 绘制输入的资源身份；没有据此增加新的滚动、缩放或普通桌面验收 |
| E28 | 外层 `.10/.10` 恢复 **113.45/113.27s PASS**：`target/s5-native-vue-alias-recovery-first.log`；仅内层 adapter 退出 P3 真实 Windows **122.54/122.38s PASS**：`target/s5-native-inner-adapter-014-second.log`，摘要 `2cc00d33ee6390aa2c21cc437d6dff7cb92cbae285a9ef4c0f03a3ba162e6bed`，宿主 `0b8e4ac4…e8c39` | 内层仅结束 A 已准入 compiler，外层自然 EOF、有限恢复、脏具体类型、旧 capture 失效、B/C 原 Arc/capture 保持、撤信任/进程0/ACK/HWND/磁盘全过。原 Node RED 6.61s及旧 setup-only 超时分别保留；不外推更新候选回滚 |
| E29 | 实际状态本地化 **RED 0.11s→PASS 0.28s**：`target/s5-{native-recovery-locale-red,recovery-locale-green}.log`；queued 安装发布 **RED→1/1 PASS 0.17s**：`target/s5-queued-install-locale-{red,green}.log` | 恢复失败/重试上限/owner 项目、EN/ZH queued 与后台失败发布及十阶段 renderer 取消边界有实证；renderer 检查不是十种实际下载操作或普通桌面可见检查 |
| E30 | managed 普通 **8/8 PASS** 2.82s（另 1 ignored）、optional **3/3 PASS** 0.13s；`target/s5-{managed,optional}-ordinary-final.log`；收据 ignored **1/1 PASS 231.52s**：`target/s5-managed-receipts-preflight-final.log`，P5 | 实际私有准备、缓存/离线复用、网络/摘要/取消失败、并发等待取消、跨 workspace 租约、显式路径与无需下载分支。普通 native-modules 2 和 services 1 另过，普通合计 14 PASS/13 ignored，ignored 不计通过 |
| E31 | 泛型 hot update **1/1 PASS 251.34s**、fault **1/1 PASS 188.40s**、data migration **4/4 PASS 847.05s**；`target/s5-{hot-update-preflight-and-rollback-green,fault-recovery-preflight-final,data-migration-preflight-final}.log`，P5 | 真实 Package/Manager 必要依赖预检、激活失败恢复旧数据/owner、后续 materialized 不重建、独立故障和中断恢复；data crash child 由父真正执行，1 filtered 不单独算通过。不是 P3 的反向 TS/Vue 更新窗口通过 |
| E32 | native_modules ignored **3/3 PASS 402.97s**：`target/s5-native-modules-sdk4af-preflight-final.log`；service sets 旧批 **2 PASS/2 FAIL 595.69s**，两失败按同 30s 预算单独 **PASS 144.28s / 221.64s**：`target/s5-service-sets-{borrowed,open}-observation.log`，P5 | A/B 原始 caller、两端退休、last pending peer、实际 stdio 握手/EOF和集合变更各有证据。service sets 四个 unique 用例各有 PASS，不称为一次整组稳定通过；其 guest 无 Snapshot、末尾 unchecked，不冒充完整 checked checkpoint |
| E33 | plugin_services 九个 unique ignored **9/9 PASS**：deferred 3/3 387.30s、lifetime 3/3 386.95s、routing 3/3 465.65s；`target/s5-plugin-services-workspace-{deferred,lifetime,routing}.log`，exe `22f79bbf…848b4` | 真实所需服务/缺失兼容/选择、原权限、异步取消/超时/退出/多跳及同 scope 偏好。原空 workspace 批次 2 PASS/7 FAIL 499.15s 保留；只修真实兄弟 workspace 输入，未改信任。各组 7 filtered 不相加为通过 |
| E34 | SDK4af context registry **1/1 PASS 92.15s**：`target/s5-context-selection-sdk4af-final.log`，ZIP `cf01bfc49fffedb36bc7bd57ed93783bf92e3234c4d2e2ce137e1cb0abfd54a0`；生态 Snapshot lifecycle **1/1 PASS 53.76s**：`target/s5-ecosystem-lifecycle-sdk4af-final.log`，P4 types `.2` | 当前公开注册/引用及 checkpoint→disable/enable→checked shutdown 有证据；后者不是四角色 compiler 语义或 mapped 编辑事务通过 |
| E35 | 取消委托 **RED 66.17s→1/1 PASS 68.78s**，`target/s2-deferred-native-isolation-final.log`；准入另 **2/2 PASS**；公开 delegate P5，程序 `a8f5a3bf…54659` | 新需求不继承已全取消的 Pending；第二真实 SDK 控制请求/绑定/同 Arc/旧请求 Cancelled/停用/checked shutdown。没有启动原生进程，process0 不当成物理 EOF 或 Vue 语义，详见委托取消记录 |
| E36 | mandatory 反向 provider 真 **RED 58.81s→GREEN**；完整边界 **1/1 PASS 54.67s**：`target/s5-reverse-provider-lib-boundaries-green-retry.log`，摘要 `6827daab2a86cfe837c93fd848517f416c97e767240eaa2da6521176a1ed37f4`，exe `234a3d8f…32f87`；此前 ordinary lib **104 PASS/2 ignored 13.56s** | 真实已解析 mandatory 拒绝、公开改选/optional、boolean gate/平台有效发布、原 owner/ref 及 checked shutdown。另 resource-only 同 ID 漏口真 **RED 27.90s→1/1 PASS 28.06s**：`target/s5-resource-only-preflight-lib-{red-valid-quota,green}.log`，摘要 `9b30c1695ad0c087b635b5090d75fdbdbd00af3e54edf1561c659bf12addeea8` / `74e67ab0b874dca2e759826f359b5cbe300abfc15323b89c4e83e94945202733`。编译成功不计行为；该最新修复尚待独立增量复核和新 app 联验 |
| E37 | 真实 P3 provider-major 更新 **RED 147.52s**：`target/s5-native-provider-major-staged-red.log`，`7f594f3f…6651`；Vue requires `typescript.adapter ^99` 候选旧 **FAIL 170.01s**：`target/s5-native-vue-candidates-and-recovery-012-second.log` | 前者确实提交 TS 2.x offered contract并破坏三 Vue Ready；区别于旧 225.15/224.93s setup-only 失败。E36 已补通用预检，但保持三原服务、epochs、类型、非空偏好的真实 forward/reverse Windows GREEN 尚待；不由 lib 或 API>=99 负例外推 |
| E38 | 历史八行先后 **FAIL 89.61s / 126.30s / 90.22s / 87.27s**，`target/s3-historical-native-{first,013-second,014-third,014-fourth}.log` | `.13` 旧轮完整过前三行，第4行失败；最新已跑候选整项/八行尚未过。输入排队、paint-only 误用与上一帧焦点观察分别修正，唯一回执 helper `d7a8…e776f4` 已与 external 共享；H3 中完整历史/external 待跑。1.57s sealed Session 共享图探针仅证明三形式提案完整，不代替宿主应用/Undo |
| E39 | S4 四角色第七次 **FAIL 156.48s**：`target/s4-four-roles-native-014-seventh.log`，摘要 `198a7e04f1f55d6113a4bbd5bba4d4fbb55d7ad04547c7326d561b1f21af5e16`，H3/P3/P4；四冲突首轮 **FAIL 157.11s**：`target/s4-conflicts-native-014-first.log`，摘要 `66ae11122fb9cdf207a28b7836cd0868c50d3e6c64c36b5c4bafc90492bf0f77` | 四角色诊断精确 marker、首个 mapped definition/refs 已走过；`count` 只有单文件编辑，按既有直接 rename 契约未显示跨文件 preview，测试改真实跨文件 fixture 待复验。冲突仅已见 exclusive 明确拒绝及 imports released，恢复阶段仍读到 Failed 而整项失败；其余三冲突及旧 confirm 更新/撤销链不计通过。早期越界、缺 capability、规范 Node 路径、literal42和第六轮失败均在 S3/S4 记录保留 |
| E40 | 当前 H3 坏显式 SDK exact 组 **1/1 PASS 251.19s**：`target/s6-typescript-explicit-sdk-final-014.log`，摘要 `e1f74490026434403a95bb287a30daa394084c7f2fe2e5c6d92835919f85b7b2`，P3 TS +独立 public consumer；旧三项批次 **2 PASS/1 FAIL 176.09s**：`target/s6-typescript-public-negative-and-native-014.log` | missing/wrong/incompatible 三 SDK 都 phase0结构化拒绝，允许尚未发布 plan 的早期失败；若有 plan 仍要求唯一正确 owner/exact SDK，不回退 bundled。旧批 public resolve/describe/脏分析/release 和缺 TS 两项已过。此为公开 consumer/SDK 行为，不把 Rust策略或 fake compiler 当真实 Vue 语义 |

当前 `cargo fmt --check` 既有 PASS 为 `target/s6-workspace-fmt-green.log`；后续生产增量仍须在最终冻结后执行完整门禁。双轴审查初轮 Standards 0、Spec 内层退出 P1 已有 E28 实证及增量复核；两项 helper P2 已独立复核源修复并有 Node 2 项 PASS。最新 resource-only P1 的 E36 GREEN 不能代替该改动的独立复审，亦不能代替 E37 真 TS/Vue 更新联验。

### 04:18 新增终态与最后候选（E41–E49）

本节直接读回实际日志及[最新 S3/S4 记录](vue-typescript-s3-s4-native-acceptance-2026-10-20.md)、[03:32–04:11 增量审查](vue-typescript-remaining-review-2026-10-20.md)。P3/P4 的实际包仍为 TS `.14` / Vue `.11` 与生态 `.2/.3`，没有把元数据已准备的 TS `.15` / Vue `.12` 写成已打包或已验收。

| 代号 | 真实结果与身份 | 证明边界 |
| --- | --- | --- |
| E41 | 历史第六轮 **1/1 PASS 144.24s，8/8 行完整**；`target/s3-historical-native-014-sixth.log`，SHA `d71c0462464778ba1f77328b2557d89bb8c518183d01e656caa6f4ec67ced35f`；宿主 `target/s6-receipt-occlusion-and-preflight/deps/editor_app.exe` `703c8ba44d5b6927ef8b5a748e734ffe8b8f99d5d7ced2a2f79bc5a516d4c04a`，P3 | class×2、JSX×3、TSX×3 每行均走脏原源、未打开 parent、完整预览/取消/应用、真实回执 group Undo、排队输入、选中补全 resolve+import、single Undo；固定磁盘/配置不变与 checked shutdown/plan inactive/process0 均过。实际滚轮解决深层回执可点击性；不清旧记录、不内部 Undo。不是任意历史依赖或 OS 桌面全覆盖 |
| E42 | external-script 第二轮 **1/1 PASS 104.93s**；`target/s3-external-native-014-second.log`，SHA `b860ac2d3f52cc033d1ce56339d1f7d407d4f9e66b871a297fe867f2fb1c5b01`；同 703C / P3 | Vue2.6/TS、Vue3/JS 两种已声明合法关联源，精确脏源诊断、原始 F12/refs、跨文件预览/取消/完整应用及真实 group Undo 均过；主 TS 保持、磁盘/元数据不变、checked shutdown及全部捕获计划退休。不能泛称所有动态导出或链接布局已验 |
| E43 | 四角色第八轮 **1/1 PASS 152.95s**；`target/s4-four-roles-native-014-eighth.log`，SHA `c9b7a4361b6bee0c9f19cb787cce02191b3a7831c60f5ee20638665835db1a42`；同 703C / P3 / P4 | 四角色精确诊断；count/shade 的 F12 到 `state.ts`、三个原文 refs、两源完整预览/取消/应用/回执 group Undo。携带 style preview 更新 `.3` 时旧 Arc/进程/诊断/预览撤销，reduced template 整链继续；携带 template preview 卸载贡献包后旧映射撤销，实际 Enter 不产生 `retired_` 编辑，普通 script hover恢复。双源/磁盘/config/checked shutdown均过。此项已证明对应 S2+S4 及扩展更新/移除联验，不能继续列为未执行 |
| E44 | 四冲突第三轮 **FAIL 162.39s**；`target/s4-conflicts-native-014-third.log`，SHA `28e481da6c4d0973842f882584151e2ff48946ca995a580416ba4ea463ef8943`；宿主 `064129a3f58120c13b543fdc6059a1affe95198103dd051155f558acbb24fa48`，P3/P4 | exclusive 拒绝及 imports released正确；移除发布后，catalog/native generation 已同步、pending_sync=false，仍缓存旧 Failed，已排除少等一帧。其余三拒绝/恢复及结尾清理未执行通过。通用失败缓存与原 scope 传递 collection 观察正在修复，不能把中间源码或静态复核记成主用例 GREEN |
| E45 | 最终 outer 入口最近 **FAIL parent 226.46s / child 226.26s**；`target/s5-native-outer-final-014-011.log`，SHA `e22ca1f12a4535bd93808ba06e9592cbb8d7572b9d0f6ca9f97b60df1c2fb827`，P3 | 停在首个 cold source：`Vue dependency call cancelled`，尚未建立第一份具体语义；checked cleanup为Ok。未触达候选更新或崩溃，不能把它当相应产品目标 RED，也不能覆盖 E28 内层恢复 PASS。须在最后修正后 solo 完整重跑；E37 的真实 forward/reverse目标 RED仍待当前实现的原生 GREEN |
| E46 | 资源候选 P1 已独立复核解决：E36 的 **27.90s RED→28.06s GREEN**；相同冻结 runtime 反向边界 **1/1 PASS 57.98s**、ordinary **104 PASS/3 ignored 13.58s**；程序 `target/s5-resource-only-preflight-final/deps/plugin_runtime.exe` `788049cd007660c25668143b106b17e60e0ebb2ae2c300aea82365a14a925628` | WASM/纯资源共用只读有效契约反查，Actor 在撤销 epochs 前预检、提交再检查。两独立轴源复核无新增发现；不再把已解决 P1 列为源码缺口，E45 真实 TS/Vue最终窗口仍待 |
| E47 | 失败重试首个目标 **RED 1.15s（1 PASS/1 FAIL）**：`target/s6-admission-retry-red-aligned-profile.log` `ae5355e20039cd4387f549eaf088c5a236b4cc3bbc828e261780d0f492ff9863`。已接受后取消的新目标 **RED 1.50s（2 PASS/1 FAIL）**：`target/s6-cancelled-demand-explicit-red.log` `81f81a3316bbf58a58d1e34ed5f92c802305d89a3b3bc1484290cf412cb3bab4`；exe `5ff367a1027924d85dddb3e1e7c4558808f076dd80bc5becd8511dab025f5c61`，测试源 `c86a134ccf333a3538df4f08ceee3605ba2d4c10a625c302864e83c94edf9527` | 新断言明确为 `cancelled admitted preparation was requeued without a published error`；不是仅凭文件名叫RED。此前1.48s停在Failed状态断言的失败也保留。当前准入拒绝不重发和晚到 preparedErr 正向两项已过；仍须区分 Completed preparedErr 与 Cancelled/TimedOut，修后3项全过及E44实际冲突恢复。没有扩大语言请求期限或允许盲重试 |
| E48 | 最终元数据已准备：TS插件 `.15` 保留 native `.14`，Vue插件 `.12` 保留 adapter `.11`；`target/s6-final-plugin-metadata-prepared.json` SHA `13933fedc477cc54260b57c75f1bc1acd00fa9baced4d168ec1d3976ba78cf64` | 独立双轴已比对24源项、47资源及11份Vue封印文件；协议/能力除已确认版本归属外保持，support/README仍pending/candidate。正常 CLI已分别guest check，当前SDK仍4af；**尚未最终pack，不存在可冒称已通过的新ZIP**。须包/源码/许可/锁/摘要闭合及最小pair smoke，按相同封印字节复用旧语义证据，无须因纯元数据重打而另增历史组合 |
| E49 | 上一轮wide门禁曾全过：fmt、非UI **319 PASS/327 ignored/0 FAIL**、workspace check；正常宿主 `target/s6-final-native-host/editor-app.exe` `ff92755696cb68a3b00aac8111077b90908f0b50019beaeb6bdf56b8545c2776` 导出4af且两guest check通过 | 03:49失败缓存首版及04:11取消终态问题之后，53367/89804两次构建被根代理按身份停止，部分输出不是最后全过。最后修复必须重新门禁和增量双轴；普通持久Windows桌面尚未验收。本轮没有提交/推送/关闭新issue回执 |

### 04:42 发布证据与最终 S5 全链（E50–E52）

以下日志摘要和冻结程序摘要均已在本轮直接读回；执行结果来自根代理，盘点者没有重跑。TS `.14` / Vue `.11` 的 P3 身份保持，最终 `.15` / `.12` 仍未打包。

| 代号 | 真实结果与身份 | 证明边界 |
| --- | --- | --- |
| E50 | 发布 provenance 三回归 **3/3 PASS 1.66s，0 ignored**；`target/s6-publication-provenance-green.log` SHA `f2c998a7f295deab462ba34d284f14e24cb708faad8e7b37190f8da7939a3fb2`。no-run **PASS 3m28s** 后冻结 `target/s6-publication-provenance-green/deps/editor_app.exe`，SHA `918351a79c4cbf0055d283b2c68c717747df49925feeef6afb95d74bfcead5a1` | 公开 `language_failures` 三项实际走完：异步未准入不逐帧重发、已准入取消但未发布错误不重发、真实 prepared error 在先观察/先撤回两种顺序仍只重入一次。两项 E47 真实 RED 已转 GREEN；原 reservation 发布证据及失败输入的独立 Spec/Standards 增量复核均0项新问题。完整集合冲突仍不能由这三项小回归推定通过 |
| E51 | 最终 S5 outer 及候选全链 **1/1 PASS parent 164.27s / child 164.09s**；`target/s5-native-outer-normal-frame-solo.log` SHA `e198220e411d865aac2e1388e90714cade3a5d69e481f8715a9de507edd954a6`。程序 `target/s6-native-normal-frame/deps/editor_app.exe` SHA `706332eba442bf2ccb266fbe23b33b7f49c84d2cb43be487a25e0c2ebcabb399`，P3；no-run **PASS 1m53s** | 三 Vue 具体补全/诊断、Vue Preparation/Contract 两类正向候选拒绝、反向 TS provider 候选拒绝均走完并保持旧包/原 epochs/三分析/私有记录；真实 outer EOF、有界恢复、脏类型变化、旧结果拒绝、B/C隔离、撤信任、checked shutdown/HWND退出全过。日志明确选择偏好记录为0，非空 scoped 选择偏好由 E24 单独证明，不冒称本轮含非空偏好更新联合样例。唯一测试帧 hunk 从强制同步改正常 render，源码 `4ea09524a20e0730a85bd20d6411ad1175d73d8cb3ebd8d51315395086ac0e2d`；两轴逆转后精确恢复原 `9b63a5312ccfbe759b1e4db23f74d9179f67cd13667c49f86229676fa835487a`，0项新问题。生产、期限、断言不变；首20s grammar记录约1778降至10，不将改善冒称延长预算或 S4 已过 |
| E52 | 前置失败继续保留：918程序的完整 S4 **FAIL 127.22s**，`target/s4-conflicts-native-provenance-green.log` SHA `bd07692442dfaf3350001084df0fe8a4f46e915f80c28157603e49ed69d86e7b`；同轮 outer **FAIL parent 222.60s / child 222.46s**，`target/s5-native-outer-provenance-solo.log` SHA `61bd813a227dcc321d30ed9b3ce33e329875a29121396782795275cd092f99e7` | S4 停在初始四角色准备，未安装任何冲突候选、未触达拒绝/移除/恢复，不是失败缓存修复的目标 RED/GREEN，也不冒称 checked cleanup。outer 同样未建立初始语义或注入故障，cleanup为Ok；后续由 E51 完整通过，原失败不删除。当前根代理在706程序独占重跑完整 S4，本文不提前记录终态、不新增其它重任务 |

### 恢复后新增实际证据（E53–E59）

本节追加05:08暂停保存及恢复后截至E59的实际终态，E01–E52证据行、旧FAIL、原程序和包身份保持不变。结果来自根代理实际执行及已落盘日志；维护者只读核对、不重跑Cargo/Node/窗口。截至E59仅修改 `#[cfg(test)]` Worker观察／原生authority交接；之后新增hover生产修复另列E60–E64，SDK与插件闭包未改。

| 代号 | 真实结果与身份 | 证明边界 |
| --- | --- | --- |
| E53 | 完整 S4 四冲突 **1/1 PASS 280.28s**；`target/s4-conflicts-native-after-native-cleanup.log` SHA `670ebf654e48a2ebf410a2f3e71d80fb423975b91d1cf0b55d36301cc96c61eb`；706 程序、P3/P4，与 E51 同宿主生产源 | exclusive、顺序环、配置冲突、不可逆必要映射均实际拒绝，逐次卸载后恢复原生诊断及 F12，原源／磁盘不变并 checked shutdown。解除 B1；E44 目标 RED 与 E52 前置失败保留，不以静态或三项小回归替代本整项结果 |
| E54 | 最终生产门禁已实际通过：fmt；非 UI workspace **85 成功 suite、319 PASS / 0 FAIL / 327 ignored**，`target/s6-final-delivery-non-ui.log`；workspace check **PASS 8.77s**，`target/s6-final-delivery-check.log`；正常宿主构建 **PASS 2m19s**，`target/s6-final-delivery-native-host/editor-app.exe` SHA `996bf4fdbb8fcf79a4949c6ae7fe3035c1cfe1b53e3de6b1d48badf52f498448` | 覆盖最终失败缓存／provenance 生产实现，B5 的“最后 wide 待跑”已解除。暂停后仅改 cfg(test) 接缝，恢复期 fmt 记录 `target/s6-resume-after-handoff-fmt.log`；不将327项 ignored 当通过，相关 ignored 由各分项实际结果关联。若再改生产，按影响补门禁，不把旧身份套给新行为 |
| E55 | Worker 交接修正版：普通组 **6 PASS / 0 FAIL / 8 ignored、1.63s**；原撤权 ignored **1/1 PASS 67.94s**；正向更新 ignored **1/1 PASS 67.84s**。源码 `worker_tests.rs` SHA `46d4d155ae45d3396ff95ae5013de727710699eb1110f602e73700d37c605577`；冻结程序 `target/s6-resume-worker-handoff/deps/editor_app.exe` SHA `d69c12703872458ea38f971c19011d5b78033eeadaee617a2087d80d17e3bc94`；日志 `target/s6-resume-worker-handoff-{ordinary,revocation,live-update}.log` | 旧 FAIL 129.11s 与新增观测 FAIL127.43s 保留。后者明确卡最后 scope-read 屏障，views复活；旧测试漏同步共享 authority，runner仍读启动true。只在发送 true/false 通知前按实际UI store(Release)，未改生产；原 `.17.1` 包、60s期限、撤权、旧grant、旧版本、正常更新及 checked shutdown 均完整走完。两轴增量0项发现；不把原失配当生产安全缺陷，不把此例替代S4/S5真实语义 |
| E56 | normal996 公开 `--export-plugin-sdk` 实际 exit0，**54 文件 / 0 Markdown**，全部字节等于 SDK `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`；`target/s6-resume-sdk-export-proof.json` SHA `480bf3a045f738442d07d98f695b5c3ddefccbc9ca4cee135613c261e55c1a43` | 当前正常宿主的公开 SDK 身份已实际验证，不再援引旧 normal 的 check 作为唯一证明。没有拷贝SDK进入插件源码；SDK与工具版本分开记录，不构成桌面或正式支持验收 |
| E57 | 同一 normal996 公开 CLI 串行、离线、独立 managed target 实际构建桌面候选：`TS/target/s6-desktop-candidate-015-package/typescript-0.1.15.zip` SHA `422074c65d63d89b2f8b060feb15ceaaffaf4b7a3c77b19f6c6c665ebdb39f43`；`Vue/target/s6-desktop-candidate-012-package/vue-0.1.12.zip` SHA `058cb1eec9dbef169aba8095c2e94c0f29935b845735d1c7488108e8a02e4a5e`；release WASM **1m07s / 1m08s**，两入口 exit0 | 两快照 workspace_root／target_directory 构建前已核，key `3914ed5b766817c2` / `5192bbbb352d98ed`。日志 `target/s6-resume-{typescript,vue}-candidate-package.log`；这是 pending 支持的桌面候选，尚非支持提升后的最终包。原 `.14` / `.11` ZIP不覆盖 |
| E58 | 独立 ZIP／SDK／快照预检 **PASS**：22+39 entries精确闭合，保留资源 **47/47**、Vue模块封印 **11/11**、源码 **73/73** 全等，SDK摘要仍4af；`target/s6-resume-desktop-candidate-packages.{json,md}` | WASM对应各自实际 managed Cargo产物；无新增／丢失／碰撞／危险路径，manifest相较旧包仅插件版本变化，能力／权限不扩。TS native仍 `.14` / `71c279…84f83`，Vue adapter仍 `.11`，许可和grammar负载不变。静态核对不冒称S1、普通桌面或正式支持通过 |
| E59 | 桌面候选双包 mixed S1 **1/1 PASS 95.97s，0 ignored，session60945 exit0**；`target/s6-resume-candidate-pair-native.log`；程序D69C、E57两包、SDK4af。成功保留 `target/s6-native-ui/profiles/s1-verified-Im6Nju`，checked shutdown通过 | 真实 Package/Manager 与混合Vue2.6/2.7/3原生脏源分析走完，可供 normal996 后续打开同一私有profile。不是普通持久桌面可见输入、OS候选窗或正式支持提升后的最终包复验；support仍pending，不提前关闭AC或工单 |

### 暖缓存悬浮生产增量（E60–E64）

该增量由普通持久桌面的真实崩溃触发。E59原S1没有暖缓存键盘断言，保留原PASS而不冒称已覆盖此缺陷；修后自动回归和普通桌面复验分别记证。

| 代号 | 真实结果与身份 | 证明边界 |
| --- | --- | --- |
| E60 | 暖缓存Ctrl+I真实 **RED116.40s→1 PASS / 0 FAIL / 0 ignored，97.62s GREEN**；`target/s6-cached-hover-{red,green}.log` SHA分别 `d0ac43dc74bbcbb73c321f3799e3108bc24e644a6ba2a006b55c8de246a618b2` / `280b89c28b559b52c880bfc8c62e512618877b37e5672be0609687f57ceac868`。修后程序 `target/s6-cached-hover-green/deps/editor_app.exe` SHA `b41758b2bf392457adbe74f5ce52cbbb3d7d78d569afe203cedb58402dc24454`；身份账本 `target/s6-cached-hover-green-inputs.json` SHA `964883e0db30bbc5f9487f3ca50a23f358a771a29abea060dacf72145e2307d6` | 普通桌面栈 `target/s6-desktop-hover-red-stderr.log`确认正缓存关联检查在父EditorApp更新内重入读取。生产hover SHA `999c226e894732e868fc867a0b233d408637029869a0775df33cbf90444db790` 将缓存选择及ContextCapture检查延后到返回任务，保留共享worker/失效重取/完成校验。回归 SHA `1789ed42b793260789e79fa198ce2ece8665d61224f295fd0d97dbd2208503cf` 经三Vue真实owner-bound provider暖正缓存、Escape只清展示，再实际Ctrl+I显示卡片；原mixed语义、磁盘和checked shutdown断言继续执行。新成功profile `target/s6-native-ui/profiles/s1-verified-x2ZMVu`保留。双轴匹配当前源码0项发现；不代表修后普通桌面、真实OS IME或共享源刷新回归已通过 |
| E61 | 同一修后程序普通hover过滤组 **12 PASS / 0 FAIL / 0 ignored，2.75s**；`target/s6-cached-hover-ordinary.log` SHA `5c0690a67b4f9b5dec90f237679cf9467fa05aea3abdbbc979b30d365981a82f` | 过滤组实际包含诊断Unicode/滚动、hover命中/遮挡/resize、键盘关闭/编辑、卡片、符号字节定位及其它名称匹配项，不称为12个独立Vue缓存场景。共享源刷新另见E62/E63，不从这12项外推修后普通桌面PASS；E54旧冻结点wide仍保留其原身份 |
| E62 | 共享源刷新首轮 **0 PASS / 1 FAIL / 0 ignored，35.65s**；`target/s6-cached-hover-related-refresh.log` SHA `58e40b193f7fdb072ff5bc5e6b1b9d8ef1f167490ba534eff63d72fd9f475e8a`，同b417程序 | 实际误选context-selection-fixture `cf01bfc4…54a0`，`panel.opened`在hover断言前报InvalidRequest/null，未进入缓存刷新行为。保留为前置输入失败，不归因stale cache、不用改生产或回归绕过 |
| E63 | 正确project-context输入的共享源关闭／同路径重开回归 **1 PASS / 0 FAIL / 0 ignored，26.25s**；`target/s6-cached-hover-related-refresh-corrected.log` SHA `633af425500066e67b6305ee575eebf3dc261c3cb4c37df4b7fbcd53a2901815`；`target/s6-cached-hover-refresh-inputs.json` SHA `d66ab47d76df1a0b82dac1fcb846edd762ec6ce778099a32c297b252d1d01c94`；根代理确认串行session95682最终exit0 | 同b417/hover999c，原测试未改；正确包 `target/s1-resume-2026-10-19/fixtures/project-context-fixture-0.1.1.zip` SHA `08cc6b165dd1f582f8ab586909c29f9ce995f740393d0e71714caaed1f44ebdd`。真实公开Package/原生hover/两条stdio后端观察丢弃脏源后回到磁盘、重开后最新draft、同一UI轮关闭重开及强制fresh查询；各项目init1、didClose序列与磁盘不变均断言。此为通用未知语言fixture，不冒称Vue compiler或checked shutdown（原例为manager.shutdown） |
| E64 | 修后普通宿主构建 **PASS58.30s**，同session95682最终exit0；`target/s6-cached-hover-normal-build.log`；程序 `target/s6-cached-hover-native-host/editor-app.exe` SHA `37fd31356ca8445b73326cb0fb8d841c9e7e8af8889cecfd87ee148f8c518aad`，`target/s6-cached-hover-native-inputs.json` SHA `b73b2944e7380b31a36442e97697774469a66f4151a36dc4fc201b1751b12fbb` | PDB `e96d6ee990f6822dd7e18dc9df65c57036de60f0d125832817d3775cf061d50c`与hover999c明确绑定。构建通过不等于普通桌面复验；新三项仓库门禁session4456仍在串行，本文不提前记终态。SDK/插件闭包未因hover修复变更 |

### 普通桌面与配色增量（E65–E70）

普通观察详见[执行中的桌面记录](vue-typescript-desktop-2026-10-20.md)。E01–E64原行保持；以下区分37fd已观察行为、颜色定向测试及尚待普通复验，不将静态审查或插件版本页头冒充引擎/后端证据。

| 代号 | 真实结果与身份 | 证明边界 |
| --- | --- | --- |
| E65 | 37fd三门禁session4456实际exit0：fmt、非UI **85 suites / 319 PASS / 0 FAIL / 327 ignored**、check **3.52s**；`target/s6-cached-hover-final-{fmt,non-ui,check}.log`。同normal公开SDK **54文件/0Markdown/0差异**，仍4af；`target/s6-cached-hover-sdk-proof.json` SHA `e2ef1f46ded4b58a927ca633373b876fa16ece66b32c4449f8ba6f1025875ed7` | 解除E64当时“三门禁正在执行”，不改原行；327 ignored不算通过。完整normal SHA37fd见E64；这些是配色修复前快照，不能自动当最新颜色快照门禁 |
| E66 | 普通Windows PID60348、normal37fd、既有Im6Nju profile/matching trust原Vue根及E57候选：重复暖正缓存Ctrl+I与鼠标Child hover、模板/组件F12原源、ShiftF12原路径列表；F2两处一次本地Undo；跨App/Child完整预览/取消/2项应用，以及独立第三次`groupTitle`计划后直接“撤销整组操作”恢复2文件；`message.toU`显示TypeScript2339，真实列表Enter选toUpperCase后写回/错误清除；ShiftAltF规范SFC脚本/CSS，script/template/CSS可见高亮 | 均来自普通持久窗口实际观察，不以TestAppContext代替。各文件Undo与独立组Undo分开，预览/未保存原文不冒充磁盘全量保护核对。深浅主题曾操作，但37fd深色hover白底浅字实际失败，配色另见E70；一次Vue3桌面不外推所有版本/历史组合 |
| E67 | 同37fd真实OS中文IME：逐键n/i显示候选“你”，空格提交仅一次、列9；第二次组合Escape取消无拼音残留；一次CtrlZ恢复提交前、列8。实际滚动顶行1→16且内容吻合，设置字号14→15px后模板F12仍准确选中原第3行声明 | OS候选窗/组合提交取消有实际观察，不再仅凭E25/E26 bridge。字号变化不冒称系统DPI矩阵，相关选择/滚动/命中观察保留；其它编码/大图/IME异步边界沿E04/E25/E26，不重复手动全集 |
| E68 | 同37fd普通TS `acceptance-desktop/a/use.ts`逐键输入`desktopTota`，实际候选desktopTotal；Enter resolve后正文完成并新增`import { desktopTotal } from "./math";`，一次CtrlZ同时删import并恢复前缀。独立6文件输入账本 `target/s6-desktop-visible-fixture.json` SHA `9c5f4d0c0988512f55b1c150760122e059c38b58cfa1895bb88ebbb792a82bf1` | 不把异步空帧或原Vue3中未出现nextTick候选记为通过。此处是本地TS模块普通入口；SFC自动导入依据仍为E41八行真实Package/native选择/resolve/import/Undo，未用TS一例替代SFC实证。临时输入清理与原工作区保护另记 |
| E69 | 同37fd普通共享源chooser实际列出A/B，重新发起后立即点击B；偏好revision1、source=`acceptance-desktop/shared.ts`、project=`typescript:9baffd8e30f9150412d4e72bd34d0d3127bd7f31e8e1893480b3789deff3245b`。独立收据 `target/s6-desktop-context-choice-proof.json` SHA `cd05de058b287d10c286ddd6a8a5f25d995153bfe8a2b8192ac53c02193f7bbd`；关闭重开无再次询问；原生“插件管理→TypeScript→运行日志”显示B根、analysis/engine、准备完成及进程启动 | 已部署projects.mjs与源码SHA相同，按Windows小写/斜线规范化B配置路径重算key精确匹配且不同于A。前两次窗口消失无偏好，不计成功或擅判超时。页头0.1.15是插件包版本，未冒称TS引擎/后端版本；完整引擎、错误/恢复仍沿E22/E29/E51专项 |
| E70 | 配色真实 **1 FAIL/0 PASS，0.18s RED→6 PASS/0 FAIL/0 ignored，1.33s GREEN**（含新绘制回归）；`target/s6-hover-colors-{red,green}.log` SHA `5bcd91c767301d97eebdf44ad648edd9163de93d263359e9350f6a066bafee52` / `c81cbd366f986dadb3e5ffc27a531f81a7bb85c9050cd1bdbf690a96bceec1a2`；另`target/s6-hover-colors-hover.log`的language::hover **1/0/0 PASS**。程序 `target/s6-hover-colors-green/deps/editor_app.exe` SHA `7ce52d3d64367884603010753a1fb5c9e649033ffeb5ac9f5e36554f3e4c6252`；`target/s6-hover-colors-green-inputs.json` SHA `61a6dba49bb04fc07b7b5e406a241939e322e1db2396dbdc417801175150940c` | 原dark prose contrast1.31失败保留；生产popovers SHA `eba111999e032bf45ce5cfe655b5a568df7d5d4b2d4913921bda309c39dd44de`，测试SHA9512b51d…e57fbd。3卡读取live主题、去固定白底；内置暗/浅×prose/inline/fenced实际卡片quad回归，Spec/Standards增量各0项可行动发现（`target/s6-hover-colors-{spec,standards}-review.md`）。最新颜色normal/三门禁session26427仍待完整终态；首轮fmt有机械tuple换行差异，不预记全PASS。修后普通深浅配色尚待，不据6项测试勾B4或promote |

### 最终配色、键盘及功能支持冻结增量（E71–E77）

E01–E70原证据行、失败与当时待验不改写。以下新增终态解除其过期阻塞；原候选包、不同程序、独立归档/策略交付与普通窗口退出限制仍分别记证。

| 代号 | 真实结果与身份 | 证明边界 |
| --- | --- | --- |
| E71 | inline精确全名真实 **0 PASS/1 FAIL/0 ignored，0.00s RED→2 PASS/0 FAIL/0 ignored，0.01s GREEN**；`target/s6-hover-inline-{red,green}.log` SHA `282662763fb02c1abcab28ddda4cc529de93231817b064c7d9bd5833fc470a23` / `8ef9ff634dcc09bb26fa147b0fb57e67fdda4b20cd9fae1700a181ac78121344`。GREEN程序 `43c1aee4e7032d104b4ece2a9d9a91db9d635606d3a9917d3a26769c2c1e441c`，`target/s6-hover-inline-green-inputs.json`；同程序cards **6/0/0，1.39s** | E70只观察外层卡片quad；本例明确暴露暗色inline字色None，`ui/controls/text.rs` SHA `b458c77f2fc3fd60e344e7faaca66edef0f336fdf84c9716f848bfe674c33b4c`显式使用live主题glyph/fill，document_style消费者后置覆盖保留。首次短名exact仅0 tests，不记RED；样式字段检查不冒充实际绘制，普通可见效果见E75 |
| E72 | 键盘两独立RED：同符号MouseMove清卡 **0/1/0，0.22s**，`target/s6-hover-reopen-red.log` SHA `e591a8c0be72220c5db581a1b64752054ad7222c38a52ff868946b156d52546e`；pending Escape后late reply复活 **0/1/0，0.15s**，`target/s6-hover-pending-red.log` SHA `a589def593866f8d15e62b14b074185932076f8e6963a9b01c0b3b669adb18f8`。首轮共享generation修复仅 **6 PASS/1 FAIL/0 ignored，1.94s**，`target/s6-hover-reopen-green.log` SHA `13b45da3770743d3ebe7657bd63c7481c5c0055110bd02519f7b2bfe59e0d015` | 文件名green不改写FAIL。首轮same-symbol通过、pending Escape仍失败；无卡片时Base没有原bubbling handler，随后补既有shell capture plain Escape撤销pending、重复dismiss保留suppressed。两新例走实际按键/MouseMove或held provider，未直接调handler/refocus规避问题 |
| E73 | 完整冻结程序 `beed53fdea083781c2a724e7185308430f2cbc8d44e3ae3fbb101d438ede40f1`：hover **7/0/0，1.98s**、cards **6/0/0，1.31s**、text **2/0/0，0.00s**、Escape **9 PASS/0 FAIL/1 ignored，3.53s**；`target/s6-hover-reopen-complete{,-cards,-text,-escape}.log` SHA依次 `0aa108e2313b39ad658bba6345fced01fb61c851bb50305ac1fa74513bf5daf5`、`7239f35d457ca00cfbf04b9e8d694c5f2cec3437c74ab480e6da19b4ccbff097`、`9d2fdf9bde240940ee304c1d29c6ab21af174a67bd5693282d238f7aa7f245cc`、`2812d5031e1a69d60a4a0261481cd17f70c0dad868f3a4bac2ed609bd753bb71` | `target/s6-hover-reopen-complete-inputs.json`绑定main37d720、demand038d42、pointerc7221e、测试17763b。共享generation/context/cache让显式请求清pointer抑制、返回前核原path/revision/cursor，取消后晚结果不复活；普通pointer一秒展示规则保留。各组重叠不相加，ignored不算PASS；language_features过滤0 tests不计覆盖。`target/s6-hover-reopen-{spec,standards}-review.md`独立增量各0项可行动发现 |
| E74 | 最终normal **42.94s PASS**：`target/s6-final-supported-native-host/editor-app.exe` SHA `1cae48c982197430fbbe0daf8c0664780d0ce5a1c30999c836d2f1c71ac89f61`、PDB `b448e1cf7081edb379696747059ca2caf3ff51129d5e43677479dbc5dd23c0af`；`target/s6-final-supported-native-inputs.json` SHA `2062ec1e420fc3e4be9d0cf6db4c7cf9be9058a391cee8468250114f0602f05f`。session72523三门禁实际exit0：fmt、非UI **85 suites/319 PASS/0 FAIL/327 ignored**、check **2.82s**，`target/s6-final-supported-{fmt,non-ui,check}.log`；公开SDK **54文件/0Markdown/0 byte differences**、仍`4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`，`target/s6-final-supported-sdk-proof.json` SHA `053c4ba2a43424d9a6295190e8e9a3cc2b7af9765514b8c6fbd17062c8fccdd8` | 最新host源码门禁与SDK已过，不再以session26427或旧tuple格式差异作开放阻塞；fmt日志为空但实际exit0由根代理读回，327 ignored不算通过。宿主源码冻结及公开SDK不代替支持标签重建后精确ZIP/pair/policy的B3验收 |
| E75 | 普通配色normal `a45aed2f7e8818c76ef4f0227c066fef273628192a82921f0e02d868ae75efc2`、PDB `e8e81ec396f4947190f38dde6ad904ed7c9a8f3b9b4ec38a531fd039cf7ea92c`，实际浅/深Ctrl+I均显示清晰const message、原始App.vue路径及代码片段；`target/s6-hover-inline-native-inputs.json`、`target/s6-hover-inline-desktop-launch.json`。最终1cae普通PID27012保持原可信Im6Nju profile/原Vue根：可见hover→Escape卡片确实消失→不点击不换页签不改光标、鼠标仍message→Ctrl+I立即重开清晰卡片，`target/s6-final-supported-desktop-launch.json` | 详见[普通桌面记录](vue-typescript-desktop-2026-10-20.md)最后两节。a45与1cae文字/绘制源码未再变，原E66–E69语言/事务/IME/字号等观察按原身份保留；B4受影响语言/编辑/主题/键盘/IME可见功能门槛闭合。不是全部Vue版本手动全集、OS DPI或任意输入法矩阵，也不声称退出exit0 |
| E76 | 退出限制及输入保护分别记证：1cae点击关闭后进程已结束，但`target/s6-final-supported-desktop-stderr.log` SHA `d1c7e8f47a33fd7d83ebabd1af90855819ba45b0564a453e65af95d455c77809`仍有AccessKit util380 ClientToScreen invalid HWND / COM cannot-unwind abort。普通临时6文件逐摘要核对后清理，`target/s6-desktop-visible-fixture-cleanup.json`确认临时根不存在，`target/s6-desktop-visible-fixture-precleanup.json`记录27项原项目磁盘摘要全等 | 与既有[窗口退役基线](../../ui/verification/project-home.md)同签名；未升级/fork上游、未断言只有自动化触发、不将功能截图或进程结束写成正常退出PASS。语言请求/编辑观察与专门checked shutdown/物理EOF回归分开；此已知限制保留在支持元数据和桌面记录，不冒称已修 |
| E77 | 功能支持元数据已限定应用T4/V5并冻结：T插件.15/native.14，V插件.12/adapter.11；`target/s6-final-support-metadata-applied.json` SHA `a3497417a3aef3e5c2b733246e764201134491f0db150556af6350825a81f036`。支持范围固定Windows x86-64、TS5.9.3、Node24.19.0、Vue2.6.14/2.7.16/3.5.22及26适用写法/固定依赖；`target/s6-final-supported-source-inputs.json`记录新独立T29/V44源码输入 | records[].status仍candidate，独立support/acceptance为supported、功能pending空；早期/未来版本、其它平台和动态语义限制独立保留。历史candidateVerification/retainedArtifacts原字节及47/11固定负载不动，不自嵌最终ZIP摘要或声称新包已测。最终归档/最小pair/policy/versions在外部宿主交付收据闭合；元数据独立审查与三仓commit/push/issue回执仍待B5，不提前勾完整AC |

### 最终精确双包、策略与独立审查（E78–E82）

本段将E77当时尚待的归档、策略和独立审查按真实终态闭合，不改E01–E77原始记录；技术验收与后续Git交付分开。

| 代号 | 真实结果与身份 | 证明边界 |
| --- | --- | --- |
| E78 | 支持标签最终双包公开构建/归档完成：`TS/target/s6-supported-015-package/typescript-0.1.15.zip` SHA `beab4419c7004ce96d4bf2ce3f4a826f0c8f2a777a9a4b0c3896496f314c038b`（22 entries/29 frozen sources）；`Vue/target/s6-supported-012-package/vue-0.1.12.zip` SHA `5f6a14c2e6b4aa139e33bcdbb3711d56ac03620b784f2dd86cda39810b7e1b73`（39 entries/44 sources）。release分别1m07s/1m09s，`target/s6-final-{typescript,vue}-package.log`。静态闭包独立核验0unexpected；`target/s6-final-supported-packages.{json,md}`，JSON SHA `9f956d536d0ce054a600f929a379afc78677305f664216e6087e178af4e4697f` | 源码/锁/公开SDK4af/各自managed WASM产物闭合；47保留资源和11Vue模块全等，native.14 SHA71c279…84f83及adapter.11不变。manifest raw摘要因公开packager重序列化而不同，独立深比较语义全等；未把raw=false写成字节相同。仅插件版本等已授权差异，不扩能力/权限；静态核验本身不证明运行，运行另见E79/E80 |
| E79 | **最终精确双包mixed S1 1 PASS/0 FAIL/0 ignored，97.62s**；`target/s6-final-pair-native.log` SHA `730338160e0f6bab1ed5ba623f7bcba80217886596314be2d496b34786cbfde9`；`target/s6-final-pair-inputs.json` SHA `3d8aff267b671bf5f1f6197d422adecaf6f542619ca0be0455a7c2146f89e43a`。实际程序 `beed53fdea083781c2a724e7185308430f2cbc8d44e3ae3fbb101d438ede40f1`、E78两包、SDK4af；成功profile `target/s6-native-ui/profiles/s1-verified-tRaa0W` | 三Vue项目经真实Package/Manager、生产Worker及TS-owned adapters请求源语义，含已加暖缓存真实Ctrl+I及原mixed/磁盘/checked shutdown断言。源包和程序实际重hash匹配inputs；这是支持标签新ZIP的实际运行，不拿E59/E60候选通过代替。普通持久窗口与故障/历史矩阵仍按E41–E76各自实际身份继承，不冒充本例重跑全部场景 |
| E80 | 最终新Vue public policy **1 PASS/0 FAIL/0 ignored，179.74s**；`target/s6-final-policy.log` SHA `cc3d819bfb0c121f29d64b892e73cfb42cb22ea225cf0461749769b5a0e276cf`；`target/s6-final-policy-inputs.json` SHA `d30c2e4eb5935cea4d0d95d546f844eda4be4e206487658ac33bdbefba8fcab7`；程序 `137fbed8191edd2b825bb3ff508647c3ea0459bf4a5cdff00d91bafb5a0c42a3`，E78 Vue ZIP/SDK4af | 通过真实Package/Manager与独立公开policy consumer/受控TS上下文元数据提供者执行版本证据、冲突、SDK及权限等既定策略。controlled TS metadata不等于实际compiler分析；精确新双包实际编译器语义由E79，历史能力由不变封印/原专项证明 |
| E81 | Vue版本策略 **5 PASS/0 FAIL/0 skipped，1345.2615ms**，Node24.19.0；`target/s6-final-vue-versions.log` SHA `d30b258cf88a5735377643582607512c99731a70e2e0881c97e7f1a7c63a68ee` | 实际公开native endpoint覆盖精确版本/冲突override、lock/broad声明证据、未知/不完整/alias/越界、帧限额及声明方法。records[].status保持candidate，独立support为supported；不外推未知未来或整个semver，不把5项当102语义套件重跑 |
| E82 | 最终九项支持元数据独立Spec/Standards **各0项可行动发现**；`target/s6-final-metadata-spec-review.md` SHA `8296c0486bfba61d9732e4efbb5675a4c3d1633c210774a4eb352e2c21e357ff`、`target/s6-final-metadata-standards-review.md` SHA `2bddd75c84d4d59f27b5fc7e29d0710ffafd4501c58b956841fa64266440fa1f` | 九项当前摘要匹配E77账本；历史candidateVerification/retainedArtifacts原文6824 UTF-8字节及SHA18cd6d…def6全等，准确区分固定版本/依赖/26写法、继承证据、协议candidate和独立supported功能标签，三份grammar provenance不扩手动观察范围。既有AccessKit窗口退役abort限制明确保留。独立静态归档审核另见E78，运行/策略另见E79–E81；技术审查不代替三仓commit/push/远端和新issue回执 |

### 35 项 AC 最新映射（恢复批次，终态截至 E82）

此表保留E01–E77全部原始证据及旧FAIL，以E78–E82闭合精确最终归档、实际pair、public policy、版本策略与最终元数据独立审查。当前适用行为和技术要求均有证据收口；S6-AC5的提交范围/Git收据及S6-AC7的实际交付尚待执行，不提前关闭完整工单。未知版本/平台/动态语义与E76已知AccessKit退役限制保留。

| AC | 截至本时点已证 | 技术结论、限制与交付边界 |
| --- | --- | --- |
| S2-AC1 | E02共享脏TS、E24原生选择/F12/隔离/偏好/重开PASS；E69普通桌面点击B、revision1持久记录、关闭重开及B根就绪日志已观察 | 行为已证；E78/E79最终身份闭合。选择/偏好及来源继承沿E24/E69，插件页头版本不冒称引擎版本 |
| S2-AC2 | E08/E16旧捕获/阶段、E25源/辅助/owner拒绝、E35再请求；E43旧map/preview、E50终态、E51 epochs及E53集合恢复PASS；E60暖缓存、E63共享源重开/fresh查询和E72/E73取消后late hover拒绝PASS | 旧结果拒绝与取消行为已证，E78/E79最终包闭合；E62错误fixture及E72/partial FAIL原样保留 |
| S2-AC3 | E03水位重建、E04 260×>64KiB同步有直接实际PASS | 同步/水位行为已证，最终身份E78/E79闭合；不扩大独立control RPC或编辑计划额度 |
| S2-AC4 | E23引用/paths/junction、E09源跨度、E25/E26 CRLF/UTF16、E42 external定位PASS；E66普通原源导航、E67真实OS IME提交/取消/Undo已观察 | 解析/位置行为已证，E78/E79按原封印继承；不泛称任意动态src、链接布局、所有输入法或DPI |
| S2-AC5 | E01 TS、E25未知语言、E41八行SFC选中resolve+import/single Undo及失效拒绝PASS；E68普通TS desktopTotal自动导入与一次Undo已观察 | 实际resolve/import/单次Undo与全拒绝已证，最终包E78/E79闭合；SFC依据E41，桌面TS一例不替代SFC，nextTick未出现候选不记PASS |
| S2-AC6 | E01/E07/E41–E43完整原生事务PASS；E66普通App/Child预览/取消/2项应用，独立“撤销整组操作”同时恢复2文件已观察 | 完整事务行为已证，E76输入保护及E78/E79最终身份收口；各文件Undo与独立组Undo分开，不重跑已证矩阵 |
| S2-AC7 | E05非法尾部、E08授权阶段、E25源/辅助/owner全拒绝；E43替换/卸载后旧preview撤销、Enter无旧编辑；E53不可逆必要映射拒绝/恢复PASS | 非法/只读/越界/旧map与授权拒绝已证，E78/E79最终身份闭合；预先invalid-map不替代E43实际旧preview撤销 |
| S2-AC8 | E01/E07/E25/E26/E41–E43沿唯一EditorState/DocumentSession；E66/E67/E68普通编辑、组Undo、IME和自动导入Undo已观察；无第二可写模型 | 唯一EditorState/DocumentSession及Undo路径已证，最终增量E82无新发现；提交范围/最终Git回执另待 |
| S3-AC1 | E09/E22声明组合及七项适用能力、E28三版本、E41/E42事务、E58闭包、E60暖缓存PASS；E66普通Vue3七项、E75主题/键盘已观察 | 适用能力与支持标签已收口：E78/E79精确最终包、E80/E81策略、E82审查；限三个精确版本/26适用组合，2.6 script setup仍拒绝 |
| S3-AC2 | E09 class2/JSX3/TSX3自身script/render、props/slots等工具；E41八行全部原生预览/取消/应用/Undo及自动导入PASS | 八行历史安全应用门槛已证，E78不变封印及E79最终包收口；保留固定依赖/合法写法边界，不另增全集 |
| S3-AC3 | E09 literal/const/有界wrapper、归属隔离/普通模块负例；E21闭包；E42 Vue26/TS与Vue3/JS实包脏源/导航/事务、主TS/磁盘/清理PASS；E58闭合 | 静态合法外部src行为已证，E78/E79闭合；动态工厂/继承等未声明语义不外推为支持 |
| S3-AC4 | E14/E15 native loader/缺失/撤销、E27/E58 grammar闭合；E66普通script/template/CSS可见高亮、E67滚动与字号改变后原源F12已观察 | 原生loader/注入/缺失/撤销及受影响普通可见高亮已证，E78全部相关资源不变；不恢复宿主内建grammar |
| S3-AC5 | E26四Windows parents、12行whole/range/选择/Undo/无保存/ABA及E25 pending IME PASS；E66普通SFC脚本/CSS实际格式化已观察 | 12行原生格式化、普通SFC/CSS与异步边界已证，E78/E79最终身份闭合；旧scheduler FAIL保留 |
| S3-AC6 | E15/E25/E26/E43/E14主题/IME bridge/缺角色降级；E67真实OS IME及滚动字号；E70/E71卡片/inline、E72/E73键盘完整回归与E75普通深浅可读/同符号重开均实际通过 | B4受影响可见功能闭合，E75与E78/E82范围一致；E76既有UIA退出abort未修，不写退出PASS或泛化主题/IME矩阵 |
| S3-AC7 | E21/E22公开闭包、E48/E58资源/封印；E74 normal公开SDK54/0Markdown全等4af；E78最终公开归档、E79实际pair、E80 public policy、E81 versions及E82独立审查通过 | E78精确公开归档/SDK/资源及E79–E81运行策略、E82独立双轴闭合；无ID白名单，协议candidate保持 |
| S4-AC1 | E11角色/集合工具、E34生命周期；E43未知四贡献发现/prepare/替换/卸载、E53完整冲突恢复均PASS | 发现/prepare/替换/撤销行为已证，E78/E79最终身份闭合；通用公开接口与只读hook边界未变 |
| S4-AC2 | E43四角色精确诊断、转换/配置/类型效果、磁盘/config/checked shutdown完整PASS；E53四类负向拒绝/恢复PASS | 四角色真实compiler与Worker行为沿E43/E53、不变资源E78收口；E79不冒充四角色全集重跑，admission不代替语义 |
| S4-AC3 | E43 count/shade到state.ts的F12、3 refs、两源完整事务；更新/卸载撤销旧map/preview，Enter无旧编辑；E53坏映射拒绝/恢复PASS | 映射事务及更新/撤销联合已证，E78/E79最终身份闭合；不以预先坏映射拒绝替代实际旧结果时序 |
| S4-AC4 | E11四类工具冲突、E22 ABI/引擎工具、E50 provenance三项；E53四类actual拒绝/移除/恢复及清理完整280.28s PASS | B1完整四冲突拒绝/恢复已闭合，E78/E79保留身份；E44目标RED与E52冷准备FAIL不改写 |
| S4-AC5 | E32 A/B/last-peer/物理EOF、E33多跳/lifetime、E34 Snapshot、E43替换/移除旧资源退休与健康功能保留；E53失败态恢复PASS | 归属/物理EOF/健康绑定/撤销已证，E78/E79最终身份闭合；service sets无Snapshot的原证明边界保持 |
| S4-AC6 | E43缺style只降对应角色、其它template/script继续，E14 grammar撤销，E29可见状态有专项证据；E69普通日志观察已就绪B根 | 缺角色/grammar只降对应能力已证，精确资源E78闭合；Quill/Ink不外推Pug/Stylus/Nuxt正式支持 |
| S5-AC1 | E31泛型准备/激活/迁移回滚；E46资源/反向保护；E51真实Vue两类候选失败及反向TS更新保持旧包/epochs/三语义/私有记录；E55撤权取消与正常更新PASS | 准备失败/激活回滚与真实两向更新保持已证，E78/E79最终包收口；E24非空偏好与E51零记录分开，不承诺恢复进程内存 |
| S5-AC2 | E36/E46 mandatory/WASM/resource-only反向预检RED→GREEN；E51 forward/reverse真实窗口保持三原语义，独立复核通过；E37目标RED保留 | 实际forward/reverse保护及旧结果失效已证，E78–E81最终包/策略闭合；无开放resource-only P1，不由API>=99负例外推 |
| S5-AC3 | E30缓存/离线/取消/坏摘要/并发lease/收据/optional、E55准备撤权与正向更新PASS；E69普通B根准备及启动日志已观察 | 依赖下载/离线/取消/摘要/lease行为沿E30/E55已证，最终封闭负载E78闭合；不把archive读取或ignored当网络通过 |
| S5-AC4 | E28内层compiler A退出/EOF/实际恢复；E51 outer真实EOF/有界重连、脏语义、旧结果拒绝、B/C保持及checked/HWND PASS | B2实际outer/inner有界语义恢复已证，E78/E79最终身份闭合；与E76普通UIA退窗abort分别记录，不混称退出PASS |
| S5-AC5 | E28信任/进程/ACK/HWND、E31回滚、E32/E33原caller/多跳、E34 Snapshot、E43扩展撤销、E51候选/故障后清理、E53/E55 checked shutdown均PASS | 生命周期/信任/旧资源与清理已证，E78/E79最终包、E82审查闭合；不把快照恢复逻辑状态说成进程内存恢复 |
| S5-AC6 | E24 scoped绑定/F12/重开、E16 connection、E35再准备；E43更换/移除、E50准入/取消/真实发布Err、E51 epochs、E53完整集合恢复、E55 authority交接PASS | 配置/选择/连接代次、重开与失败发布隔离已证，E78/E79最终身份闭合；E47旧seam/前置失败原样保留 |
| S5-AC7 | E29中英queued/失败/恢复、E40 SDK结构化拒绝、E55 typed stale有实证；E69普通日志显示B根、analysis/engine、准备完成和进程启动，未重复权限弹窗 | 中英状态/版本证据/恢复入口及正常授权路径已有专项与普通证据，E82确认元数据准确；页头插件版本不冒充引擎版本 |
| S6-AC1 | 35AC/14V；E41历史、E42 external、E43 mapped、E50 provenance、E51更新/故障、E53冲突、E55 Worker、E60/E63缓存、E66–E76普通交互/受影响回归/门禁；E78–E82最终包/策略/审查终态 | 历史重构、mapped事务、更新/撤销旧结果拒绝三项联合已证；E78–E82最终技术收口，Git交付另待，不重跑已完成专项 |
| S6-AC2 | E73 hover7/0/cards6/0/text2/0/Escape9通过1ignored；E74 normal42.94s、fmt/nonUI319/0/327/check2.82s、SDK54/0/0差异；E78两guest公开构建、E79 pair1/0/0、E80 policy1/0/0、E81 versions5/0/0 | E73/E74宿主门禁、E78公开构建及E79–E81最终精确运行策略通过；327及Escape1 ignored不计PASS，零匹配不计覆盖，旧工具结果按原身份继承 |
| S6-AC3 | E24–E26/E28/E41–E43/E51原生专项；E66七项普通操作/独立组Undo、E67真实OS IME/滚动字号、E68导入、E69 chooser/日志、E75浅/深正文/inline/片段与同符号Escape→Ctrl+I均已观察 | B4语言/编辑/主题/IME普通可见功能已闭合，E76输入保护有记录；既有UIA退役限制单独保留，不冒称正常退出 |
| S6-AC4 | E74最终normal公开SDK全等4af；E77精确supported功能标签；E78最终T15/V12归档与T29/V44源码/47资源/11模块闭合；E79真实最终pair、E80受控public policy、E81版本策略通过 | E78源码/SDK/锁/许可/47资源/11模块、manifest深层语义及E79–E81精确最终包通过；归档摘要/终态在外部host收据，不循环改插件包 |
| S6-AC5 | 此前生产资源/生命周期/provenance/缓存/配色/键盘独立增量审查已处理；E82最终九文件Spec/Standards各0项，E78静态归档独立0unexpected，E79–E81最终运行策略通过 | 最终生产增量及E82元数据独立双轴0项发现，E78归档独立核验通过；提交范围/文档检查及实际三仓Git收据仍待交付阶段完成 |
| S6-AC6 | 旧包/FAIL/数据保留，E30/E31离线/回滚、E51实际拒绝保持PASS；E77 supported功能范围严格精确三版本/固定依赖并保留历史字节、早期/未来/其它平台限制及E76退出限制 | 正式/基础/待验/限制、历史后端及数据保留准确，E78/E82闭合；未知未来与其它平台不自动支持，E76既有退出限制未修 |
| S6-AC7 | #121交付基线保留；本轮技术门禁与独立审查终态齐全，但尚未读回三仓完整commit/push及新issue关闭收据 | 尚未完成：根代理须实际普通提交三仓、push并核远端SHA，再读回新#122–#126；旧#108–#118与父#107不动，无Release |

### V01–V14 最新映射（恢复批次，终态截至 E82）

| 规格项 | 当前证据与真正剩余 |
| --- | --- |
| V01 | E09/E22七项、E28三版本、E41–E43事务、E60缓存及E66/E68/E75普通操作已证；E78/E79精确最终包、E80/E81策略与E82支持范围审查闭合，不外推所有版本手动通过 |
| V02 | E24原生歧义/偏好/F12、E02共享脏源、E69普通B选择/精确配置key/关闭重开均已证，E78/E79最终身份闭合；偏好不因归档重打改写 |
| V03 | E09合法Options/Composition/JS/TS/setup及2.6拒绝、E41/E42历史写法/外部块已证；E78/E82精确版本/依赖/支持标签闭合，不扩任意动态写法 |
| V04 | E41 class2+JSX3+TSX3八行native预览/取消/应用/group Undo及resolve/import/single Undo全过；E78封印不变、E79最终包闭合，八行不再待跑 |
| V05 | E23实际引用/paths/合法junction、E42 external脏源/导航/事务已证；E78/E79闭合归属/越界及静态关联范围，不声称无限动态解析 |
| V06 | E03/E04水位/大图、E25 ABA、E24隔离、E43旧map/preview、E50/E51终态/epoch/owner、E53集合恢复、E60/E63缓存及E72/E73晚结果拒绝已证；E78/E79最终身份闭合，旧FAIL保留 |
| V07 | E25 pending wire/UTF16、E26 CRLF/Unicode、E04大图及E67真实OS中文候选/提交/取消/Undo/光标已证；不外推所有输入法或DPI矩阵，E82支持范围保持 |
| V08 | E01/E07/E25/E41–E43完整事务、E66普通跨文件预览/取消/应用/独立整组Undo、E68自动导入Undo已证；E76输入保护与E78/E79最终身份收口 |
| V09 | E26十二行样式/独立formatter/无fallback、E27资源、E43缺style及E66/E67普通格式化/高亮/滚动字号已证；E78资源全等，不再要求手动重跑12行 |
| V10 | E34 Snapshot、E32/E33生命周期、E43四角色、E53四类冲突拒绝/恢复全过；E78闭合精确包，admission不等于compiler语义，E79另证实际编译器 |
| V11 | E30下载/离线/取消/摘要/lease/收据、E55准备取消/更新及E69普通就绪日志已证；E78封闭负载收口，不把B根就绪当网络失败全阶段复验 |
| V12 | E28/E31/E46/E43/E50/E51/E53/E55故障/回滚/撤销/更新/恢复已证，E78/E79最终包闭合；E76既有普通UIA退窗abort单列限制，不混称正常退出 |
| V13 | E40 SDK/缺TS、E29中英状态、E69普通B根/阶段已证；E78/E80/E81最终包/策略与E82精确supported标签闭合，插件页头不冒充引擎/后端版本 |
| V14 | E25未知语言resolve/原子失败、E34注册、E32/E33委托、E43角色、E50发布provenance、E53冲突及E74公开SDK已证；E78–E82最终归档/运行/独立审查闭合，无专属API或白名单 |

### 当前最小剩余执行顺序（恢复批次）

B1完整生态冲突、B2最终Native S5全链及B4受影响普通可见功能均已解除；E74最新host门禁/SDK、E75实际桌面及E76输入保护/已知退出限制分别保留。E78–E81闭合B3最终精确归档、实际pair、public policy和versions；E82闭合B5最终元数据独立双轴技术审查。当前行为及技术门槛不再列待跑，不因纯元数据打包重复历史八行/external/四角色/故障矩阵。尚未称新#122–#126全部交付，剩余仅Git和工单回执。

1. **B5交付部分：三仓普通提交与推送。** 根代理核对最终提交范围、文档和最终源/归档身份后，只提交本任务确认变更，保留原工作区其它内容；按既有授权完成普通commit及push origin，并实际读回三仓本地/远端SHA。当前未有本轮完整Git收据，不能以技术通过代填。
2. **S6-AC7：工单与最终收据。** 推送核对成功后，记录新#122–#126的实际验收/提交/归档关联并读回issue状态。只有实际交付完成才标记整项完成；旧#108–#118和父#107保持不变，不创建Release。外部宿主交付收据保留最终ZIP/测试身份及E76既有AccessKit退役abort限制，不再次改插件包写入测试结果。

执行始终由根代理统一调度 native Cargo / 实际 WASM / Windows 资源，本文维护者没有运行下列命令。历史八行、external、四项formatting、outer/inner恢复、正反向更新及完整四冲突已有实际PASS；以下仅是相关源码将来再变化时的历史复放索引，不是当前待跑清单。代码块固定指向当时已验TypeScript `.14` / Vue `.11`；E57候选及E78支持标签后的最终 `.15` / `.12` 分别保留，后者已通过E79精确pair、E80 public policy与E81 versions。实际新快照、程序、路径、SDK和归档摘要由对应inputs及外部交付收据记录，不把旧包exact结果套给新元数据。若日后复放，对选定测试exe保留双包环境变量及 `NANOBUG_VUE_FIXTURES`，先列匹配项确认数量；不得直接运行private child：

```powershell
# $FinalAppTest 指最后冻结并核 SHA 的测试程序，不默认采用 H3。
$env:NANOBUG_TYPESCRIPT_PACKAGE = 'C:/Projects/RustProjects/TypeScript-for-Nanobug/target/s4-context-package-014/typescript-0.1.14.zip'
$env:NANOBUG_VUE_PACKAGE = 'C:/Projects/RustProjects/Vue-for-Nanobug/dist/vue-0.1.11.zip'
$env:NANOBUG_VUE_FIXTURES = 'C:/Projects/RustProjects/Vue-for-Nanobug'
& $FinalAppTest --exact 'extensions::worker::vue_editing_tests::historical::historical_vue_forms_apply_native_rename_and_resolved_auto_import_with_undo' --ignored --nocapture --test-threads=1
& $FinalAppTest --exact 'extensions::worker::vue_editing_tests::external::external_vue_scripts_map_dirty_sources_and_native_rename_with_group_undo' --ignored --nocapture --test-threads=1
# 复放格式化时跳过内部 child，应恰好四个 parents；旧首轮未带 skip 的示例不是当前命令。
& $FinalAppTest 'extensions::worker::vue_formatting_tests' --ignored --skip formatting_child --nocapture --test-threads=1
& $FinalAppTest --exact 'extensions::worker::native_vue_recovery_tests::real_vue_backends_recover_without_reviving_old_results' --ignored --nocapture --test-threads=1
& $FinalAppTest --exact 'extensions::worker::native_vue_recovery_tests::real_incompatible_typescript_provider_preserves_working_vue_backends' --ignored --nocapture --test-threads=1
```

S4 入口及 env 以 S3/S4 记录和当前测试 ignore 说明为准：除 COMBINED、WITHOUT_STYLE、CONFLICT、BAD_MAPPING 外，四类拒绝还需 `NANOBUG_ECOSYSTEM_CYCLE_PACKAGE`、`NANOBUG_ECOSYSTEM_CONFIGURATION_CONFLICT_PACKAGE`，都指 P4 对应 SDK4af 新包。runtime 精确八目标/fixture 环境与各例 checked/unchecked 边界见 `target/s5-runtime-handoff-2026-10-20.md`；旧 P2 包及旧 paused 成功记录不替换本轮结果。
