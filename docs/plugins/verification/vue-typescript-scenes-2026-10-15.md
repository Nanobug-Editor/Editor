# Vue 与 TypeScript 六场景实施记录

日期：2026-10-15。状态：22:51 按最新指令暂停，代码及验证进度已保存，见[最新暂停检查点](vue-typescript-paused-2026-10-15-2251.md)。尚未完成正式支持验收、最终审查、提交或推送。

## 当前断点

- 仅推进新 #121–#126；旧 #108–#118 及父 #107 不再改动或关闭。
- S1 单次控制任务结束错误回收原生上下文的回归已 RED→GREEN：未知当前 SDK 包连续两次 `LanguageRequest`，真实 stdio `initialize` 从错误的 2 次变为 1 次。GREEN 75.58s 含冷包准备/JIT，实际两次控制阶段 423.08ms；不能把冷准备时间称为紧密循环。源身份、配置与独立 sealed caller chains 保留后续调用租约，文档关闭、授权链退休、配置变化及撤信任会释放。
- 同 TS 0.1.7 / Vue 0.1.7 的原 mixed 复测在更早的自然冷启动取消竞态失败：打开后立即输入使旧准备取消，新有效 demand 遇到 `Abandoned activation is still retiring`，46.01s，尚未走到上述控制链。由新 S1 继续修复，未报告 mixed 通过。
- S2 最新真实 TS 0.1.8 原生重命名/自动导入/Undo 完整回归 1/1 通过（43.49s）：包括只读目标整包拒绝、只读辅助输入变化撤销预览、未知目标正常打开重算、预览取消、应用不保存、跨文档分组 Undo，以及实际补全 resolve/自动导入的一次 Undo。Vue 跨组件事务与混合共享源仍待联合验收。
- S3 最新可见编辑器回归仍未全部通过：实际 grammar 及浅主题曾通过，Undo 夹具已按逐键输入契约修正，另一轮在样式颜色就绪处失败，深主题/IME/滚动/缩放仍待完整验收。
- S4 TypeScript 0.1.8 源码完整原生服务测试 35/35（无跳过）及类型检查通过；Vue 0.1.8 源码与真实密封闭包各 84/84、宿主原生 grammar 2/2。三包 production actor 联验等待 S1，不能以工具层通过代替。
- S5 实际热更新回归发现恢复实例与冷状态记录不一致，最小修复后原完整回归 1/1（215.69s）；数据迁移/激活失败回滚已通过，详见 [S5 通用记录](vue-typescript-s5-runtime-2026-10-15.md)。实际 Vue 后端恢复仍待验收。

最新冻结候选为 TS `dist/typescript-0.1.8.zip`（SHA-256 `6265d3db2a19798e803fb700ccf9b2d9abd6fe00cd355f1179252847787f8229`）与 Vue `dist/vue-0.1.8.zip`（SHA-256 `92c9dc9c14f82138a0ac22589e1b9566ddcde2c2fa85a66baf2d0ed9a5d5ce24`）；旧候选均保留。控制租约回归显式使用当前 SDK 构建的 `target/s1-fixtures/capability-example-0.19.1.zip`（SHA-256 `d9c4a4b8933b7372fde0115cd678e04fb85b3f034a5ed6e54a9510e22b954b68`），未覆盖原固定输入。

新增日志：`target/s1-control-lifetime-{red,green}.log`、`target/s1-control-lifetime-{red,green}-build.log`、`target/s1-mixed-control-lease-green.log`（文件名含 green，但实际结果 FAIL）、`target/s2-typescript-editing-018-test.log`。最终检查仍需覆盖最终源码与产物。

## 范围与进度保留

用户已批准[六场景重拆方案](../tickets/vue-typescript/replan-proposal.md)。新工单 #121–#126 的父子关联与原生阻塞边已通过 GitHub 读回核对，见[发布记录](../tickets/vue-typescript/scenarios/publication.json)。旧 #108–#118 全部保持开放，已补进度评论，保留已验证、已写未验证、尚未实施的区别及新场景引用；没有修改或关闭父议题 #107。用户后续明确只执行新 #121–#126，旧 #108–#118 不再推进或关闭；既有代码与测试作为新场景基础复用。

原实现和失败证据见[暂停检查点](vue-typescript-paused-2026-10-15.md)。宿主仍在隔离 worktree `C:/Users/Tmiracle/.codex/worktrees/vue-typescript/Editor`；两个插件分别在独立仓库，原始 Editor 工作区的其他任务改动没有纳入本任务。

## S1 / #121 已执行

- 补齐公开 `Request.context_snapshot`（默认 false）与 `SourceMetadata`，分析 opt in 完整关联快照，resolve/attach 使用单目标；公开消费者无需猜测 LSP wire version。
- TypeScript 原生分析验证宿主封存的源向量，可选前置条件使用原生 DocumentVersion，返回同一原生版本向量；传输版本与文档 revision 分离。
- TypeScript 自身及适配模块允许标准范围格式化，非法或反向范围拒绝。
- 修复暂停时 diagnostics 两处缺参数及 mixed 测试 root 移动后的编译错误；更新 app 的 typed wire sources 与同连接完整版本核对。
- TypeScript 0.1.1 通过当前公开 `--plugin-cargo`、`--plugin-build`、`--plugin-package` 生成候选 ZIP；不表示最终交付。

| 命令 / 证据 | 实际结果 |
| --- | --- |
| TS `node --test --test-name-pattern='native document identities' test/control.test.mjs` | 先 RED（缺少猜测的 snapshot vector），修复后 GREEN 1/1；native revision 37 与 wire 9 分离，旧 revision/wire 拒绝 |
| TS `node --test --test-concurrency=1 test/control.test.mjs` | 5/5 |
| TS rangeFormatting 定向回归 | 先 RED（Unsupported language feature），修复后 GREEN 1/1 |
| TS `node --test --test-concurrency=1 test/*.test.mjs` | 12 通过、4 跨仓库用例跳过；跳过不算通过，日志 `target/s1-native-tests.log` |
| TS `node node_modules/typescript/bin/tsc -p jsconfig.json` | 通过 |
| `cargo check -p editor-app --tests -j 2` | 暂停断点修复后通过，保留既有 warnings；不覆盖之后继续修改的最终状态 |
| `cargo build -p editor-app -j 2` | 首轮 SDK 宿主构建通过；含 OpenSet 与后续修复的第二轮也通过，日志 `target/s1-sdk-host-rebuild.log` |
| native_control_and_document_aliases_share_one_connection 显式 ignored | 1/1，0.16 秒，日志 `target/s1-control-host-test.log` |
| related_native_sources_are_explicit_and_auxiliary_changes_revoke_results | 新 metadata 断言先失败：误把每个文档 wire 初值当成 1，实际该连接递增至 2；改为与真实 didOpen 帧独立观测对应后通过 1/1（0.07 秒），日志 `target/s1-related-host-retest.log` |
| actual Vue/TS production Worker mixed 显式 ignored | **失败**，355.58 秒；Vue 报 `InvalidHandle: Activation original intent was revoked`，识别降为 text，无 bindings，未进入模板分析。日志 `target/s1-mixed-host-test.log`；没有以此宣称阶段通过 |
| cancelled_first_preparation_keeps_static_catalog_and_can_prepare_again | 真实 Package→Manager RED→GREEN 1/1：晚到取消结果不再把全包 catalog 标错，真正准备/激活错误继续报告；首次取消原因仍待 production 验证 |
| mixed 第二轮 / Vue 0.1.3 | 失败 118.16 秒，但冷启动 actor / 原生语言识别阶段标记通过；三个上下文均准备超时，两个插件 catalog 保留。日志 `target/s1-mixed-host-retest.log` |
| mixed / Vue 0.1.4 release | 失败 87.81 秒；三个 Vue 委托绑定已出现，随后依赖调用取消，仍未进入模板分析。日志 `target/s1-mixed-release014-test.log`；继续用分阶段错误与控制请求观测定位，未扩大超时 |
| mixed / TS 0.1.2 + Vue 0.1.5 release | 失败 89.42 秒；分阶段错误证实 resolve 超时。Vue main JIT 19,059 ms、TS main JIT 17,007 ms，三个控制请求已从 Worker 被原生 UI 消费，随后 TS 再次开始编译。日志 `target/s1-mixed-release015-test.log`；调查重复编译/首次上下文准备，不再以 UI 缓存推测根因 |
| independent_vue_candidate_grammar_uses_the_native_loader | 首次失败于夹具非 canonical root，未进入 loader；修正夹具与生产 Contribution 相同的 root 不变量，新 bin 待重跑 |
| live compiled code / warm role | 固定 SDK guest 首轮 warm 之后，新的语言角色在原 5 秒预算内完成：先 RED（TimedOut，总 60.71 秒含冷启动），复用同一编译代码、每次创建独立 Store 后 GREEN 1/1（总 53.47 秒含冷启动）；不能把总时间当 warm 阶段耗时，日志 `target/s1-warm-role-{red,green}.log` |
| mixed / TS 0.1.4 + Vue 0.1.7 | 失败 95.40 秒；三个 TS 项目计划均发布且日志确认代码复用，随后三个进程初始化报 Windows `EISDIR: lstat 'C:'`。日志 `target/s1-mixed-release017-test.log`，不表示模板分析通过 |
| Windows native initialize / import | public stdio 首轮 initialize RED 86 ms → native realpath 修复后 GREEN 588 ms；宿主规范模块路径另一个 RED 587 ms → GREEN 995 ms，包含真实适配后 hover 和未封闭邻居拒绝。TS 0.1.6 候选包含两项修复，仍待 production Worker 全链 |

TS 候选：`C:/Projects/RustProjects/TypeScript-for-Nanobug/dist/typescript-0.1.1.zip`，40,551,850 bytes，SHA-256 `7D47CF88ED7EB6FEA28D58063593B55AE263C8F793ED856205E5D410A4B00E6C`。该包不含之后 S4 的组合能力，后续版本单独生成并保留本候选。

## S2 / #122 已执行

真实 TS 0.1.1 原生编辑回归在只读辅助输入变化处 RED（56.36 秒）：预览未自动撤销，自动导入后半段尚未运行。日志 `target/s2-typescript-editing-test.log`。原因是预览只检查请求/授权退休，没有保存原文前置条件的身份向量；应用前已有版本校验，但预览显示仍需同步失效。

已把只读条件与编辑目标的原生版本元数据保留到预览，并在源/目标变化时以 StaleRevision 撤销、向异步准备返回明确错误；不复制文本、不改变 Undo 目标。第二轮真实回归已通过该检查及重命名预览、取消、应用、分组 Undo，随后失败于自动导入前未出现补全弹层（164.02 秒，`target/s2-typescript-editing-retest.log`）。新测试等待实际 Base completion provider 就绪后再输入，尚待重跑；不能把后半段标 GREEN。

等待 Base provider 后仍失败（165.85 秒，`target/s2-typescript-editing-third.log`），排除单纯准备时序。最小真实 stdio 回归 `auto-import survives a completion set larger than 256 candidates` 在 695 ms 复现：原生转换先截断编译器候选 256 项，再由宿主按输入排序，项目导出 `double` 被永久丢弃。移除静默截断、保留传输字节预算显式失败后，同一测试 694 ms GREEN，实际 resolve 返回导入编辑；`test/language.test.mjs` 3/3，native TypeScript 检查通过。新 0.1.3 候选仍待完整原生事务回归，未把该工具层 GREEN 外推成 UI 通过。

TS 0.1.3 原生重跑已经出现并选中候选，随后因文件头 import 插入与首 token 替换共用起点被拒（92.35 秒，`target/s2-typescript-editing-013-test.log`）。紧密的 `completion_import_at_first_token_boundary_forms_one_atomic_proposal` 先 RED、边界操作合为单一提案后 GREEN；连同重复插入、真正重叠和主插入冲突拒绝共 2/2，`target/s2-completion-boundary-{red,green}.log`。实际完整 UI 事务仍待重跑。

完整原生事务已用冻结 TS 0.1.8 重跑 GREEN 1/1，43.49s，日志 `target/s2-typescript-editing-018-test.log`。此轮实际走过上述自动导入的选择与 resolve、整个原子提案、一次本地 Undo，以及前半段重命名预览/取消/辅助冲突/跨文件应用/分组 Undo；不是仅凭之前的紧密边界测试报告通过。

`paged_related_graph_synchronizes_every_large_native_source_without_truncation` 显式 ignored 1/1 通过（6.47 秒，`target/s2-large-graph-test.log`）：260 个未保存源每个大于 64 KiB，经有界元数据分页及正常原生同步完整到达，逐个核对正文与完整 260 项 revision 向量。此项不等于已证明任意大小的重构计划可超出其独立预算。

## S3–S6 当前界限

- S3：Vue 0.1.4 源码与密封闭包各 68/68，通过后追加历史自动导入源/密封各 8/8；0.1.5 正在验证非法 script 语法及未知块语言注入修复。实际宿主七项能力、高亮和范围格式化尚未完整验收，历史 formal 标记仍等待 S6。
- S4：`plugin.services 1.2` 的显式组合 OpenSet/ValidateSet 首轮真实 release SDK 夹具 2/2 通过（182.38 秒），新增空集合与 peer 空贡献 guard 仍待四项重跑。未知原生模块已实际改变 TS 类型与 Vue 原文映射诊断，最新组合测试 4/4；独立生态 WASM 包进入宿主的整链尚待验收。普通 Open 继续单选。
- S5：未把既有平台能力当作本轮完整恢复验收；更新回滚、兼容保留、离线/取消/坏摘要、多消费者和撤权仍需实际组合验证。
- S6：最终 V01–V14、全部强制检查、原生交互、双轴审查及普通提交/推送/issue 状态读回未完成，不发布版本。

新 S3 的补充证据：Vue 0.1.6 实际 ZIP 的生产 grammar loader 1/1 通过；Vue 0.1.7 + TS 0.1.4 两项 `vue_package_native` 2/2（8.26 秒，Vue `target/native-blocks-017.log`），覆盖八个真实 grammar、query 原文范围、缺失/未知块不冒充、已有选择保留及退休拒绝。原生选区格式化由未知语言包进入 GPUI，先 RED 缺少用户动作，完成标准范围请求后 GREEN 1/1（5.31 秒，`target/s3-range-{red,green}.log`），CRLF/Unicode 坐标、一次 Undo、磁盘不变。编辑器可见高亮测试首次在 projection ready 失败（31.27 秒），尚未进入主题/输入断言，正在定位夹具装配。

新 S4：最新集合夹具 4/4 通过（188.18 秒），普通语言上下文 7/7、2 ignored；真实角色测试继续补齐，不能以此前 Node 21/21 外推当前全部角色或安装包联验通过。

新 S5：既有 fault 回归首轮因旧测试假设 install 后自动激活而失败（22.25 秒）；迁移到当前公开显式 Demand、固定 SDK 版本及冷重启语义后 1/1 通过（194.16 秒，`target/s1-fault-isolation-green.log`），保留 fuel/memory 故障、pending 退休、健康 peer 与私有数据恢复检查。该证据属于新 S5，不执行或关闭旧工单，也不等于实际 Vue 后端恢复通过。

S6 首轮 `cargo fmt --check` 失败，尚有本任务 Rust 文件待集中格式化；未将该检查记录为通过。最终 workspace 测试、check 与审查仍待完成。

## 独立源码仓库

用户明确批准创建公开的 [Vue](https://github.com/Nanobug-Editor/Vue) 与 [TypeScript](https://github.com/Nanobug-Editor/TypeScript) 仓库。已创建并读回身份/公开状态，两个本地独立仓库已设置各自 HTTPS origin，当前均未提交或推送；不创建 Release。
