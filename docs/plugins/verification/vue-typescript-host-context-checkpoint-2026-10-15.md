# 宿主上下文分支暂停记录

用户要求暂停后停止实现与测试；本记录只保存续接事实，不代表工单验收完成。

## 工作位置与归属

- 工作树：`C:/Users/Tmiracle/.codex/worktrees/vue-typescript/Editor`，分支 `codex/vue-typescript`，基线 `1379f58`。
- 原始 `C:/Projects/RustProjects/Editor` 未由本子代理修改。集成工作树中其他代理的更改必须保留。
- 本子代理未提交、推送或关闭议题。
- 本子代理启动的命令均已退出；没有仍运行的自有测试或语言服务进程。最后的 PTY session 为 `20580`，已成功退出。

## 已有可继续使用的行为

1. 通用 `LanguageDocument` 按文档需求、opaque 项目上下文、按上下文原生服务租约与委托 binding；同一个服务 Arc 的借用别名不会重复退休，最后需求消失才回收。
2. 实际文档 languageId 与多语言别名；公开 control 方法通过现有 stdio LSP 连接执行，source 使用普通 LSP 文本通道。模块 metadata、绑定、原始调用者 authority 来自 runtime。
3. 新公开 `language_context` 固定图协议和 `Provider.related_documents`。宿主按 provider 返回的成员集合同步打开文档，未选中候选只提供 metadata，不发送文本。
4. `navigation::ContextCapture` 固定点同步 target 和辅助文档，分析前后检查图 generation 与每份原生 wire version，结果携带完整原生 DocumentVersion 向量。
5. 标准 feature、definition、hover、diagnostics、formatting 生产入口已接上下文向量。hover 正结果缓存与已发表 semantic markers 也记录相关文档 seal。
6. `LanguageFixOrigin::GuardedLsp` 与 `EditorAuthority.with_guard` 联合保存配置/磁盘事件 epoch；guard 进入原生 request 与 WorkspaceBatch，不退休服务。06 代理已接 rename 的 capture.fix_origin，并在非空 workspace file events 撤销 epoch。

## 实际验证

- `cargo check -p editor-app --tests -j2` 在接入 definition/hover/初版 diagnostics/formatter/guard 时通过，session `20580`（现有警告 68 条）。**此后还有未编译的修改，不能把本次结果视为最终工作树通过。**
- 显式运行 ignored 真实服务回归 `related_native_sources_are_explicit_and_auxiliary_changes_revoke_results`：先观察无 graph handler 的 RED（null 响应被 InvalidRequest 拒绝），随后构建 fixture handler，GREEN 1/1，0.08 秒。观察仅 target/related 两份 didOpen，没有 unrelated 文本；只改辅助 wire revision 使目标请求 StaleRevision。
- `language_contexts` 常规 5 项通过；新增固定公开 graph schema 的 Package 回归通过。
- earlier ignored 真实 SDK 文档上下文 Manager 回归通过，99.39 秒。
- earlier GPUI `installed_document_contexts_route_native_tabs_without_cross_project_text` 显式通过，54.89 秒；两个未知语言子项目隔离 root/脏文本，Base 实际跳转钩子可用。
- earlier ignored `native_control_and_document_aliases_share_one_connection` 显式通过，0.15 秒；70KB source、真实 languageId、控制 marker、取消、业务拒绝不重启连接。
- 最终 workspace fmt/test/check、真实 TS/Vue mixed 包测试不在此子代理宣称完成范围。

## 最新未验证 WIP 与下一步

1. **已知当前编译问题**：`editor/diagnostics/tests.rs` 两处 `apply_semantic_diagnostics` 调用（约 237、255 行）缺新增 `context` 参数，应传 `None`。06 代理最后的 `cargo test ... --no-run -j2` session `77488` 因此退出；暂停后未修。
2. 刚接 `Request.context_snapshot` 的 control opt-in：默认 preparation 控制仅同步 target，analysis 请求可要求 graph 固定点。`extensions/editor_requests/language_control.rs` 最新改动尚未 check。父代理负责 protocol Request 字段、SourceMetadata 及所有 guest literal 迁移；不要重复改其文件。
3. `ContextCapture.wire_sources()` 在 operation 前记录 `{document,uri,version}`，控制 wrapper 注入 `_nanobug.sources`；`control_request_at_version` 在同 connection lock 将 target 原生版本配对实际 wire 版本。控制 sources 上限 128，整个 native 控制 wrapper 上限 64KiB。应补真实 wire metadata 回归并核对父代理新 `SourceMetadata` 类型，当前本地保留 Value 元数据。
4. 上述 control wrapper 在借用 context 的 closure 中移动 parameters/native_source；需要后续 cargo check 核对所有权与新增字段编译。仅有期望方案，尚无最新通过证据。
5. 新 semantic_context 剪枝、check_epoch 与 latest control 改动未验证；应修已知 tests 参数后精确 check，再测对应 native/GPUI 请求行为。
6. GuardedLsp 配置/文件变化应增加公开 Package→Manager 回归，验证已发 request 和 batch authority 撤销，而服务/其他项目仍活跃；目前尚未添加此专项回归。
7. 继续与 06 代理协调：其拥有 completion、rename、workspace edits 与 Plan.conditions，不要覆盖；`ContextCapture.selected_sources()` 与 `documents()` 已提供只读快照。其 Plan.conditions 负责预览期间辅助内存 revision 检查，epoch 负责磁盘/配置变化。
8. DefinitionProvider 新增代码使 navigation.rs 文件继续增长；后续可按职责纯迁移 provider 至子模块，避免和 06 的 connection capability 字段改动冲突。
9. 最终需要重建 host exe，随后外部 TS/Vue 仓库使用新的内嵌 SDK 构建包；目前 host exe 不含最新 related context/source metadata SDK。

## 主要文件

- 公开协议：`crates/plugin-protocol/src/language.rs`、`language_context.rs`。
- runtime：`manager/language.rs`、`manager/language/{contexts,delegation}.rs`、相关 activation；guard 为 `direct.rs`、`manager/direct.rs`、`language_service.rs`、`instance/workspace_files/native.rs` 小范围变更。
- app：`app/language_servers/{contexts,demands,snapshots}.rs`，`language/navigation/{context,context_tests,control}.rs`，`language/navigation.rs`、`language/hover.rs`，`extensions/editor_requests/language_control.rs`，`editor/diagnostics.rs`、`editor/diagnostics/publications.rs`、`editor/text_session.rs`、`editor/language_features/requests.rs`、`editor/language_edits.rs`、`editor/documents.rs`。
- 真实夹具：`plugin-runtime/examples/lsp_fixture.rs`，`plugin-runtime/tests/fixtures/project-context/`；父代理维护 language-delegate fixture。

