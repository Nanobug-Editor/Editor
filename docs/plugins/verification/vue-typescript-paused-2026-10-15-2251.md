# Vue / TypeScript 新工单暂停检查点

时间：2026-10-15 22:51（Asia/Shanghai）。状态：收到“暂停所有工单并保留进度”后停止后续实施与测试启动。代码、未提交文件、候选包与日志均保留；没有提交、推送、关闭工单或发布版本。

## 执行边界

恢复时只执行新 [#121–#126](../tickets/vue-typescript/README.md)。旧 #108–#118 保留历史进度，不再推进、改写或关闭；父 #107 保持不变。原批准的六场景与 43 项验收要求没有删减。暂停期间不继续自动实施。

| 新工单 | 暂停前阶段 | 已验证与待验证的区别 |
| --- | --- | --- |
| S1 / #121 | 定向验证 | 控制租约回归 GREEN；冷启动取消竞态的普通公开回归 GREEN；真实混合项目尚未复测通过 |
| S2 / #122 | 定向验证 | 真实 TS 原生重命名/预览/取消/冲突/分组 Undo/自动导入 GREEN；Vue 跨组件事务、混合共享源待验证 |
| S3 / #123 | 定向验证 | Vue source/sealed 各 84、原生 grammar 2 项 GREEN；可见 UI 深主题/IME/滚动/缩放未完整通过 |
| S4 / #124 | 实现与验证 | 真实角色、集合、工具层映射 GREEN；实际三包 production actor 全链及组合事务待验证 |
| S5 / #125 | 实现与验证 | 依赖准备、实际 WASM 故障、迁移/回滚与真实热更新 GREEN；实际 Vue 后端退出/恢复、组合更新与撤权待验证 |
| S6 / #126 | 待联合验收 | 最终整库检查、双轴审查、提交、推送与新工单关闭均未完成 |

## 保存位置

- 宿主隔离 worktree：`C:/Users/Tmiracle/.codex/worktrees/vue-typescript/Editor`，分支 `codex/vue-typescript`，HEAD `1379f58df76622bdf4d6a7c4167469f124d17a0b`。本轮新增文件及改动尚未提交。
- TypeScript 独立源码：`C:/Projects/RustProjects/TypeScript-for-Nanobug`，origin `https://github.com/Nanobug-Editor/TypeScript.git`。
- Vue 独立源码：`C:/Projects/RustProjects/Vue-for-Nanobug`，origin `https://github.com/Nanobug-Editor/Vue.git`。
- 两个公开远程仓库已经创建并核对身份；本轮尚未推送源码。原始 `C:/Projects/RustProjects/Editor` 中其他任务的改动没有混入本 worktree。

## 冻结产物

| 产物 | SHA-256 |
| --- | --- |
| TS `dist/typescript-0.1.8.zip` | `6265d3db2a19798e803fb700ccf9b2d9abd6fe00cd355f1179252847787f8229` |
| Vue `dist/vue-0.1.8.zip`，4,090,408 bytes | `92c9dc9c14f82138a0ac22589e1b9566ddcde2c2fa85a66baf2d0ed9a5d5ce24` |
| 宿主 `target/s1-fixtures/capability-example-0.19.1.zip`，当前 SDK 独立构建 | `d9c4a4b8933b7372fde0115cd678e04fb85b3f034a5ed6e54a9510e22b954b68` |
| TS `test/fixtures/project-consumer/dist/unknown-project-consumer-0.1.0.zip` | `647facdfe18834e5a3688ea0d683df992a7ae2bc6c2e13243694b0e5a5748ca0` |

旧候选 ZIP 和原固定夹具未覆盖。源码版本、协议版本与能力版本分别管理，尚无正式支持或 Release 声明。

## 本轮关键结果

1. `sequential_native_controls_keep_the_open_source_transport`：真实未知 SDK 包发出两个连续控制请求，RED 观察到 2 次 `initialize`；保存成功控制的 source incarnation、配置及独立 sealed caller chains 后 GREEN 1/1（75.58s 含冷 SDK/JIT；实际两次调用 423.08ms），只初始化 1 次。日志 `target/s1-control-lifetime-{red,green}.log`。新 `control_sources` 随文档关闭、来源身份变化、调用授权退休或撤信任回收；完成任务本身不再回收后续调用所依赖的上下文。
2. 同 TS .7 / Vue .7 原 mixed 复测仍 FAIL 46.01s：自然打开后输入使旧准备取消，新需求遭 `Abandoned activation is still retiring`，尚未进入控制链。日志 `target/s1-mixed-control-lease-green.log` 虽含 green 字样，实际不是通过。新增 `activation/deferred.rs` 保留有效需求原完成门/截止时间，等待旧 flight 退役；普通公开 Manager 回归已经 RED→GREEN 1/1（0.03s），真实 mixed 待新 bin。
3. `real_typescript_rename_opens_unknown_targets_previews_cancels_and_group_undo`：冻结 TS .8 原生完整测试 GREEN 1/1（43.49s），实际包含自动导入选择与 resolve、原子编辑和一次 Undo；前半段只读整包拒绝、辅助条件变化撤销预览、未知目标重算、预览取消、应用不保存、分组 Undo 均通过。日志 `target/s2-typescript-editing-018-test.log`。
4. TS .8 真实 Node 完整测试 35/35、无跳过；类型检查及插件格式检查通过。Vue .8 source/sealed 各 84/84；真实 Vue .8 + TS .7 原生 grammar 两项通过。候选已通过公开宿主内嵌 SDK 构建，不代表最终宿主验收通过。
5. S5 实际热更新回归修复后 1/1（215.69s）：恢复 Main 后更新对应冷记录的 materialized 状态，避免后续需求重复激活并撤销新 plan。数据迁移/激活失败回滚分别通过。细节见 [S5 记录](vue-typescript-s5-runtime-2026-10-15.md)。
6. UI 日志在 Vue `target/native-editor-018.log` 与 `target/native-editor-018-second.log`：前者的多字符输入一次 Undo 预期已修为单字符；后者 `$tone` 显式颜色断言失败。锁定上游 Default Dark 的 variable 没有显式 color，尚须用真实 capture 与主题解析结果确认，而非把默认前景色当作没有高亮。最新 resolver 透明观测已保存，未执行；没有修改生产高亮。

## 恢复时的第一处断点

最后命令 `cargo test -p editor-app --no-run -j 2` 已自然退出 101；日志 `target/s1-mixed-recovery-build.log`。准确错误为 `crates/editor-app/src/extensions/worker/typescript_package_tests.rs:149` 无法找到 `anyhow!` 宏。暂停后只记录，没有继续修复或构建。

最后成功的 app 测试程序是 `target/debug/deps/editor_app-d4e1859b30e4cb6a.exe`（2026-10-15 22:41:39），含控制租约修复，**不包含**之后 cold-race / 最新 UI 观测 / 新增测试。不能用旧 exe 给新源码报通过。稳定 SDK 宿主是 `target/debug/editor-app.exe`，当前候选使用其内嵌 SDK `06f3a760...`。

新增但未完成编译/执行的测试：

- `extensions/worker/typescript_package_tests.rs`：配置 TS / 推断 JS 七项能力与 Vue 禁用、未知 SDK 消费者五次连续公开调用。
- `extensions/worker/vue_editing_tests.rs`：Vue 3 跨组件 prop 重命名、未知父文件正常打开、预览/取消/不保存/分组 Undo；先验证单切片，再补历史组合。
- `tests/language_editing/definition_offsets.rs`：真实 Base 定义 hook 的 UTF-8 byte offset 与 LSP UTF-16 位置。当前 production `LanguageDefinitionProvider` 的 character conversion 仍待真实 RED 验证，未提前修改。
- `extensions/worker/vue_recovery_tests.rs`：真实后端计划停止、另一版本继续、有限恢复及撤信任清理。
- `language/plugins/vue_editor_tests.rs`：最新主题/capture/range 观测与已修单字符 Undo / 独立 IME 预期。

恢复建议顺序：先修新测试编译断点并重建 app 测试程序；运行快速字节 offset 回归；再验证原 mixed 的 cold-race 和跨调用生命周期，随后 TS-only / unknown consumer、Vue 事务、三包生态、实际恢复与 UI。保留单一 GPUI 时段及原生 Cargo 并发限制，依据实际新失败继续垂直修复，不跳过验收。

## 未完成的交付门禁

第一次 `cargo fmt --check` 仍有本任务文件差异，最终格式检查未通过；最终 `cargo test --workspace --exclude editor-app`、`cargo check --workspace`、全部适用 ignored 用例和双轴 review 尚未执行完成。只关闭全部验收通过的新工单，先普通提交并推送 origin，再读回远程与 issue；不碰旧工单、不创建 Release。

暂停保存本身不触发下一次执行；等待用户恢复指令。

全部代理已确认停止且各自 Cargo/test sessions 已退出。最后 `language_contexts` 普通套件 10 passed、2 ignored、0 failed（0.12s），覆盖最新 deferred/materialized 源码；没有把两项 ignored 报为通过。更细的资源与回滚断点见 [S1/S5 续接记录](vue-typescript-s1-s5-checkpoint-2026-10-15.md)。宿主最后 no-run 的编译失败保持原样，不在暂停后修复。
