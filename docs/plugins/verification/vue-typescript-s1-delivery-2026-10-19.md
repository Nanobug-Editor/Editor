# Vue / TypeScript S1 正式基线复验与交付

日期：2026-10-19（Asia/Shanghai）。范围仅 [#121 / S1](../tickets/vue-typescript/scenarios/01-real-projects.md)；#122–#126 继续暂停，旧 #108–#118 与父 #107 不变，不创建 Release。

当前状态：**#121 已完成并关闭。** S1 在正式基线上的门禁、真实包联验、相关原生交互及双轴审查均通过；三个仓库均已普通提交、推送并核对远端 main。GitHub 实际读回 `closed / completed`，关闭时间为 `2026-10-19T13:07:07Z`（本机 21:07:07）。此前行为测试、失败历史、候选身份及完整命令见[恢复记录](vue-typescript-s1-resume-2026-10-19.md)。

## 正式提交边界

- 父提交为 `b59f5ac546a53ba084c729d382871a046bec9d38`。本地 main 与 `git ls-remote origin refs/heads/main` 均已读回相同 SHA。
- S1 使用 `C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor` 的 `codex/vue-typescript-s1` 分支；`cargo metadata --no-deps --format-version 1` 确认 workspace 与 target 均在该树内。
- 主工作区唯一已有改动 `docs/窗口布局拖放规则.excalidraw` 不进入 S1。其 SHA-256 为 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6`，交付前后核对保留。
- 原验证源码先存入 stash `465649b4f1396d5aeba5617326de16af6b5989a9`，并保留 `target/s1-resume-2026-10-19/s1-validated-overlay.zip`：154 entries、677,403 bytes、SHA-256 `ea9d8d391f3d2f5441dcc88bed464ee694948e9d2c8df5e76b66b73b8b786085`。没有整批应用含外部 WIP 的 stash。
- 逐文件三方核对冻结外部基线、正式父提交与 S1：忽略 CRLF 后，正式父提交与冻结基线有 28 个实质差异；75 个 S1 修改路径的两版基线内容相同，77 个新增路径在两版基线均不存在。唯一双方修改的 `docs/plugins/README.md` 已三方保留 main 新增的 docking 文档链接和 S1 索引。其余主分支改动未被覆盖。
- 历史差异账本按原冻结时点保留；本次正式集成另行记录实际提交范围。UI、主题、高亮底座和 Git 工作台前置不作为 S1 成果。

## 输入和证据复用

S1 的协议、runtime、语言控制与测试源码均与先前已通过的 overlay 对应；SDK 的公开契约和 LF 规范化源码未变。内容寻址缓存读取原始字节，11 个 SDK 输入的 CRLF 差异会产生不同缓存摘要，不能冒称新旧宿主的缓存身份相同。最终普通 bin 已通过 `--export-plugin-sdk` 实际导出全部 53 个输入，逐项与嵌入源码完全相同，按宿主算法计算的新摘要为 `9187f5a734591977dc2f6bce15402d3bae6a99a26d63eebd41dcc2d1ba4a38b1`。随后两个插件均经公开 `--plugin-cargo` 使用这个实际缓存编译检查通过。

复用的 Node、Vue 策略矩阵及授权生命周期证据仍保留各自源码、SDK 和包身份；不将 `.9` 的策略矩阵追记成 `.10` 重跑。纯换行没有改变公共契约，不为追平缓存名重打或覆盖已验收包。正式基线另复跑仓库门禁、受影响宿主测试、真实三版本混合项目与 Windows 控制生命周期，并验证普通原生桌面的源位置与引用。

| 最终候选 | SHA-256 | 边界 |
| --- | --- | --- |
| TypeScript `0.1.10` | `fbc363cb36e67f5583449628b428ed77f2dbcb0fe9afe759ae0c18b15743cff5` | 40,917,920 bytes；22 entries；服务 ZIP `33764ce30d0b2101df7c1c07f4edb96411939ac928c723cc6d595bdb449179aa` |
| Vue `0.1.10` | `59cc1e3ae71c5e0897df096eeec88269ff8f5a6433af0e1448c931dcc8f39ee4` | 4,108,507 bytes；39 entries；11 项模块闭包保持原字节 |
| 上述候选构建时的公开 SDK 缓存 | `76a7a14a0bc46d4dc9573cc5342edfda352c805ba4f780e1f9b1b493a2ccdd12` | 两个独立插件与未知消费者均经当时宿主公开 SDK 构建入口；不将新宿主的换行摘要追记为旧包构建身份 |

初始插件仓库还保留历史 S2–S6 候选源码、测试与封印资源；保存这些既有文件不代表本轮推进、验收或关闭其他场景。`dist/`、`target/` 和运行日志不进入源码提交。

## 本轮门禁与原生复验

以下命令均在正式基线加 S1 的集成树实际执行。日志位于该树 `target/s1-main-integration-2026-10-19/`；ignored 只在显式执行的用例中计为通过。

| 检查 | 实际结果 | 日志 / 边界 |
| --- | --- | --- |
| `cargo fmt --check`、工作树及最终暂存 `git diff --check` | PASS | 没有使用原快照的两处格式豁免。暂存检查发现 7 份历史导入 Markdown 的额外 EOF 空行，仅删末尾空行后实际复核通过，不改其他场景正文、状态或行为 |
| `cargo test --workspace --exclude editor-app -j1` | PASS；318 passed、0 failed、322 ignored，82 suites；7m04.79s | `workspace-test.log`；不是 app UI 或全部 ignored 通过 |
| `cargo check --workspace -j1` | PASS；8.69s | `workspace-check.log` |
| `cargo test -p editor-app --no-run -j1` | PASS；2m30.10s | `app-test-build.log`；只表示编译，实际运行另列 |
| `cargo build -p editor-app --bin editor-app -j1`、`cargo build -p plugin-runtime --example lsp_fixture -j1` | PASS；1m08.99s / 0.39s | `app-bin-build.log`、`lsp-fixture-build.log` |
| 新宿主公开 SDK 编译两个独立插件 | PASS；TS 33.20s / Vue 33.91s | `typescript-current-sdk-check.log`、`vue-current-sdk-check.log`；`--plugin-cargo <Cargo.toml> check --locked --offline --release --target wasm32-wasip2 -j1`；先核对两个 workspace/target，未重打、覆盖候选 ZIP |
| app 普通 `language::` | PASS；44 passed、24 ignored，6.52s | `app-language-tests.log`；不将 ignored 计为通过 |
| strict ACK、同管道控制、Unicode Base 定义、关联源撤销、最终 Vue grammar | 5 次定向均 PASS；0.45s / 0.15s / 0.32s / 0.08s / 0.55s | `host-regressions.log`；匹配准确测试名，以 `--include-ignored` 执行适用入口；部分普通用例与上一行重叠，不相加为独立场景数 |
| 最终 TS `.10` / Vue `.10` 实际三版本混合 | PASS；1/1，110.84s | `actual-mixed.log`；生产 Worker、固定 Vue 2.6.14/2.7.16/3.5.22 与 TS 5.9.3、五项模板语义、真实源位置、唯一诊断、三独立 TS 进程、磁盘不变与 checked Shutdown |
| Windows 实际调度的控制生命周期 | PASS；parent 117.97s / child 117.90s | `native-controls.log`；一个场景，不重复计数。原 1000ms plain/module 请求实测 1.0262951s / 1.0262431s，准确取消、同 Arc/barrier、关闭 pending/new identity、未知导入保留撤销权及实际 HWND 销毁均通过；没有放宽期限 |
| 普通原生桌面 | 通过；进程退出 0 | 私有 profile `target/s1-main-acceptance-b59/profiles/s1-verified-M0HFHm`，仅操作本次私有路径窗口。三种 Vue 的 F12 分别到各自 `App.vue` 的 4 / 4 / 3 行；Vue 2 中文/表情后偏移正确。Vue 3 引用列表为真实 `:3:7`、`:5:26`，无虚拟 URI。关闭私有窗口，保留用户其他 Nanobug 窗口 |

本轮固定验收程序保存在 `target/s1-main-acceptance-b59/`，后续 Cargo 不覆盖这些输入：

- `deps/editor_app.exe`：SHA-256 `9862ae51e12d16a96843f769c18ee2574547109c6ecaa56a897971315d229118`。
- 配对 `examples/lsp_fixture.exe`：`4604b1d2a9043379d9422ec07b3fc4486af04c6fff08ddca9d3247124819e9e1`。
- 普通 `ui/editor-app.exe`：`53de97e21a30ff5f5e7e01bc9284c308d57a0dc4e171201e63998121b03aa096`。

桌面前后三个固定源文件摘要不变：Vue 2.6/2.7 `App.vue` 均为 `4265eef7d5aef971caa1d9d2f21035e86ca45d6a1c14b4dc52d0330382bfd04f`；Vue 3 为 `accfb683e1c4a8a0c17509c9ef5ac93e1efc92fbf43c27fc789dc5cc283d6033`。本轮新基线桌面复验为只读跳转/引用，不新增中文 IME 或主题交互验收声明；旧基线临时输入/Undo 的证据按原身份保留。

## 审查与交付

以正式父提交为固定比较点，分别复核 Standards 与 Spec，均为 **0 findings**。报告为本轮 target 中 `standards-review.md`、`spec-review.md`；审读时候选包括全部新增文件，未将空的 `b59f5ac...HEAD` 冒称为有变更的提交审查。最终只暂存核对后的 155 个 S1 路径；后续状态更新不改变已验收实现。

| 仓库 | 已核对的普通提交 / 树 | 推送状态 |
| --- | --- | --- |
| [Editor](https://github.com/Nanobug-Editor/Editor/commit/8e3104a2b62470d2d02f9b7d3d2343a844e8f2f8) | 实现提交 `8e3104a2b62470d2d02f9b7d3d2343a844e8f2f8`；父提交固定为 `b59f5ac`；tree `81cb06652b964fdebc5ed4406c55e40522f3b40f` | 已普通推送 main，`ls-remote` 精确读回 |
| [TypeScript](https://github.com/Nanobug-Editor/TypeScript/commit/873e8c3d3ebe3a86a8bb1e028a3c4bcd1c557b94) | `873e8c3d3ebe3a86a8bb1e028a3c4bcd1c557b94`；tree `3e98d127e5c17268fffcac3c2c53766f07eff75c`，77 files | 已普通推送 main，`ls-remote` 精确读回；GitHub main 清单与提交一致 |
| [Vue](https://github.com/Nanobug-Editor/Vue/commit/f8e5880bf89161f16723ae7be1c76458d50a676c) | `f8e5880bf89161f16723ae7be1c76458d50a676c`；tree `b3b7d5f9a5262cedcaff9307765ef33286b2afc5`，137 files | 已普通推送 main，`ls-remote` 精确读回；GitHub main 清单与提交一致 |

两个插件的临时 index checkout 与暂存源码逐文件 SHA 相同；TypeScript 的 15 个服务文件和 3 个 grammar、Vue 的 11 项模块闭包与原候选逐字节一致。Vue 新增带注释的 Git 属性以保留资源和 CRLF 夹具原字节，仅对四个精确封印资源路径保留上游空白；另忽略单个构建日志、删除非包输入 legacy slots 源末尾一个多余 LF。普通 cached whitespace 检查均通过，没有全局豁免或修改已封印负载。

三个仓库的验收、审查、普通提交与源码推送核对均已完成，用户原有关闭授权继续有效。GitHub 连接器的 issue PATCH 实际返回 403（`Resource not accessible by integration`）；后续读取 Git 凭据以调用 REST 的命令被自动审批拒绝，仅返回 `blocked by policy`，命令没有执行、没有取得或输出凭据。浏览器界面操作又因工具无法可靠确定当前 URL 而停止，已停止所有界面输入。

改用 GitHub 正式支持的提交关闭关键字：文档交付提交 [9e6ed7c](https://github.com/Nanobug-Editor/Editor/commit/9e6ed7c8e2135c0feeaa664669536c80dfc02bdb) 引用 `Closes #121`，普通推送默认 main，先核对远端 SHA，再经连接器实际读回本单 `closed / completed` 与上述关闭时间。该机制见[GitHub 官方说明](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue)；没有新增凭据、权限或 PR。远端 issue 正文的勾选未成功写入；已验收八项 AC 的本地工单与本记录保留准确结果。

本次完成只包括 #121。#122–#126 继续暂停，旧工单和父设计议题没有推进或关闭；没有创建 Release。主工作区既有 Excalidraw 改动保持原摘要，旧 worktree、冻结包与失败证据继续保留。最后的关闭回执和索引更新仅修改文档，不改变已验收实现；检查本地链接、JSON 与差异空白，不重复无关 Rust 构建。
