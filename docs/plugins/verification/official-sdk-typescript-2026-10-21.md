# A0：TypeScript 正式目标兼容修复

日期：2026-10-21–22，Asia/Shanghai。状态：**已完成本记录范围的兼容修复与实际业务验收**。本记录不关闭历史 Vue/TypeScript 工单，不提交、推送或发布，不将静态通过计为语言业务通过。

## 来源与范围

- 起始 Editor 本地 HEAD 为 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b`，已有大量未提交改动；TypeScript 起始清洁 HEAD 为 `2bc5be0301fc485d3b684641104e4dd596d3456e` / 0.1.15，Vue 起始清洁 HEAD 为 `173c265e4c19298ca2fdb86b849ae90b6e3e4043` / 0.1.12。
- 已交付公开实现位于 Editor `d80c5708d7ce5e016e5e1a30e40c4b5c0b77b813`；本地 `origin/main` 与 `codex/vue-typescript-remaining` 均指向其后仅补文档的 `d498beff1c5d3bd07fabda69f2fe6c89d043fe9e`。任务开始时主工作区尚未合入该实现。
- 用户已明确选择：将必需的公开契约与宿主路径合入当前工作区并保留全部已有改动。复用该来源的 `language.lsp 1.6` 上下文注册、固定交互准备契约、内部提供者准入/呈现及 `workspace.files 1.4` 只读输入条件和必要事务校验；不另行执行历史 S2–S6 工单。
- 没有删除 `interactive`、放宽未知字段、添加 TypeScript/Vue ID 分支或使用全局 EditorIntent。协议仍为 7，SDK crate 仍为 0.2.0，包版本与能力版本分别处理。未将历史完整 rename/completion/range-formatting 等实现全部搬入，本次结论仅覆盖列明的兼容路径与实际业务场景。

每个复用文件按 `HEAD`、任务前工作区和交付来源做三方合并。任务前文件、逐文件清单和合并输入保存在 `target/sdk-adoption-typescript-2026-10-21/before/` 与 `merge-index.json`；该清单包含探索后已回退的文件，最终以实际补丁为准。冲突处保留当前结构化静态诊断、比较图标和运行观测，仅加入所需语言接线。共享方案和索引不在本会话修改范围。

## 原始失败输入

固定基线 EXE：`target/plugin-developer-tooling/debug/editor-app.exe`，SHA-256 `c82565746b49011761884604da17e1bcbd30a45cb972b42a2a6ba20fda940856`；内嵌 SDK 内容身份 `8e2f359fedf819d8e8b6b3da1012722629e12d9731c02c38e5b01ccab8bd02e4`。本会话没有覆盖该产物或操作其窗口。

| 原 ZIP（`target/official-plugin-sdk-audit-2026-10-21/packages/`） | SHA-256 | 固定基线结果 |
| --- | --- | --- |
| `typescript-supported-0.1.15.zip` | `beab4419c7004ce96d4bf2ce3f4a826f0c8f2a777a9a4b0c3896496f314c038b` | exit1，manifest.json:480:21 |
| `typescript-desktop-0.1.15.zip` | `422074c65d63d89b2f8b060feb15ceaaffaf4b7a3c77b19f6c6c665ebdb39f43` | exit1，同一未知字段 |

命令为确切 EXE 的 `--plugin-check <ZIP> --format json`。原始 JSON 为本会话输出目录的 `baseline-{supported,desktop}.json`，均为 `executed=false`。原 ZIP 保持原字节。

## 额外修正的隔离边界

独立双轴审查发现配套源码还需补齐以下行为，不能仅使清单解析通过：

1. A 的上下文刷新晚到且焦点已移到 B 时，仅撤销 A 的源结果，不取消 B 当前可见请求。真实 SDK 回执在 actor/UI 现有边界保留，再启动 B 的 References 后派发 A，作为回归场景。
2. 注册上下文的控制调用在实际派发和绑定完成处比较所捕获的 live token、provider 与 canonical root；不能将 left 请求静默发送到 right 计划。
3. 每个 owner/scope/provider/source 的选择拥有不可复活的瞬时封印，重复同一选择幂等，A→B→A、关闭源和 owner 退役均使旧封印失效。该封印只约束请求完成/最终通知和未消费绑定，不进入共享进程的持久 EditorAuthority。
4. 已完成但尚未交给 guest 的成功回执仍在实际通知边界检查；同批前一回调重选也不能使后一旧成功漏过。Refresh 捕获此前选择 epoch，并在同一 registry 锁内检查再提交；取消或失效不称为回滚已完成的外部效果。
5. 内部执行提供者可经公开 LanguageDocument 需求进入既有服务准备路径，仍从用户提供者目录隐藏。更新回滚恢复已激活 owner 后，后续角色采用该 incarnation，避免再次 Activate 并替换其资源所有者。
6. 失败准备捕获原始声明集合输入，仅这些输入改变才撤销已发布的终结错误。每个 reservation 有独立发布证明；UI 仅结合自身 Completed Err 授权重新准备，取消、超时或其它等待者的错误不能自动重开用户选择。
7. F12 的主程序自定义处理器与原生 show-document 均传递实际来源计划；等待后再次检查原计划。交互门面检查尚未消费的导航提示，随后按实际 native provider、key/generation/root 检查缓存；内部引擎和非交互角色不受该提示影响。
8. 原生 Control 已按识别语言打开文档时，下一帧的高亮语言可能仍为 `text`。识别更新在文档映射同一把锁内只退休语言不符的 lease；真实关闭、重命名和上下文撤销仍无条件退休，迟到的 wire close 仍检查原 lifetime。

## 构建与初步验证

独立 Cargo 输出为 `target/sdk-adoption-typescript-2026-10-21/host/`；metadata 已核对 workspace_root 为当前主工作区、target_directory 为该独立目录，原始结果见 `metadata.json`。工具链为 rustc 1.98.1 / cargo 1.98.1；未安装或升级工具。TypeScript 原生运行时使用保留的 Node 24.19.0。

| 实际检查 | 已观察结果 | 原始记录 |
| --- | --- | --- |
| 原始 .15 ZIP 经新生产 Package 准入及下限/扩权/未知字段负例 | 1 passed，0 failed/ignored，4.17s | `interactive-admission.log` |
| 标准交互准备声明与非交互契约回归 | 1 passed，0 failed/ignored | `interactive-profile.log` |
| TypeScript `node --test --test-concurrency=1 test/*.test.mjs`，设置独立 Vue 源码夹具 | 48 passed，0 failed/skipped，64.17s | `typescript-node-tests.log` |
| TypeScript JS 类型检查 / cargo fmt --check | exit0 / exit0 | `typescript-jscheck.log`、`typescript-fmt.log` |
| 工作区 cargo check 初步集成 | exit0 | `workspace-check-3.log` |
| 工作区非 app 测试一轮（后续瞬时封印修正前） | 452 passed，0 failed，349 ignored，92 summaries；ignored 未运行 | `non-ui-2.log` |
| 新宿主首次 build | exit0，7m55s；仍需在最终源快照上复查 | `host-build.log` |

首次宿主 SHA-256 为 `9fb75adce0e789239b6e791926e179d5808dcf72cb988a83bed80ab6dc5b4f0b`。两个原 ZIP 经该 EXE 静态检查均 exit0，报告为 `first-static-{supported,desktop}.json`。后续源修正后的最终宿主/SDK/新包身份另行记录，不能混用首次 EXE 的身份。

## 新包与验收身份

- 独立新包：`C:/Projects/RustProjects/TypeScript-for-Nanobug/target/sdk-adoption-2026-10-21/packages/typescript-0.1.16.zip`，SHA-256 `a254457d2fc846f9c218b4b59f5f310b02947ed473bdda2279d9d2e83131e478`。由首次匹配目标的公开 `--plugin-package` 构建，后续没有覆盖其字节。
- 内嵌 SDK 内容身份为 `f1cda18f38da09d4c6387b048cf2978a6aee7117ce1a6fa4301ced9f48ab4520`；TypeScript 受管构建和 guest 测试均使用该 SDK。最终主程序通过 `--export-plugin-sdk` 导出到本会话 `final-sdk-5/`，仅有 SDK 清单、源码和协议资源。
- 包内 `typescript.wasm` SHA-256 `d899803cf8d68079bb3dffdc47a81273286d9ba357bb99b8bb0aa808b96219b8`。原生服务仍为 0.1.14，包内 `artifacts/typescript-windows-x86_64.zip` SHA-256 `71c2794f0833bb548d365255878362b4637750bdac05e7ff7e8392df73884f83`，与保留原归档相同。审计见 `final-package-audit.json`。
- TypeScript 只更新 `Cargo.toml`、`Cargo.lock`、`package.json`、`manifest.json`、`plugin.toml` 的版本与双语 README 兼容说明，没有改 native 或 guest 业务源码；协议、权限不变，没有新增 SDK helper。Vue HEAD、源码和版本保持起点。
- 三版本实际组合使用的 app 测试 EXE SHA-256 为 `71c4370324853e6dec54a321bc502df2834eb2070404cae80cfe572e1ddd1ba0`，见 `vue-combination-executable.json`。后续仅加强 lease 回归的真实 wire 断言，生产代码相同。
- 最终主程序为 `host/debug/editor-app.exe`，SHA-256 `e69a563786a896729bad4c8aaeed4c3237294617b9ee158050e54460aaee67c5`，`cargo build -p editor-app -j 2` exit0，28.74s，见 `final-host-build-5.log`。它静态检查两个原 .15 ZIP、新 .16 ZIP 及 Vue .12 ZIP 均 exit0、passed true、executed false，SDK 身份均如上；见 `final-static-{supported,desktop,rebuilt,vue}.json` 和 `final-static-gates.json`。不将此前 EXE 身份混用为最终主程序。
- 最终四项 app 复跑收据为 `final-app-gates.json`，其测试 EXE SHA-256 为 `22d6b8da151f660b8177eed402624b8bd3f5fb21375037820140f564eb66d9a3`。随后仅加强识别回归的 wire 帧断言；该次 EXE SHA-256 为 `cd0434d3b6c9cd9c330c66e277a51f6213a3ece23d2fc27e51a15b069de0fabf`，收据为 `recognition-wire-lifetime-gate-3.json`。

## 真实业务与门禁

所有下述相对日志路径均位于 `target/sdk-adoption-typescript-2026-10-21/`。WASM/外部包测试显式传入固定 ZIP 并使用 `--ignored --nocapture --test-threads=1`；普通 workspace 的 ignored 数不计为通过。

| 实际入口或场景 | 观察结果 | 记录 |
| --- | --- | --- |
| runtime library：源选择 A→B→A、已完成 Control/Refresh 的最终通知门禁等 | 122 passed，0 failed，3 ignored | `runtime-unit-final-2.log` |
| 真实 context-selection 包：取消的共享等待、原集合改变撤销 Err、新 reservation 不继承证明 | 1 passed，56.50s | `failure-publication-final-2.log` |
| 原 .15 包严格准入负例及隐藏 engine 的公开 LanguageDocument 需求 | 2 passed，24.26s | `internal-engine-admission-final.log` |
| TS 受管 guest lib 测试：通过内嵌 SDK 的公开 `--plugin-cargo` | 1 passed | `typescript-guest-tests.log` |
| TS 单独运行：禁用 Vue 后配置项目和推断源、实际语言请求、独立根与清理 | **1 passed，84.00s** | `typescript-native-final-4.log` |
| 真实项目选择：取消、左右项目语义、A/B 请求独立、持久偏好、不兼容候选拒绝、F12 临时继承与重新打开 | **1 passed，70.41s** | `shared-context-final-10.log` |
| Vue 2.6/2.7/3 共存：未保存中文/emoji、实际定义/悬停/补全/References、各源唯一诊断及正确范围、独立进程与退出 | **1 passed，113.49s** | `vue-typescript-final-7.log` |
| 识别更新保留实际 Control 语言、真变更撤销、其它源保留、迟到关闭不关闭或重开新 wire 文档 | **1 passed，0.07s**；真实 definition reply 为顺序屏障，迟到 close 前后 open/close 帧集合不变 | `recognition-wire-lifetime-final-3.log` |
| 原生分析输入失效阻止待应用写入，已提交恢复收据保留；owner 退役撤销恢复 | **1 passed，5.70s** | `analysis-guard-final-3.log` |
| `cargo test --workspace --exclude editor-app -j 2` | **455 passed，0 failed，352 ignored**；所有测试汇总 exit0 | `final-non-ui-3.log` |
| `cargo fmt --check` / `cargo check --workspace -j 2` / `git diff --check` | 最终生产 app 修改后均 exit0；最后仅测试加强后格式和差异再次 exit0 | `final-format-3.log`、`final-workspace-check-3.log`、`final-diff-check.log`、`final-gates-3.json`；测试后为 `final-format-5.log`、`final-diff-check-3.log` |

三版本场景用真实 Vue 仓库的原始项目和实际依赖作为只读输入，所有源修改留在 EditorState，结束时比较磁盘原文。先等待各源初次绑定，再保留三个未保存源并发请求实际语义；没有用模拟语言回复、另造宿主插件白名单或放宽断言。

保留的失败记录及纠正依据：

- 首次快速连开并立即输入导致冷 WASM 重复编译和超时，见 `vue-typescript-final.log`。最终顺序建立初次绑定的场景不能据此推断该冷启动压力场景已重跑通过。
- 私有复制夹具漏了 Vue 3 的传递 `@vue` 依赖，导致其类型退为宽泛类型、诊断为空。`vue3-diagnostic-{probe,original,fixed-fixture}.log` 对比了真实后端报告；最终直接使用原项目依赖，未修改服务。此夹具问题与随后实际 lease 竞态分别记录。
- 补齐依赖后暴露 `Document has been closed` 的生产竞态，见 `vue-typescript-final-{4,5,6}.log`；最终按实际 lease 语言修正后，完整三版本场景通过，没有延长生产超时或给取消失败自动重试授权。
- 一次并行 app 测试重新链接命中了仍运行的同一 EXE，`shared-context-probe.log` 为 LNK1104；随后 app 测试串行运行。一次 workspace 链接报 `msvcrt.lib` 无法打开，保留在 `final-non-ui-2.log`；受限并发原入口复跑通过，不归为源码缺陷。
- 加强 lease 回归时，最初将 push-diagnostics 轮询误当成 wire 屏障，读取正在写入的 JSON 行失败；随后直接调用私有 `definitions_for` 被编译器拒绝。记录在 `recognition-wire-lifetime-final{,-2}.log`。最终使用现有测试 wrapper 取得真实 definition reply，不扩大生产接口可见性，帧断言通过。

## 隔离桌面、更新与退出

仅操作本会话独立 EXE/项目/profile，未操作固定基线窗口或日常配置。桌面记录与最新生产回归的身份分别列明：

1. `cd914a7af204e58f23ebc364b950450e4c4de06a6349d074e04cf5d8f70ce01e` / SDK 同上，`native-profile-2`，同一个 GUI PID `43108` 经历完整更新序列。真实 TS 错误产生原生标记，未保存 `// 热更新保留 中文😀` 经过失败候选和成功候选后均保留，Undo/Redo 可用，原磁盘 SHA-256 `5c249f28518f42f5e11f46dd259562d1ff97ee7d284e8e655bd67b8d7e216a23` 不变。
2. 仅在 `native-source/` 复制开发源构造 0.1.17 的失败/成功 probe，未改真正 .16 包。generation 2 激活失败；generation 3 经公开 SDK 写入并关闭 `rollback-probe.txt` 后故意失败，旧内容 `old-private-state\n` 及 SHA-256 `1d500d57bdf79eadcaeb4b8a46e13f702084f9e38c9fb23a0d55dfd78930decf` 恢复，旧 .16 注册与已提交 owner 保留。
3. generation 4 使用恢复原 guest 业务源码的成功 probe 切换，真实 Node PID 从 `10052/62468` 变为 `41868/57652`，旧资源撤销；同一窗口的文本/Undo/Redo 保留。见 `native-registry-after-{data-failure,success}.json`、`native-processes-{before,after}-success.json`、`native-after-*.json`。
4. `native-report-repaired.json` 故意包含失败历史，因此 admission/startup/cleanup passed、activation/status failed、normal termination；不能把负例报告写成整体通过。最初关闭窗口曾触发 AccessKit `0x80070578` teardown panic，保留该环境/可访问性限制，后续使用 owned CLI `stop` 正常退出。
5. `e299c380a3339623bd637a4c4992f01226aa642658f402399fa84caea25e02d1`（实际语言 lease 修正前）用真正 TypeScript .16 源重建运行，`native-report-clean.json` 的 admission/startup/activation/cleanup 均 passed，normal termination；4 个资源 owner 全退休，handles/requests/processes/native_pending 均 0，无 unconfirmed cleanup。工具的 overall status 仍为 inconclusive、business 为 not_exercised，CLI exit2；语言业务结论使用上表真实 GPUI 场景，不能由 lifecycle 字段替代。
6. 用户授权保留窗口内容并退出。关闭前观察到手动输入已经撤销回原四行源；可见全文保存在 `native-window-backup.ts`，与磁盘基线同 SHA-256 `5c249f…16a23`。随后通过本会话 CLI `stop` 关闭 owned 实例，没有覆盖用户文件。

本节桌面更新结论属于上述确切 EXE；后续 owner-adoption、失败输入和 lease 修正通过各自最终公开包/原生回归核验，不将较早桌面报告重新标注为最终 EXE。最终源的双轴审查没有剩余生产发现；审查是静态审查，不替代上述实际运行。

## 交付边界

保留原 ZIP、固定基线 EXE、当前工作区既有改动和独立 Vue 源。没有提交、推送、发布、关闭 issue、安装到日常 profile，也没有执行 Git/Markdown 的并行任务或修改其共享方案/索引。本次只确认公开目标与列明的 TypeScript/Vue 实际路径，不重做历史完整迁移验收。Windows 之外的原生交互、完整 IME 候选/提交流程和原始高速冷启动输入场景没有由本次最终矩阵覆盖。
