# Vue 与 TypeScript 暂停检查点

保存时间：2026-10-15 18:59（Asia/Shanghai）。状态：用户明确要求暂停；所有代理已停止新增实现和测试，已确认本次测试及分析进程没有残留。没有创建自动续跑任务。

所有源文件与候选产物留在磁盘；没有 stash、reset、删除 worktree、提交、推送、关闭议题或发布版本。最终检查尚未完成，当前是允许存在编译断点的 WIP，不标记为功能完成。

## 工作目录与授权

| 工作面 | 路径 / 状态 |
| --- | --- |
| 宿主隔离 worktree | `C:/Users/Tmiracle/.codex/worktrees/vue-typescript/Editor`，分支 `codex/vue-typescript`，基线 `1379f58df76622bdf4d6a7c4167469f124d17a0b`；所有本任务宿主编辑在这里 |
| 原始用户工作区 | `C:/Projects/RustProjects/Editor`；有大量其他任务改动，不得当成本任务成果或一并提交 |
| TypeScript 独立插件 | `C:/Projects/RustProjects/TypeScript-for-Nanobug`；源码正升至 0.1.1，生成清单/服务 ZIP 尚未刷新；部分文件只是 intent-to-add，并无提交 |
| Vue 独立插件 | `C:/Projects/RustProjects/Vue-for-Nanobug`；源码 0.1.4 WIP，未生成该版本 ZIP；Cargo.lock 和文档账本待刷新 |

[方案](../specs/vue-multi-backend.md)及[11 张工单](../tickets/vue-typescript/README.md)已经用户确认并获连续实施、并行与子代理授权。父议题 #107、子议题 #108–#118 已创建，实际 ID 和阻塞关系在 [publication.json](../tickets/vue-typescript/publication.json)。所有议题仍开放。用户暂停优先于此前实施授权；明确恢复后才继续，不要自行启动后台工作。

## 已保存实现

- 通用 `language.lsp 1.5` 文档/项目上下文、多语言别名、委托提供者；同工作区不同项目各有实际 native plan，借用关系不复制进程所有权。
- `language.control 1.0` 复用既有 LSP 通道，冻结原始未保存文本，控制参数/结果保持 64 KiB；合法业务错误与取消不破坏共享连接。
- `language.modules 1.0`、`native.modules` 安装权限、完整闭包与摘要验证、精确 exporter/consumer/scope/document 校验、一次性 native binding。相同代码组有多个独立消费者时分别保留授权。
- TypeScript 项目/分析/适配契约与 Node/TS 受管进程；Vue legacy/modern 适配资源、版本证据选择、虚拟源映射和完整 SFC 格式化。固定候选组合为 Vue 2.6.14、2.7.16、3.5.22 与 TS 5.9.3，未外推全版本支持。
- 关联文档 membership、完整 revision 向量、图代次与 `ContextCapture`；宿主只同步提供者明确选择的成员，相关结果和预览有失效 guard。
- 跨文件 rename 发现未打开目标后，创建正常 DocumentSession，再重新分析、全量预览和原子应用；不自动保存磁盘。补全选择 resolve、自动导入与主编辑同次原子应用。
- `workspace.files 1.4` 的 `Plan.conditions` 只读输入向量；预览/提交检查源及辅助文档，不把这些条件加入 Undo 目标。纯文档批次同一 UI turn 完整预检后应用。
- 原始 native consumer 退休通知已写入宿主与 TS native：`nanobug/controlConsumersRetired`；精确调用链计数、批通知与 TS 引用清理。最新宿主部分尚未编译/端到端验证。

宿主代理的细节另见[上下文检查点](vue-typescript-host-context-checkpoint-2026-10-15.md)。阶段证据见[持续实施记录](vue-typescript-implementation-2026-10-15.md)。这些记录不能代替最终工作树验收。

## 恢复时的首要断点

1. `crates/editor-app/src/editor/diagnostics/tests.rs` 两处 `apply_semantic_diagnostics` 调用（暂停时约 237、255 行）缺新增 context 参数；已知会编译失败，遵用户暂停未修。
2. `extensions/editor_requests/language_control.rs` 已使用 `control.context_snapshot`，但 root 尚未给 `plugin-protocol/src/language_control.rs::Request` 添加该默认 false 字段。须同步全部 Request literals、SDK 导出及 TS WASM facade：仅语义分析 opt in 完整 context snapshot；resolve/attach 使用单一 target，防首次 attach 改变项目图后使自身准备失效。
3. 宿主 `ContextCapture.wire_sources()` 与控制器已开始注入 `_nanobug.sources`，尚未编译。root 尚未添加公开 `SourceMetadata { document:DocumentVersion, uri:String, version:i32 }` 类型；wire version 始于 1，与原生 revision 不同，不能要求 WASM 消费者猜测。
4. TS `native/control.mjs::analyze` 仍要求 `{uri,version}` 内部 wire 向量。须改为优先使用 host-sealed sources 校验所有实际 overlay，公开消费者可提交原生 DocumentVersion 前置条件，结果回传原生向量。真正 A/B 公开消费者联合验收依赖此项，不能只用猜测版本 1 的测试证明契约可用。
5. root 新增 `language_service/consumers.rs`、`register_language_consumer`、管道退休批通知和模块方法拆分尚未最终 check/rustfmt。TS native 的退休回归 1/1 已通过，但 native A/B 退出、共享模块和宿主通知整链待验证。
6. `extensions/worker/vue_typescript_tests.rs` 的真实 production Worker 联验首轮失败：300 秒内没有语言状态，未进入 adapter 阶段；后加显式 `panel.poll` 与详细诊断，未重跑。须确认测试 publication/语言识别路径，不把它报告为包联验已通过。
7. TS `build.mjs` 已增加 related-documents 固定 schema、`related_documents` 与 0.1.1，但还需把 `workspace.files` 最低版本改为 `>=1.4,<2`，刷新服务归档及 SHA、manifest、Cargo.lock、SDK guest 与 ZIP。Vue `package.cjs` 也待更新最低 API 声明。

恢复后先核对这些 WIP 断点，再编译嵌入新 SDK 的宿主，按公开 `--plugin-cargo` / `--plugin-package` 重建消费者。不要用旧宿主包构建器误判新增字段或能力；不要安装缺失工具链，也不要调用已退役 scripts。

## 最近实际验证

| 验证 | 结果与范围 |
| --- | --- |
| Runtime document contexts | 5 项普通测试通过，真实 SDK hook ignored 测试单独显式通过；非最终状态全量检查 |
| Runtime immutable module 委托 | 最新公开 Package → Manager → SDK guest → native binding 测试 1/1 通过，126.63 秒；包含错误项目 plan 拒绝、source 撤销与无关项目保留 |
| Native control | 首次同步 70 KiB 源、真实 per-document language ID、业务错误与取消保持一次 initialize；定向测试通过 |
| 未知语言双项目 GPUI | 真实 Base definition、未保存文本与分项目 root，1/1 通过，54.89 秒 |
| 关联文档 native fixture | 先真实 RED，再加入 graph handler 后 GREEN：1/1，0.08 秒；只同步显式 related，辅助版本改变使结果失效 |
| 真实 TypeScript rename UI | 旧状态 1/1 通过，约 56 秒：未知目标打开后重算、取消、双文档提交、磁盘不变和 group Undo |
| Runtime 跨文档计划及条件 | `native_cross_document_plans_require_workspace_grant_and_reject_invalid_suffix` 1/1 通过；日志 `target/native-cross-test.log` |
| 新增 UI 回归 | 自动导入、只读目标整包拒绝、预览期只读辅助输入变更撤销等尚未运行；最近 no-run 因 diagnostics 测试缺参数失败，日志 `target/rename-build.log` |
| TS native graph | 1/1 通过：共享 import 纳入、外项目排除、分页代次稳定、未保存 import 改图 |
| TS native consumer retirement | 1/1 通过：旧引用失效、另一 caller 保留、超过引用限额的累计临时 caller 不泄漏 |
| TS → Vue 强闭包加载 | Vue 0.1.2 曾 4 项失败；改 0.1.3 后 `test/adapter.test.mjs` 5/5 通过；未对所有最新源码重跑最终全集 |

完整工作树 `cargo fmt --check`、`cargo test --workspace --exclude editor-app`、`cargo check --workspace` 和最终 editor-app/ignored 矩阵未完成。Cargo 使用 `-j 2` 或更低；此前未限并发曾出现 Windows pagefile 1455 与 rustc ICE，未修改系统分页设置。

## Vue 0.1.4 WIP 与产物

- 最近通过受管 loader 的包：`C:/Projects/RustProjects/Vue-for-Nanobug/dist/vue-0.1.3.zip`（9,440,234 bytes）。0.1.0、0.1.1、0.1.2 均保留，0.1.2 的 UMD 动态依赖缺陷不能算验收通过。0.1.3 改 ESM 打包入口；强闭包 loader 的 32 项与 mixed markup 4 项通过。
- 0.1.4 新增 `adapters/styles.cjs`、Sass 纯库及格式器、`legacy/external-js.cjs` 和 JSX/TSX/SCSS/Sass/Less 注入。源测试 styles+markup+format 19/19、外部 src JS/TS 六例 6/6、grammar query 一项通过。最后 sealed 全套只跑到部分用例即因用户暂停终止，日志 `target/sealed-014-tests.log`，不可报全绿。
- Sass 输入固定 upstream commit `0517ae13f9efc718259ae582dc3ac49b4823d46d`，tar SHA256 `6d9447a26c5e07a2fda1cca6b31595d41021751f6e856783a1accf63b9bfb8e7`；`vendor/sass/` 包含许可及 provenance，`target/sass-source/` 保留构建输入。依赖版本与 ignore-scripts 已固定；文档与复现说明待同步。
- 外部 Vue2 JavaScript 修复覆盖静态 `Vue.extend` 默认导出及缺省泛型；任意别名、动态导出、纯对象导出不能泛称已覆盖。SFC 格式化不会读取/覆写外部源文件。
- Tree-sitter WASM ABI14、来源 commit/许可与 web-tree-sitter 加载已核对；**宿主 tree-sitter 0.26.13 的实际 grammar loader 尚待验证**。缺失注入 grammar 不应把 Sass/Less 冒充 CSS。
- 独立消费者夹具 A/B：`tests/consumers/dist/ecosystem-consumer-{a,b}-0.1.0.zip`，用 public `run` command 发起直接 TS resolve 和 Vue analysis 三跳，结果通过真实 Snapshot 观察；构建不等于生命周期验收。说明见其 `tests/consumers/README.md`。

TS 最近 ZIP 仍为 `C:/Projects/RustProjects/TypeScript-for-Nanobug/dist/typescript-0.1.0.zip`（40,547,464 bytes），不包含之后新增 graph、sources 或退休逻辑。0.1.1 尚无 ZIP，源码/生成清单版本暂不同，继续前必须刷新。

## 后续尚未完成

最新 SDK/native source 向量接通；两个实际插件包从公开安装到原生 UI 的混合项目验收；全部标准特性及语法/样式矩阵；未知生态插件的实际转换/配置/类型贡献（A/B 纯消费夹具尚不能替代）；更新不兼容、坏摘要、离线、撤权和上下文故障恢复；两轴审查、最终检查、普通提交、推送与逐项 issue 状态读回。历史更早版本和未来版本仍按已批准长期方案逐步验收，不冒称已支持。
