# Vue 与 TypeScript 实施记录

日期：2026-10-15。状态：实施与联合验收中，未发布版本，未关闭实施议题。

用户现已要求暂停；最新源码、验证边界与恢复断点见[暂停检查点](vue-typescript-paused-2026-10-15.md)。下文是阶段证据，不表示最终工作树已通过检查。

依据：[已确认方案](../specs/vue-multi-backend.md)、[工单与真实议题映射](../tickets/vue-typescript/README.md)。原始工作区存在其他任务的未提交修改；本次从 `1379f58df76622bdf4d6a7c4167469f124d17a0b` 创建 `codex/vue-typescript` 隔离 worktree，宿主改动只在该 worktree。两个官方插件在独立源码目录构建，不加入宿主 workspace，不随宿主发行或 SDK 导出。

## 公开接缝

- `language.lsp 1.5`：文档上下文、插件拥有的项目身份与代次、多语言别名、委托提供者。一个项目缓存拥有实际 native plan；文档及委托者借用该 plan，不复制所有权。最后借用释放、配置换代或原实例失效后清理。
- `language.control 1.0`：插件只能请求自己的提供者声明的方法；参数与结果有界，走现有受管 LSP 连接。宿主冻结原生源文本后通过普通 LSP 同步，源文本不塞入 64 KiB 控制信封。合法服务业务错误保留连接；取消不让旧响应满足下一请求。
- `language.modules 1.0` 与安装权限 `native.modules`：导出完整文件闭包、摘要、入口、ABI、引擎范围。宿主与 native loader 均校验全部文件与目录边界，复用也重验；新增邻接文件不能借旧校验进入进程。模块只运行在受管原生分析进程，具有 OS 权限，不声称是 WASM 或 OS 沙箱。
- 一次性语言绑定同时校验精确 guest incarnation、工作区、当前文档 intent。相同包名/provider 不能替代另一个项目的 native plan。共享相同代码的独立消费者各有租约，单个消费者释放不能撤销仍有其他授权的同一代码组；导出者失效则撤销整个代码组。
- `Provider.related_documents` 引用固定只读查询 schema。查询只发送打开文档的身份、URI 与 revision；成员集合由提供者的项目图决定。宿主只同步明确成员，并校验成员版本向量及图代次；该部分仍在实施与回归中。

类型、字段、限额与权限的正式用法保留在 [language](../../../crates/plugin-protocol/src/language.rs)、[language_control](../../../crates/plugin-protocol/src/language_control.rs) 和 [language_context](../../../crates/plugin-protocol/src/language_context.rs) 源码注释，随宿主 SDK 导出相同源码。

## 已观察到的行为

| 验证入口 | 实际结果 | 边界 |
| --- | --- | --- |
| `cargo test -p plugin-runtime --test language_contexts` | 5 项普通公开 Manager 测试通过；真实 SDK hook ignored 测试另行构建并显式执行通过 | 不等于所有插件已联验 |
| `cargo test -p plugin-runtime --test native_modules independent_guest_delegates -j 2 -- --ignored --nocapture` | 真实 SDK guest → 服务调用 → native EditorRequest → 一次性绑定通过；另一个项目的 plan 不能 attach 或签发 binding；卸载 source 先回收进程，无关项目保留，原 engine 可重新准备 | 最近完整执行 1 passed、0 ignored，126.63 秒；不是私有 registry mock |
| 原生 control targeted test | 首次请求先同步 70 KiB 未保存源、实际 per-document language ID；取消和合法业务拒绝后仍是一次 initialize 的同连接 | 源文本走普通受管 LSP 帧 |
| SDK 两子项目 GPUI test | 真实 Base definition provider 导航；两项目的 root、未保存文本、language ID 与实际服务分别匹配 | 最近执行 54.89 秒；未知语言夹具，不依赖 Vue 名称 |
| 真实 TypeScript 跨文件重命名 GPUI test | 首轮发现未打开文件后创建真实 DocumentSession，再重算、完整预览；取消不改文本，确认同时改两文档，磁盘不自动保存，group Undo 同时恢复 | 最近执行 56 秒；后续并发修改与冲突仍需回归 |
| TypeScript native 项目图 | 真实 tsserver 的 shared import 纳入、另一项目排除、分批 generation 一致、未保存 import 改变图；1 项测试通过 | 用于关联文档同步，不代替宿主最终 revision 检查 |

## 分发包测试发现的问题

Vue 0.1.2 的源码语义测试通过，但 TS 强闭包 loader 的真实启动拒绝 HTML language service 的 UMD 动态 `./parser/htmlScanner` 依赖。原先“复制 exports 再起普通 tsserver”的测试还可能命中开发目录的插件搜索路径，不能作为分发闭包通过证据。该测试正在改成由 TS 的真实 managed bootstrap 加载精确 descriptor；包入口正在修正，不放宽闭包允许列表。

所有候选 ZIP 仍是实施输入。当前工具测试覆盖固定 Vue 2.6.14、2.7.16、3.5.22 与 TS 5.9.3，不能外推全部 patch、历史版本或未来版本。普通 Node 测试、独立包构建、宿主集成、原生交互及更新恢复须分别记录；完整矩阵未满足前不标“正式支持”。

## 检查与未验收项

定向 `cargo check -p plugin-runtime -j 2`、`cargo check -p editor-app -j 2` 曾通过；后续并发实现继续修改，交付前必须对最终状态重跑。Windows 曾因未限制 Cargo 并发出现 pagefile 错误 1455/rustc ICE，改为 `-j 2` 或更低；没有改变系统分页设置或安装工具链。

最终仍需记录 `cargo fmt --check`、`cargo test --workspace --exclude editor-app`、`cargo check --workspace`、相关 editor-app 测试与显式 ignored 包验收。尚未完成的共享脏文档隔离、自动导入、生态扩展贡献、更新恢复和首批矩阵不能凭当前表格关闭议题。
