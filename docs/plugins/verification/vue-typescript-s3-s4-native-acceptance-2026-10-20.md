# S3 历史写法与 S4 生态组合实际验收

日期：2026-10-20。范围为新 #123 / #124，以及与 #122 事务和 #126 联验重合的既定行为；旧工单仅保留历史，不推进或关闭。

当前状态：冻结 `5832E020…B1287` 的四项 FAIL 及真实断点保留。冻结 `703C8BA4…D4C04A` 的历史第六轮完整 GREEN 144.24s、external-script 第二轮完整 GREEN 104.93s、S4 四角色跨文件第八轮完整 GREEN 152.95s，均通过 checked shutdown。四类冲突第三轮 FAIL 162.39s 确认同一 Availability 下的失败缓存缺陷；最终原 reservation 发布证据修正版的小回归 GREEN 3/3、1.66s，两轴增量审查无新问题。冲突第四轮初始准备超时的 FAIL 127.22s 保留；第五轮使用 `706332EB…ABB399` 完整 GREEN 280.28s，四种明确拒绝、逐次真实恢复和 F12、checked shutdown 全部通过。本文各范围均有完整入口的通过证据，但来自下述明确冻结版本；不冒充统一交付门禁或所有依赖组合均已通过。

## 测试边界与固定输入

工作区为 `C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor`，HEAD `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 加本轮未提交实现。独立 Vue 夹具根为 `C:/Projects/RustProjects/Vue-for-Nanobug`。

历史八行和生态组合入口采用 `TestAppContext` / `VisualTestContext`，使用真实 `Package`、生产 `Worker`、独立 WASM 和受管 Node 进程；编辑仍进入唯一 `EditorState`、原生预览和 Undo。它们不等同于 WindowsPlatform 窗口或 OS 桌面交互。下节转存的四个格式化父测试则实际进入独立 WindowsPlatform 子进程，并检查其 HWND 和物理退出。两类证据分别报告。

本轮固定归档（SHA-256，执行前后再核对）：

| 输入 | 路径 | SHA-256 |
| --- | --- | --- |
| TS .14 | `C:/Projects/RustProjects/TypeScript-for-Nanobug/target/s4-context-package-014/typescript-0.1.14.zip` | `92C29C83C8FA4E72FEA83CFC11F3917773E43845C54A57D24E1F68D8DAE5E9C9` |
| Vue .11 | `C:/Projects/RustProjects/Vue-for-Nanobug/dist/vue-0.1.11.zip` | `8E665285AC0D2701BA513D2D11DDD63F61771A27E21843E6869F000A7EE16252` |
| 四角色 .2 | `target/s4-ecosystem-assets-sdk4af/combined/unfamiliar-combined-0.1.2.zip` | `3E8A249F96EE23E9B27ADA05BC5BF216E4133DD22ABA092DCD8B90702265FB15` |
| 缺样式替换 .3 | `target/s4-ecosystem-assets-sdk4af/without-style/unfamiliar-combined-0.1.3.zip` | `DF56DC420157285AF63A3FB7771E198A9B04FEDE02C6ABA5CFE49B65E28CAAC6` |
| 竞争冲突 .2 | `target/s4-ecosystem-assets-sdk4af/exclusive-conflict/unfamiliar-exclusive-conflict-0.1.2.zip` | `EAB82046C42BC5EA956CB1C973E32A317A888EE464D9FD48DBCD2D9E51151A71` |
| 顺序环 .2 | `target/s4-ecosystem-assets-sdk4af/ordering-cycle/unfamiliar-ordering-cycle-0.1.2.zip` | `567010B6A8CDD2522D6DA16539CBD876496ABDA5DDC3B35E320C46FD725ABC12` |
| 配置冲突 .2 | `target/s4-ecosystem-assets-sdk4af/configuration-conflict/unfamiliar-configuration-conflict-0.1.2.zip` | `67C2F8AFDDE63E2890A2E258AAB056F32D27AFA668F4DD3CCFEBCAF2825FE2EE` |
| 不可逆映射 .2 | `target/s4-ecosystem-assets-sdk4af/invalid-mapping/unfamiliar-invalid-mapping-0.1.2.zip` | `03B9DC1764FC85C60AC3CBC82777530A7ACF3EA8A1225ABD59BB12E472149AD7` |

准备阶段源码 SHA-256（均相对 `crates/editor-app/src/extensions/worker/`）：

| 文件 | SHA-256 |
| --- | --- |
| `vue_editing_tests/historical.rs` | `EFDF7EFA2E5A686675DD471DCC2CBEF8B7BCF189F4C3177A4E644B77D7417C30` |
| `vue_editing_tests/historical/fixtures.rs` | `536F6F92A77579653335761177E89777294814947E3C18C31D1A12F6F28B896A` |
| `vue_editing_tests/historical/transactions.rs` | `A0BA546E2A10205F573989AB0B637D460677022C32F5A7290D460B98E1BBB45E` |
| `vue_editing_tests/historical/completion.rs` | `A1D26384B627C48D008F0E68BF1A17DF6DF60EBEED7D5A53BC042D94D6440FF8` |
| `vue_editing_tests/historical/receipts.rs` | `24975D8C58F8115280F076C065258C1C018F12A5F3840A3849E8DE2C1169DBDE` |
| `vue_ecosystem_tests/roles.rs` | `60CC70B9809A569BBA6ED24830311D1D6FB258F61A63A0BBD9595A1BBCECAF79` |
| `vue_ecosystem_tests/roles/scenarios.rs` | `0CD011B3D897F2E2F3F0546262C0AC73233E97C3008605CFF600A058CAD599DC` |
| `vue_ecosystem_tests/roles/retirement.rs` | `F9A12946E29783E80D4C764449DBBE344BC64E707BD0701D919F42A2E19EB044` |

根代理交付冻结程序 `target/s6-cutover-epochs-provider-red/deps/editor_app.exe`，本次已读回 SHA-256 `704679343F8E87A8F4D0B7E042B83D3B48C9D012AA5A3D9552D1DEC6F0B83DBF`。同目录树 `examples/lsp_fixture.exe` 的摘要为 `A7E30FB146622239228ED175937B473C0D9343754674707EA828754EB4A76075`。根代理构建 PASS 2m10，包含上述历史回执修正、S4 const literal 观察和委托取消隔离；本代理未自行构建或复制程序。

## 已存在的四项格式化 WindowsPlatform 通过记录

事实转自 `target/s3-formatting-execution-2026-10-20.md`，本次逐一读回以下日志的通过终态及摘要。未把其旧输入替换为本轮 TS .14。

- 程序：`target/s6-native-format-context-locale-red/deps/editor_app.exe`，SHA-256 `8A5BF17237C3716FFF31488F8AB106A5A9DC0667B6B4A0FA8C93186A812FB6E3`。
- 输入：TS .12，SHA-256 `F869529D1906D0A06C8A6C90225DFA09377092C1AE6654F20273A5EBB4A1C3A5`；Vue 同 .11。
- 同目录 `examples/lsp_fixture.exe`，SHA-256 `A7E30FB146622239228ED175937B473C0D9343754674707EA828754EB4A76075`。

| 父测试（`vue_formatting_tests::` 下） | 实际 parent / child 秒 | 日志与 SHA-256 |
| --- | --- | --- |
| `matrix::installed_vue_whole_and_selected_style_formatting_preserve_source_and_single_undo` | PASS 101.79 / 101.63 | `target/s3-native-formatting-matrix-first.log`；`E702AF87850B094F17E09AA75E73CA612D94E9C22029E89438EB49F0E81626A7` |
| `boundaries::installed_vue_rejects_empty_cross_block_attribute_and_external_src_format_ranges` | PASS 93.09 / 92.95 | `target/s3-native-formatting-boundaries-first.log`；`BD34F36A29ACD8C79C5B5689DD6F40CC51340852F6994897CE56876730A80B69` |
| `matrix::installing_vue_preserves_the_existing_typescript_formatter_and_primary_analysis` | PASS 70.28 / 70.13 | `target/s3-native-formatting-provider-first.log`；`92F77663B8B0660F3D656C646B30452467A589C4E52278E43035DF32B400D0D8` |
| `selection::installed_vue_format_rounds_cannot_overwrite_same_turn_native_input_or_ime_aba` | PASS 93.14 / 92.99 | `target/s3-native-formatting-aba-first.log`；`D4D97DF3627FE6EB7AE61593996F133F9BDEA4D6F102D664E549831CC310F572` |

每项父测试实际 1/1，通过场景断言和 checked cleanup 后自然退出。matrix 覆盖三 Vue × CSS/SCSS/Sass/Less 的 12 行 whole/range、single Undo、块外源/属性/外部 src、CRLF/Unicode 和不保存；boundaries 覆盖空/跨块/属性/外部 src range 拒绝及无 whole fallback；provider 保持既有 TS 分析和显式格式化选择；ABA 保持 range/whole × 输入/IME 取消的旧结果拒绝。以上不替代八行历史编辑和本轮生态组合，也不证明 OS 候选窗交互。

## 历史八行：既有断点和本轮待验阶段

完整入口：`extensions::worker::vue_editing_tests::historical::historical_vue_forms_apply_native_rename_and_resolved_auto_import_with_undo`。

所有行均要求同一完整链：脏源 → 未打开 Parent 发现 → rename 全量预览/取消/应用 → 真实回执 group Undo → 原生输入排队重放 → 真实选中补全 resolve 和 import → single Undo → 磁盘不变。原 180 秒准备、30 秒控件、15 秒源观察预算保持；结尾 checked shutdown 后捕获的所有 plan 必须 inactive/process0。

本次只读核对被复用 dependency junction 的实际包：Vue26 为 `vue@2.6.14`、`vue-class-component@7.2.6`、`vue-property-decorator@9.1.2`、`vue-tsx-support@3.2.0`；Vue27 为 `vue@2.7.16`、同 class/decorator 版本；Vue3 为 `vue@3.5.22`，未凭名称增加 Vue3 class 组合。三个固定项目 `package.json` 的 SHA-256 分别为 `4C6A05FF78E55AA8234A93E9A0CA0BEAE8F6D513D52703D3D798C36C82938538`、`992C6D03B62AFEC8CCC6B7BB1EF6A1C011022665F7E046170217495F5C2B873A`、`975AC2A8DF08AB19633C6D4D3276C3CE01C970BE6DF807AA7DC240C4D372B382`。这项来源核对不替代完整八行语义验收。

| 固定行 | 本轮结果 |
| --- | --- |
| Vue 2.6 class / decorator | 第六轮完整链 PASS |
| Vue 2.7 class / decorator | 第六轮完整链 PASS |
| Vue 2.6 JSX | 第六轮完整链 PASS |
| Vue 2.6 TSX | 第六轮完整链 PASS |
| Vue 2.7 JSX | 第六轮完整链 PASS |
| Vue 2.7 TSX | 第六轮完整链 PASS |
| Vue 3 JSX | 第六轮完整链 PASS |
| Vue 3 TSX | 第六轮完整链 PASS |

旧轮次保留原终态：

1. `target/s3-historical-native-first.log`：FAIL 89.61s。首行 rename / group Undo 已过，补全输入前置立即读取未等待 linked-input 重放；随后仅修测试观察和输入目标诊断。
2. `target/s3-historical-native-013-second.log`：FAIL 126.30s。前三行完整阶段 PASS，第四行 Vue 2.6 TSX 的源观察失败。不是八行通过；后加 source phase 和实际回执命中诊断。
3. `target/s3-historical-native-014-third.log`：FAIL 90.22s。首行 rename postimage PASS 后，测试在普通 update 阶段误调 paint-only `Window::content_mask`。现 receipts SHA `24975D8…19DBDE` 仅移除此非法观察，保留真实 wheel / pointer-down / pointer-up、旧回执和原预算；修正尚待本轮完整复验。

本轮第四次实际运行（上述冻结程序、TS .14 / Vue .11）：`target/s3-historical-native-014-fourth.log` **FAIL 87.27s，0/8 整行通过**。Vue26 class 的 cancelled preview、取消不变、confirmed preview、全量 rename postimage 均精确通过；第一个 Undo 按钮 bounds 为 `(23.5,724.5)`、`252×28`，处于 derived `544..764` 区域且 `painted_visible=true`，pointer-down 后快照 `focused=Some(false)`，测试在此失败。

第四轮日志 SHA-256：`A061BA8D613D5F47045DE2532E1F79919C67B3F545939C8C4F236B92CADC9CAA`。执行后冻结程序和 TS/Vue 包摘要与准备表一致。

仅修改测试观察：`ElementSnapshot` 是上一 paint 的事实，新增 `window.refresh()` 后再 draw，继续保留同一按钮的 pointer-down 焦点断言；失败再报告实际 mouse / 当前焦点及覆盖该点的可见节点。上游 `gpui-base 0.7.0` / GPUI div 的 focusable MouseDown bubble 确实会请求焦点；该证据仍不能只凭最后绘制快照判定物理命中或缓存根因。修改后 receipts SHA-256 `277C2F4A53EA008DD24269286E18348217E23AE064743D16442890ACC78FA7C5`，scoped rustfmt 检查通过，**未编译/未复验**。不移除旧回执，不调用内部 Undo。

根代理随后要求将相同行为集中供历史及新 external-script 用例复用：现唯一实现位于 `vue_editing_tests/receipts.rs`（SHA-256 `D7A8BDA60FABBD96435A26E827BA68BF1E10A0389E65DDA5B5FACE15C7E776F4`），两份子目录副本均通过补丁移除，两调用方直接传各自原 phase，无仅转发包装层。历史测试名和全部源/Undo 断言不变。模块连接 `vue_editing_tests.rs` 摘要 `EA34B465EE1FD68BEBB8DC9CA4F685A48F95E211F18F4DCB4A4E3A0AAF68FF58`；历史主体 `488EBB709D430EB9F1628BFF36993CEB64D2A3B3C84F1EB8BABA4938653FD790`；历史 transactions `5F5F774FC5C193E583F36C0BD8554BD27CA653D056C44A1B76546F2F0B73C92C`；external 调用方 `10C9452FCF2CA75200BB00E8A9461DA0C2A3C0658B068253F76FB35AEEAB29B5`。定向格式检查通过，仍待根代理统一编译及真实复验。

## S4 四角色映射、生命周期与四类拒绝

两个完整入口：

- `extensions::worker::vue_ecosystem_tests::roles::scenarios::four_ecosystem_roles_map_native_queries_edits_replacement_and_removal`
- `extensions::worker::vue_ecosystem_tests::roles::scenarios::conflicting_and_irreversible_package_contributions_reject_and_recover`

旧冻结程序的第一项第六轮曾 FAIL；后续完整结果见下节。第一项必须覆盖四角色实际诊断精确源跨度，template / style 的 F12 和 refs、rename 预览/取消/应用/Undo；携带 pending style preview 更新为缺 style 包，旧 plan/process 和 preview 必须撤销，template 保持；再携带 pending template preview 卸载贡献包，旧映射失效，普通 script hover 仍正确；不生成配置/源文件并 checked shutdown。第二项分别安装竞争、顺序环、配置冲突、不可逆映射包，要求明确错误和原源不变、无可确认的编辑前缀；移除后每次恢复诊断和真实定义查询。四项拒绝不外推为 ABI 或所有配置组合已验收。

旧 S4 第四轮 `target/s4-four-roles-assets-sdk4af-fourth-native.log` FAIL 118.55s：实际 attach 返回 Adapter failed to load；TS .14 已修规范入口路径，需由后续真实运行验证。

旧第五轮 `target/s4-four-roles-native-014-fifth.log` FAIL 242.78s：已出现真实四类标记，UTF-8 范围为 numeric `66..68`、template method `154..165`、unknown component `174..185`、style method `225..236`，整项仍未通过。当前源码同时接受 TS const literal `42` 及 widened `number` 的同一无此方法错误，仍精确检查原 token、数量和范围；不可把旧失败改称通过，或仅凭这些 marker 假定 F12/refs/事务和生命周期均已触达。

本轮第六次实际运行（上述冻结程序及固定包）：`target/s4-four-roles-native-014-sixth.log` **FAIL 150.92s**。四角色 marker / primary provider 的首个 wait 已通过；后在 `control("workspace-plan-confirm", true)` 的原 30 秒期限失败。旧日志没有 symbol / apply 轮次，不能据此认定具体哪次取消、应用或 pending preview 失败。

第六轮日志 SHA-256：`09948EAEA4C8F3379D88A55CBF4D15ACB011005EE1FF115535C473351BD9A08E`。执行后冻结程序和 TS/Vue 包摘要与准备表一致。

已仅补测试阶段和失败观察：每个 symbol 的 definition → references → mapped rename、每次 apply=false/true 的 prompt / dispatch / preview；控制器超时打印生产 `app.status`、active path、Editor Entity、原生 DocumentVersion、selection、实际焦点、语言状态与可见 Dialog/workspace 控件。新 `roles.rs` SHA-256 `0CE4A940FEA33B37A63A1DE645B74A8BD77D55278343C7C27DB8E1FE93542176`，`roles/scenarios.rs` `AC4E962513054E271772960930B5614D92D3C9468B9F46CF8896D76FC9574B39`；scoped rustfmt 检查通过，**未编译/未复验**。产品行为、原 180/30/15 秒预算及所有 source / mapping / process 断言不变。

## 新诊断冻结程序的四项实际结果

程序为 `target/s6-preflight-external-diagnostics/deps/editor_app.exe`，本次读回 SHA-256 `5832E020B94D7DCF2CBF273ACBF2E351F83B612F94E2FE8E9555FF69CD4B1287`；根代理构建 PASS 2m56。此程序包含共享原生回执驱动、强制刷新后的焦点/重叠节点观察和 S4 阶段诊断，尚不包含下节针对真实失败的后续修正。同树 `lsp_fixture.exe` 仍为上述 `A7E30F…6075`。执行前后 TS .14 / Vue .11 的归档摘要均与固定输入表相同。

四项按顺序单独运行，无 Cargo 或并行 Vue fixture 写入。调用形式为冻结程序加下列完整过滤词及 `--ignored --exact --nocapture --test-threads=1`，环境为固定输入表各归档及 `NANOBUG_VUE_FIXTURES=C:/Projects/RustProjects/Vue-for-Nanobug`。

| 过滤词（`extensions::worker::` 下） | session / 终态 | 实际证据及日志 SHA-256 |
| --- | --- | --- |
| `vue_ecosystem_tests::roles::scenarios::four_ecosystem_roles_map_native_queries_edits_replacement_and_removal` | 60686 / exit101，FAIL 156.48s | `target/s4-four-roles-native-014-seventh.log`；`198A7E04F1F55D6113A4BBD5BBA4D4FBB55D7AD04547C7326D561B1F21AF5E16` |
| `vue_ecosystem_tests::roles::scenarios::conflicting_and_irreversible_package_contributions_reject_and_recover` | 60984 / exit101，FAIL 157.11s | `target/s4-conflicts-native-014-first.log`；`66AE11122FB9CDF207A28B7836CD0868C50D3E6C64C36B5C4BAFC90492BF0F77` |
| `vue_editing_tests::historical::historical_vue_forms_apply_native_rename_and_resolved_auto_import_with_undo` | 98464 / exit101，FAIL 90.68s | `target/s3-historical-native-014-fifth.log`；`37402EAF460E7283C96739D8FC6E1A62230FF6957EC0C2DB272B5438752BBED0` |
| `vue_editing_tests::external::external_vue_scripts_map_dirty_sources_and_native_rename_with_group_undo` | 65171 / exit101，FAIL 95.63s | `target/s3-external-native-014-first.log`；`49108896E2CFC9F6D6E987451DF77A76CA45D96ABC9621500CC5676A3EB21DFF` |

S4 第七轮通过四角色精确原文诊断、count F12 及 refs；首个 count apply=false 的 rename prompt 和 Enter 都已派发。随后确认控件未出现，原源已成为 Modified / revision228，原生 DocumentVersion revision456，context Ready。源码核对表明原夹具只有单个 App.vue 的编辑，按现有 `editor/language_edits/rename.rs` 单文档契约直接应用并保留本地 Undo，不能以它要求跨文件预览；本轮 FAIL 不代表产品应强制所有 rename 弹确认。完整替换/移除阶段尚未触达。

冲突第一轮明确出现 `Exclusive contribution slot conflict: template-compiler`（-32005），原源未变；测试紧接发送卸载后，旧 configuration 的预期拒绝仍在原生 publication 中，原 `wait_effects` 立即将它误判为新恢复失败。顺序环、配置冲突、不可逆映射及各次恢复 F12 尚未触达。

历史第五轮首个 Vue26 class 的预览/取消/应用后像精确通过；external 第一轮首个 Vue26 / TS 行走到真实 group Undo。二者都在 `(149.5,738.5)` 按下 Undo，按钮 bounds `(23.5,724.5) / 252×28`，painted_visible=true 且完整位于回执区内。强制 refresh/draw 后实际焦点均为 `FocusId(2v1)` 的 Explorer tree，同一点下的 `README.zh-CN.md` TreeItem 被选中，Undo `focused=false`。因此不是只凭旧快照判断：浮层缺少阻断下层 hitbox 的行为。两项全量 Undo 未完成，八行和 external 的其余组合均未验完，checked cleanup 未触达，不宣称通过。

## 对真实失败的最小修正

以下源码已统一编译进入 `703C8BA4…D4C04A`，各自实际复验结果在后续执行回执中记录。

1. `extensions/workspace_transactions/presentation.rs` 的回执浮层增加公开 `.occlude()`；GPUI `BlockMouse` 的逆序 hitbox 命中会阻止下层 tree 同时处理 MouseDown。未修改 `EditorState`、批次应用或 Undo 实现。原生真实滚轮/MouseDown/MouseUp 及完整回退后像仍是通过条件。
2. 唯一回执交互实现移至 `extensions/worker/vue_test_support/receipts.rs`，historical、external 和 S4 mapped rename 三处直接调用，不留转发包装或副本。它仍保留旧回执、原 30 秒预算、真实 hover/down/up 和命中诊断。
3. S4 夹具新增原始 `state.ts` 的 count/shade 导出；App.vue 命名导入后在模板/样式映射使用。F12 必须命中原始 state 声明；refs 独立推导并精确比对三个原文位置；rename 必须对 App.vue 与 state.ts 一起预览、取消、应用及 whole-batch Undo。两文件磁盘不变，项目实际条目数为四，tsconfig 只扩充既定 TS 源包含。pending preview 跨贡献替换/卸载后的两原文和磁盘检查保留。没有调整产品的单文档 rename 契约。
4. 冲突恢复沿用一个原 180 秒预算，先捕获拒绝 owner、错误和 configuration revision；卸载发布前仅容许原 owner 的同一已识别错误，待包条目移除且 configuration 推进后经过一个完整原生 frame，再使用普通失败检查和完整 role effects/F12。新错误不被吞掉，不增加额外等待预算。

本次修正源码摘要（均相对 `crates/editor-app/src/`）：

| 文件 | SHA-256 |
| --- | --- |
| `extensions/workspace_transactions/presentation.rs` | `C5F550C7F425F6A2C59F89D492A6BAEEDDC2F0C38B1DA3076C7C634EFF7F7B06` |
| `extensions/worker/vue_test_support/receipts.rs` | `C47FAB49EFEDCFD1D254A9CCF895A8A2310727ED478D4C5E3031D51DDBDC9859` |
| `extensions/worker/vue_test_support.rs` | `346BB38CABE40E07FCAF33105F55B7AD9794D077F64B8C469AB3BB3BB5CA7F19` |
| `extensions/worker/vue_ecosystem_tests/roles.rs` | `E3633217B18CB2EDA3212439B036E85D9CC6D42E4B3B3422BD77530447679849` |
| `extensions/worker/vue_ecosystem_tests/roles/mapping.rs` | `B351845206A1D910856B2D911490EA430CBEE60F25492566560D2BAFA6BE31E6` |
| `extensions/worker/vue_ecosystem_tests/roles/scenarios.rs` | `47085DD58AE2DFDEAE8EAE737BE31E1CDCBE201DCA182283B73B3D23FB05C677` |
| `extensions/worker/vue_ecosystem_tests/roles/retirement.rs` | `FE0F82F05C19EA12CC892FE181B9EE848D2871227244747A04B01517128CE563` |
| `extensions/worker/vue_editing_tests/historical/transactions.rs` | `DDF91270D51FF7A532366BDEFBCBD74934C2338836F604618091AE64C91D0BAC` |
| `extensions/worker/vue_editing_tests/external.rs` | `08602C734941DD40D13C56CA052B429FB47F281957D828181812C09355BA5048` |

## 后续执行回执

上述四个 session 全部已退出。定向 `rustfmt --check --edition 2024 --config skip_children=true` 与本范围 `git diff --check` 均 exit0；后者只有既有 CRLF 转换提示。只读独立核对确认回执的公开 `.occlude()` 机制及层次正确，尚不替代真实交互验收。

根代理新冻结 `target/s6-receipt-occlusion-and-preflight/deps/editor_app.exe`，SHA-256 `703C8BA44D5B6927EF8B5A748E734FFE8B8F99D5D7CED2A2F79BC5A516D4C04A`，no-run PASS 2m43；包含上述完整修正。同树 A7E30F fixture/PDB/DLL 已由根代理复制。执行代理启动前读回 exe 摘要一致，历史第六轮 session 43393，日志 `target/s3-historical-native-014-sixth.log`。此时仅该项运行，其后按 external、S4 四角色、冲突串行执行，不并行修改 Vue 夹具树。

历史第六轮已 exit0，**1/1 PASS 144.24s，八行完整通过**。日志 SHA-256 `D71C0462464778BA1F77328B2557D89BB8C518183D01E656CAA6F4EC67CED35F`。各行实际预览/取消/全量后像、批次 Undo、原生排队输入重放、选中补全 resolve/import 和 single Undo 均通过；结尾 checked shutdown、所有捕获 plan inactive/process0 及源/配置磁盘原文不变断言均执行通过。第四行实际通过滚轮将回执从 y778.5 移至 y682.5 后点击，后续更深及反向滚动也通过；没有清除历史回执或调用内部 Undo。固定依赖组合的这八行证据不外推至全部历史版本或任意依赖版本。

external 第二轮随后串行运行，session 79652，日志 `target/s3-external-native-014-second.log`，使用相同程序及 TS/Vue 固定包。已 exit0，**1/1 PASS 104.93s**，日志 SHA-256 `B860AC2D3F52CC033D1CE56339D1F7D407D4F9E66B871A297FE867F2FB1C5B01`。Vue2.6/TS 与 Vue3/JS 两组合的脏关联源精确诊断、原始 F12/refs、跨文件预览/取消/应用及真实 group Undo 均通过；两次实际回执 pointer-down 命中按钮。TS 主提供者保持、源/元数据磁盘字节不变、checked shutdown 及所有捕获 plan inactive/process0 断言均执行通过。该证据仅覆盖这里的两种合法 external-script 组合。

S4 四角色第八轮随后串行运行，session 92333，日志 `target/s4-four-roles-native-014-eighth.log`，使用新跨原文件夹具和固定 combined .2 / without-style .3 包。已 exit0，**1/1 PASS 152.95s**，日志 SHA-256 `C9B7A4361B6BEE0C9F19CB787CCE02191B3A7831C60F5EE20638665835DB1A42`。四角色精确原文诊断通过；count / shade 各自 F12 到 state.ts 原始声明、refs 三个原文位置、双源预览/取消/完整应用及真实 group Undo 全部通过。携带待确认 style 预览更新为 .3，原 Arc 退休、process0、旧诊断拒绝及预览撤销通过；reduced template 的 count 整链重验通过。再携带待确认 template 预览卸载贡献包，原映射及预览失效，普通 script 的实际 hover 恢复正确。双原文/磁盘/config 不变、四项目条目及 checked shutdown 断言全部执行通过。

冲突第二轮随后串行运行，session 32931，日志 `target/s4-conflicts-native-014-second.log`，固定 combined .2 加四种各自 .2 候选。已 exit101，**FAIL 154.55s**，日志 SHA-256 `6D240E6B4CDEB7A2F3998740AC6FD6B804CA4B0676F5AC6D9D5478C270A94AAA`。首次竞争拒绝正确可见；worker 已打印移除发布 `14->16`，但再一个共享 helper frame 后 `app.language_service_states` 仍保留同一旧 exclusive refusal，普通失败断言因此触发。未到达其余三种拒绝或 checked shutdown，不称四类冲突通过。

进一步核对发现 registry 的 `configuration_revision` 推进只证明 worker 发布。`ExtensionPanel::poll` 先刷新 catalog，再以 `cx.defer` 标记原生 contribution sync；测试的 cached draw 不能单凭“下一次 predicate”证明 `shell.render` 内的原生 reconciliation 已运行。因此仅修改 test helper，在包条目消失及 revision 推进后明确调用已有 `app.sync_runtime_contributions(window,cx)`，与已通过的 WindowsPlatform 格式化/恢复 harness 的 frame 路径一致；下一轮继续普通失败检查。原 180 秒总预算不变，不按相同错误文本忽略新失败；若明确 reconciliation 后仍失败，按真实产品问题继续定位。新增日志打印新旧 Availability、原生/catalog generation 和采用后的 service states，以便区分两者。

这次只修改 `extensions/worker/vue_ecosystem_tests/roles/scenarios.rs`，源码 SHA-256 `085C3F1E708ACAA93BFE43DAD8D3BA7C14D86B623D1B7435A252CD929E6BE508`，定向 rustfmt 检查通过。上述三项 GREEN 使用的是表中的冻结源码，未被重命名为此新 observer 的验证。

根代理统一构建 PASS 2m33，交付 `target/s6-contribution-recovery-adoption/deps/editor_app.exe`，SHA-256 `064129A3F58120C13B543FDC6059A1AFFE95198103DD051155F558ACBB24FA48`；paired `examples/lsp_fixture.exe` 为 `A58DAD702A78BF96AA1351F7115E9487FE4AC9EE8A4FA5C8833E6028BDF0414F`。执行代理已读回两者摘要一致，待根代理原生窗口场景终态并释放 Vtarget 后，仅独立启动冲突第三轮 session 84738，日志 `target/s4-conflicts-native-014-third.log`。TS .14 / Vue .11 / S4 固定归档和原预算不变，尚待完整拒绝/恢复/cleanup 终态；本轮不改文本或 close/reopen 绕过缓存。

第三轮已 exit101，**FAIL 162.39s**，日志 SHA-256 `28E481DA6C4D0973842F882584151E2FF48946CA995A580416BA4EA463EF8943`。首次竞争拒绝正确；卸载发布 `14->16` 后，新旧 Vue Availability 完全一致（manager2 / generation3 / configuration3），native_generation 与 catalog_generation 都为2、pending_sync=false。显式 `sync_runtime_contributions` 后仍为同一旧 exclusive Failed，下一普通失败断言再次触发。因此已排除 cached render 或“少等一帧”解释：集合贡献改变没有使终态失败准备及原生失败上下文失效。其余三种拒绝及 cleanup 仍未触达。

## 集合变化后的失败准备失效修复（未验证）

只扩展通用宿主内部机制，不修改 SDK、公开协议或插件标识分支：

- 文档 reservation 在撤回任何旧 binding 前，对其 manifest 声明的 collection 依赖保存原 scope、contract、单调 revision 及原 Arc epoch。直接复用 Broker 现有 `CollectionState`，含可选空集和显式偏好；metadata-only 观察不发引用、不执行 guest，沿用原1024项上限。上限错误发生在旧 binding 撤回前。revision 加原 epoch 指针/live 检查同时拒绝 ABA 和表条目重建碰撞。
- `Prepared.failure_inputs` 只影响终态 Err 的保留；pending 与健康 Ok 保持原权限及 lease 撤销规则。失败发布沿用 reservation 的原快照，不能在错误返回后采新快照掩盖变更竞态。没有 source reservation 的旧 workspace-only pure preparation 保持原行为。Broker 比较锁在驱逐缓存前释放，避免销毁原有 plan 时持有 Broker 锁。
- 原生 `ContextRecord.prepared_failure` 只标记完成的准备失败或明确读取到的缓存 Err。既有 Worker publication 撤回对应失败后，原生记录重新发起受来源/revision/权限校验的真实 demand；直接 admission 错误不会成为每帧盲重试。没有改源、保存、close/reopen 或扩大预算。

冻结生产源码如下；本次未增加测试专用 API 或仅镜像实现的私有单测，实际完整 `conflicts` 已产生上述可观察 RED，待同一入口 GREEN 及既定回放。

| 文件（相对工作区） | SHA-256 |
| --- | --- |
| `crates/plugin-runtime/src/plugin_services.rs` | `39EC195EAF4CF08376EF8CFDD90A60CA17590C9C2621BAF517E69C100B415804` |
| `crates/plugin-runtime/src/plugin_services/collections.rs` | `A966947303282DD0108D10214EA40F831A561D023A2CF9BC8E28555B3FD813B0` |
| `crates/plugin-runtime/src/manager/language.rs` | `2AB10E8EA1B502EDB4AF1AA7A32E9AF75D22E844E06A066B459AAB985FA3A35C` |
| `crates/plugin-runtime/src/manager/language/contexts.rs` | `5F194277D1B6EDF02CB42A22BC47D572F0DCC2F62A97C008CCB3B34AAF386040` |
| `crates/editor-app/src/app/language_servers/demands.rs` | `9814CE924B256F94127EFFC63F78CBDEAC75EE87A620730E26B0DD9B30D54BF6` |
| `crates/editor-app/src/app/language_servers/contexts.rs` | `080A53B26C2BC1CCFE1B740E5E494ED96139F5A5A949302AC7F336C97E118714` |

`roles/scenarios.rs` 仍为 `085C3F1E…BE508` 的完整四冲突回归。此补丁定向 rustfmt 及 scoped diff check 均 exit0（仅 CRLF 提示），尚未编译/运行，不宣称缺陷修复通过。当前无执行代理的测试、Cargo 或 Vtarget 写入；由根代理统一构建和调度下一原生窗口/冲突复验。

冻结后只读核对实际归档暴露了该首版补丁的覆盖缺口：Vue .11 的 `manifest.json` 只声明 `typescript.project`、`typescript.adapter`、`typescript.analysis` 三个普通依赖；TS .14 才声明可选 collection `language.contribution`，实际在 `contributions.rs` 中 OpenSet。因此只观察原 Vue 的直接 collection requires 会得到空快照，不能处理此次 Vue→TS→collection 的传递失败。此事实已在根代理构建期间报告，未在读取中的源上继续修改。需扩大原 scope 的受观察声明集合或正确跟随声明依赖图，仍只撤回失败记录、不动健康计划；在补齐并得到同一实际回归 GREEN 前，该源码表仅为未验证中间实现，不代表缺陷闭合。

根代理随后停止并核对本轮编译进程树，授权采用更小的保守范围：原 scope 已公开服务 providers 的所有声明 collection，加 source 自己的 collection。修正版按此观察既有 `CollectionState` 原 revision/epoch，覆盖 Vue→TS 间接集合，不扩 Provider 或 SDK。明确边界是同一 scope 的无关集合变化也可能使终态失败重试一次；健康 Ok/pending 的保留与撤销规则不变。

独立 Standards 另发现异步准入问题：`DemandWait` receiver 的 Err 也可能来自尚未 reservation 的 actor/Manager quota 或校验拒绝，不能一律认为有可撤回的准备失败。仅靠 UI 曾观察 publishedErr 又会遗漏“准备错误先撤回、结果后送达”。修正版以 `Work::Demand` / `DemandWait` 内部共享的 admitted bit 记录阶段，仅在 `manager.begin_demand` 返回 Ok(request) 后置真，异步准入拒绝保持 false。该宿主内部标记不进入公开协议；已接受的准备失败即使原 publication 先撤回，也能重入；直接准入拒绝不会逐帧重发。

新增两个普通 GPUI 回归位于 `extensions/worker/worker_tests/language_failures.rs`，使用实际 resource-only Package→Manager 未知语言，沿原 recordedWorker `Work::Demand` / `Demands.dispatch` 接缝观察 native failed 状态和队列：

1. `extensions::worker::worker_tests::language_failures::asynchronous_language_admission_rejection_does_not_requeue_each_frame`：实际排队后 Manager 撤信任，异步 begin 准入拒绝；12个原生 frame 不得新增或累积同 intent Demand，且没有 prepared publication。
2. `extensions::worker::worker_tests::language_failures::withdrawn_prepared_language_error_requeues_once_even_before_ui_observation`：按需解析时缺失可执行文件，产生真实 Manager preparedErr；通过既有 mockWorker publication 接缝分别安排先观察后撤回、先撤回后 receiver 送达，每序都必须只有一次同 intent 更新，后续8帧 pending 不重复。该小例不代替实际生态包的 runtime 集合撤销主验收。

这些测试不需 WASM、原生 child、在线依赖或 Vtarget 夹具。根代理临时仅恢复旧的 receiver Err 无条件 `prepared_failure` 判定以取得行为 RED，再恢复 admitted 判定；实际前置失败和首次目标 RED 见下节，GREEN 和完整四冲突尚待后续结果。

修正版冻结摘要如下，取代上表作为下一构建输入（保留中间实现账本，不改写历史）：

| 文件（相对工作区） | SHA-256 |
| --- | --- |
| `crates/plugin-runtime/src/plugin_services.rs` | `39EC195EAF4CF08376EF8CFDD90A60CA17590C9C2621BAF517E69C100B415804` |
| `crates/plugin-runtime/src/plugin_services/collections.rs` | `1A04CAFDCDF9FAAD75C032D0674E98A3FF38F5B06CDD6075AA5F231942DD0C56` |
| `crates/plugin-runtime/src/manager/language.rs` | `69FC34BB190299CF36B2E63539F195420628BCA46E3AC87167767AB79ABE4A56` |
| `crates/plugin-runtime/src/manager/language/contexts.rs` | `5F194277D1B6EDF02CB42A22BC47D572F0DCC2F62A97C008CCB3B34AAF386040` |
| `crates/editor-app/src/app/language_servers/demands.rs` | `C2B5FACE86791D110EB959D76DA425B1835DA5EC8792B259B8F239268CCBE92D` |
| `crates/editor-app/src/app/language_servers/contexts.rs` | `60C899FEC9694978E48978DBF2503BCEC59714D137C0A8ABBC14B8263656D338` |
| `crates/editor-app/src/extensions/worker.rs` | `2F91B23DF3D4C2281D457B70D0EC11586A480005786967566644E93DDE61010D` |
| `crates/editor-app/src/extensions/worker/demands.rs` | `5241862607449A778047EF45B54C1E908188A98B397FA0CDC81D4B44B2CC962C` |
| `crates/editor-app/src/extensions/worker/worker_tests.rs` | `14553B186DAF26D4ACA78841723CC8D0B9BD525F8CC79430496178C59219C42E` |
| `crates/editor-app/src/extensions/worker/worker_tests/language_failures.rs` | `00123F933BFE6BA7A683405D135354E49591D34CCE676B4F9F794EC9CD35FF15` |

完整四冲突回归 `roles/scenarios.rs` 仍为 `085C3F1E…BE508`。修正版所有相关文件的定向 rustfmt 与 scoped diff check exit0，只有既有 CRLF 提示；执行代理未运行 Cargo 或测试，等待统一调度。

## 异步准入回归的前置失败与实际 RED

以下由根代理统一构建、执行；保留旧日志，不把未进入目标行为的失败计作 RED。

| 轮次 | 实际终态与原因 | 是否进入目标行为 |
| --- | --- | --- |
| 首次 no-run，session 55818 | 编译失败 E0433：新测试缺少 `std::cell::RefCell` 导入；随后仅补测试导入。 | 否；这是夹具编译错误。 |
| 补导入后的首次实际运行，冻结 `E22EDC…` | 两用例在 `Package::from_files` 拒绝：测试清单的 `on_demand:true` 缺 installation plan。随后去掉不必要的该字段，保留实际语言 demand 才解析缺失工具的行为。 | 否；合法包尚未建立。 |
| 第三 no-run PASS 33.11s 后，冻结 `EF3F2503…79DFDE97` | `target/s6-admission-retry-red-valid-package.log` 两用例均于 0.29s 前置失败：首个真实 source 应排队一次 Demand，实际为0。 | 否；未发出待检验的 Demand。 |

最后一项的来源是测试运行目录不一致：Manager 使用临时插件 store，而 `EditorApp::new` 采用默认 profile 的 provider root；`publish_manager` 只发布 entries / availability / ready，不会在另一目录生成包声明和资源。最终夹具使用私有 `SessionState::load_in_profile` 与 `EditorApp::new_with_session`，Manager store 同为 `extensions::profile_runtime_root(profile)`，workspace 仍是独立项目目录。返回前检查实际 trust、runtime/provider store 一致、未知语言识别、已选 provider、context 能力和 Availability，不手设 providers 或 `ContextRecord`。

最终测试源码 `crates/editor-app/src/extensions/worker/worker_tests/language_failures.rs` SHA-256 为 `45E2E4A9BE8974A803B7229BB47199607F926ED0973FDA6256ED649D48BF0D3C`，取代上表 `00123F…FF15` 的中间夹具输入。根代理用只保留旧 receiver 判定的冻结程序（SHA-256 `9DD2EC0479C6ED9496B854D70DB7436134AAF0F45F014956E1A1E824811283F8`）执行两项，日志 `target/s6-admission-retry-red-aligned-profile.log`：**1 PASS / 1 FAIL，1.15s，0 ignored**。

- `asynchronous_language_admission_rejection_does_not_requeue_each_frame` 明确失败于 `admission rejection was requeued`，证明未 reservation 的异步准入拒绝被错误当作可撤回的准备失败，后续 frame 重发 Demand；这是本轮首次目标行为 RED。
- `withdrawn_prepared_language_error_requeues_once_even_before_ui_observation` 同次实际 PASS，保留准备错误先撤回、receiver 后送达仍能重入的正向契约；不以修复负例为由去掉晚到结果保障。

该日志 SHA-256 `AE5355E20039CD4387F549EAF088C5A236B4CC3BBC828E261780D0F492FF9863`。取得 RED 后，根代理恢复当时用于区分准入的 `record.error.is_some() && admitted` 中间判定，源码 SHA-256 `60C899FEC9694978E48978DBF2503BCEC59714D137C0A8ABBC14B8263656D338`；随后 Spec 复核发现下节的取消分支缺口，根代理只停止本人 session 89804 的进程树，保留中断门禁日志。这一中间版不称最终修复或完整门禁通过。

## 已接受后取消的实际 RED 与最终发布证据修复（未验证）

`begin_demand` 返回 Ok 只证明已接受，不证明曾完成或发布语言准备错误。实际 `RequestUpdate::Cancelled`（含超时）可以令 Manager 移除未完成 reservation，Worker 却返回 Err；中间 admitted 判定因此会在缓存为 None 时反复重开同一源请求。仅改成 `Completed { result: Err }` 仍不严格：丢失 preparation lease、委托 Pending 退休或 deferred 再准入失败，也可能以 Completed Err 返回而从未发布语言错误缓存。

新增第三个普通 GPUI 回归 `extensions::worker::worker_tests::language_failures::cancelled_admitted_language_preparation_does_not_requeue_without_published_error`。它沿合法未知 Package、真实 native source、实际 `Work::Demand` 和 `Demands.dispatch` 确认 Manager 接受原目的；在任何准备 tick 前通过公开 `Manager::release_language_document` 取消原 waiter。该路径返回真实 `RequestUpdate::Cancelled`，保持同一 Availability/source/native guard，公开语言缓存和 binding 均为空。首帧及其后12帧不得重新发出 Demand，同时应保留 Failed 终态；没有直接写 `ContextRecord`、缩短期限或伪造请求状态。

首次运行 `target/s6-cancelled-demand-red.log` 是 **2 PASS / 1 FAIL，1.48s**，第三例在首帧 Failed 状态断言失败，诊断指向该帧已重发。为让结果直接锁定症状，只把首帧队列断言放在 Failed 状态之前，保留 Failed 与后12帧断言，最终测试源码 SHA-256 `C86A134CCF333A3538DF4F08CEEE3605BA2D4C10A625C302864E83C94EDF9527`。

根代理随后冻结 `target/s6-cancelled-demand-explicit-red/deps/editor_app.exe`，SHA-256 `5FF367A1027924D85DDDB3E1E7C4558808F076DD80BC5BECD8511DAB025F5C61`；实际执行 `target/s6-cancelled-demand-explicit-red.log`：**2 PASS / 1 FAIL，1.50s，0 ignored**。第三例明确失败于 `cancelled admitted preparation was requeued without a published error`，异步准入和晚到准备失败两例均 PASS。日志 SHA-256 `81F81A3316BBF58A58D1E34ED5F92C802305D89A3B3BC1484290CF412CB3BAB4`；此为取消分支的明确目标 RED。

根代理取得该 RED 后才授权以下最小生产修正，不扩公开插件 SDK 或协议，也不改变集合 scope、权限或健康计划生命周期：

- `Prepared.failure_publication` 为原 document reservation 分配独立 Arc；`DemandRequest` 在原 provider / DocumentContext / 当前 project 的 reservation 上取得同一只读证据。新 reservation 创建新 Arc，删除重建同 key 不会把新发布借给旧请求；发现后 cache key 改名仍保留原证据。
- 只有 `publish_document_language_lease` 通过原 intent 校验、实际写入终态 Err 缓存后才置真。普通 activation/deferred 错误、Pending Drop、取消、超时及准入失败不置位。缓存撤回不抹除旧请求所持的历史证据，因此“先撤回、结果后送达”仍有可验证来源。
- host-only `DemandRequest::has_published_language_error()` 不给 guest authority。Worker 只在原请求为 Completed Err 且原 reservation 的标记为真时转送；成功 receipt 再准入失败不被冒充为准备错误。`DemandWait::preparation_error_was_published` 和 `ContextRecord.published_failure` 反映真实含义，移除中间 admitted 语义。
- 原生读取到同 provider/document 的实际错误 publication 仍可记录该事实。准入、取消、超时的终态本身不再因缺少 publication 自动重发；既有来源、revision、generation 和 lease 撤销检查保持。

最终新增/更新源码冻结摘要如下；不改写前面的中间实现账本：

| 文件（相对工作区） | SHA-256 |
| --- | --- |
| `crates/plugin-runtime/src/manager/language.rs` | `154239C3CB10595E84C0AECA767E762F096D5FF8C6E55E4521A1C10A95FEC0ED` |
| `crates/plugin-runtime/src/manager/language/contexts.rs` | `5C5E46417C88E8835E04CA70AF567027855E653193BB16556E11F912CDF515BE` |
| `crates/plugin-runtime/src/manager/activation/types.rs` | `8934C1E202170B81D0284DCCE9CB645B76CD90ECEB13BE4418E05D4D415B3AF0` |
| `crates/plugin-runtime/src/manager/activation/driver.rs` | `9C7856B2EA93F69152527AF30AF838E99E5C2955906E51A81E571157D0983B2C` |
| `crates/editor-app/src/extensions/worker.rs` | `DFEBCC339233E68617359D8D7F39F0BB5BB025AE4EABD6F9082BD0BE6BB35117` |
| `crates/editor-app/src/extensions/worker/demands.rs` | `E4E73689B3D2A282F684E3489566EC73C639E30B87D87241706E1A24CE666422` |
| `crates/editor-app/src/app/language_servers/demands.rs` | `5C9CDE2265E0C2446156825D5CA1FB7695497356460A7C85C645FED112D70A93` |
| `crates/editor-app/src/app/language_servers/contexts.rs` | `D0713367380655FE2D82FFC243ADD55427ECA4CE65E388603BD0114CFECF3A5E` |
| `crates/editor-app/src/extensions/worker/worker_tests/language_failures.rs` | `C86A134CCF333A3538DF4F08CEEE3605BA2D4C10A625C302864E83C94EDF9527` |

这次定向 `rustfmt --check --edition 2024 --config skip_children=true` 与 scoped `git diff --check` 均 exit0，仅有既有 CRLF 提示。执行代理没有启动 Cargo、测试或 Vtarget 写入；根代理负责后续三项小回归 GREEN、统一门禁、两轴审查和完整四冲突实际复验。当前只确认修正已冻结，不宣称行为已通过。

## 发布证据小回归 GREEN 与完整冲突第四轮前置失败

根代理 no-run 构建 PASS 3m28s，冻结 `target/s6-publication-provenance-green/deps/editor_app.exe`，SHA-256 `918351A79C4CBF0055D283B2C68C717747DF49925FEEEF6AFB95D74BFCEAD5A1`。三项普通 GPUI 小回归实际 **3/3 PASS，1.66s，0 ignored**；日志 `target/s6-publication-provenance-green.log` SHA-256 `F2C998A7F295DEAB462BA34D284F14E24CB708FAAD8E7B37190F8DA7939A3FB2`。异步未准入、已接受后取消两项均不重发，真实准备失败在先观察/先撤回两序下仍能重入；该结果不代替 runtime 实际集合变化和生态完整验收。根代理报告两轴增量复核各0新问题。

随后仅执行完整四冲突入口，不并行 Cargo、Node 后端测试或其他 GUI：

```powershell
& target/s6-publication-provenance-green/deps/editor_app.exe extensions::worker::vue_ecosystem_tests::roles::scenarios::conflicting_and_irreversible_package_contributions_reject_and_recover --ignored --exact --nocapture --test-threads=1
```

输入是上表固定 TS .14、Vue .11、combined .2 和四种 .2 负例，执行前读回摘要一致。特别核对 `NANOBUG_ECOSYSTEM_BAD_MAPPING_PACKAGE` 指向 SDK4af 的 `invalid-mapping/unfamiliar-invalid-mapping-0.1.2.zip`（SHA-256 `03B9DC…49AD7`）；只读 ZIP 中的 `contributions.json` 确认其 id 为 `unknown-invalid-source-map` 且 `invalidMapping=true`，与 combined 的 `unknown-source-check` 不重复。这是旧 unique .1 变体的新 SDK .2 产物，未误用重复贡献 ID 的历史包。

session 7075 已退出101，日志 `target/s4-conflicts-native-provenance-green.log`：**FAIL 127.22s，0 passed / 1 failed，0 ignored**。初始 `wait_effects(...,true)` 的四角色诊断仍为 `Some([])`，随后严格 `assert_preparation` 在 `vue_ecosystem_tests.rs:218` 报 `context:target/s4-roles-QHJVDG/App.vue: TimedOut: The plugin did not become available in time.`；未进入任何冲突包安装、显式拒绝或卸载后恢复。因此这是初始真实准备的前置失败，不计为集合缓存修复的目标 RED 或 GREEN，也不外推为任一负例未按要求拒绝。

日志 SHA-256 `BD07692442DFAF3350001084DF0FE8A4F46E915F80C28157603E49ED69D86E7B`。终态后读回 exe、runtime language/contexts、原生 contexts 和 `roles/scenarios.rs` 摘要仍与冻结输入一致，未编辑源码、调预算或再次启动测试。测试 panic 前尚未调用结尾 `shutdown`，所以本轮没有 checked shutdown 或全部 plan/process0 的通过证据；执行进程已物理退出，只读进程清单未发现匹配本轮程序或插件后端命令的残留，但不以此冒充 checked cleanup。下一轮需先解决初始准备路径，再沿同一完整入口实际验完四类拒绝与恢复。

## 完整四冲突第五轮 GREEN

根代理完成独立 Native S5 全链及清理后，交付冻结 `target/s6-native-normal-frame/deps/editor_app.exe`，SHA-256 `706332EBA442BF2CCB266FBE23B33B7F49C84D2CB43BE487A25E0C2EBCABB399`。相对上轮 `918351A7…EAD5A1`，该程序只纳入 Native S5 测试自身的真实 render frame 修正；本 S4 场景、最终失败发布证据生产实现、原180秒阶段期限和全部严格断言均未改变。此事实不反向确定上轮初始超时的原因，也不抹去该 FAIL。

独占串行运行相同完整过滤词及 `--ignored --exact --nocapture --test-threads=1`，session 78401：**exit0，1/1 PASS 280.28s，0 ignored**。日志 `target/s4-conflicts-native-after-native-cleanup.log` SHA-256 `670EBF654E48A2EBF410A2F3E71D80FB423975B91D1CF0B55D36301CC96C61EB`。执行前后 TS .14、Vue .11、combined .2 和四个负例 .2 的七个摘要均与固定输入表一致；不可逆映射仍为已核实 unique id 的 SDK4af .2 包。

| 真实安装负例 | 实际明确拒绝 | 卸载发布边界 | 后续必须执行且已通过 |
| --- | --- | --- | --- |
| exclusive-conflict | `Exclusive contribution slot conflict: template-compiler`（-32005） | `14→18` | 原始四角色诊断及 count F12 恢复 |
| ordering-cycle | `Contribution order contains a cycle`（-32005） | `34→38` | 原始四角色诊断及 count F12 恢复 |
| configuration-conflict | `Contribution configuration conflict: checkUnknownComponents`（-32002） | `52→56` | 原始四角色诊断及 count F12 恢复 |
| invalid-mapping | `Contributed source mapping does not preserve its source block`（-32002） | `70→74` | 原始四角色诊断及 count F12 恢复 |

每次拒绝后的内存 SOURCE 原样、`state.ts` 磁盘原文不变及无可确认编辑前缀断言均通过。每次卸载采用后，Vue Availability 仍为同一 manager2 / generation3 / configuration3，native/catalog generation均为2；当前 Vue context 从终态失败重新进入 Loading，并在原恢复预算内取得真实 role effects 与原文定义结果。因此本次实际覆盖了先前 162.39s RED 的同 intent 集合变更恢复路径，而非通过改源、close/reopen 或全插件配置 generation 推进绕开旧缓存。

结尾 `shutdown` 已实际发送 `Work::Shutdown` 并取得无错误的受管关闭确认，完整用例才返回 PASS。测试 exe PID52048 已物理退出；只读样本未发现该 exe、隔离 profile `.tmppzIizH` 或本轮 `s4-roles-tTU4yA` 对应 Node/backend 残留。没有调用内部 Undo、调期限、重写断言或并行启动 Cargo/其他后端测试。执行后 `roles/scenarios.rs` 仍为 `085C3F1E…BE508`，runtime language/contexts 及原生 contexts 仍分别为上述 `154239C3…EC0ED` / `5C5E4641…15BE` / `D0713367…F3A5E` 的最终生产修复输入。

每个完整入口保存退出码、总时长和阶段，不把旧冻结程序当成新修正验证。核对固定包摘要，保留产品语义、文本相等、预算、真实进程退休和 checked cleanup 断言；不绕过测试调度检查，不清除旧回执，不使用内部 Undo 或手动 tsserver 预热替代。
