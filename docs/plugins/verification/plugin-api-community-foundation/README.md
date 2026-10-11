# 基础社区生态：实施与验收入口

日期：2026-10-12。状态：01–06 已验收交付，07 行为验收与最终独立双轴复核完成，待普通提交、推送与关闭。

本次恢复按用户新约定使用源码接口注释和不含 Markdown 的独立 SDK，保留宿主原生终端与公开执行/PTY 能力。主分支普通合并、源码包 Red/Green、四个正式独立包与当前原生连接均已通过，[2026-10-11 恢复记录](2026-10-11-resume.md)保存精确输入和结果；后续工单继续按依赖实施，历史包验证只适用于其记载的输入。

依据：[已批准方案](../../specs/plugin-api-community-foundation.md)、[七张工单及测试责任](../../tickets/plugin-api-community-foundation/README.md)、[实际议题映射](../../tickets/plugin-api-community-foundation/publication.json)。父设计议题 [#92](https://github.com/T-miracle/Nanobug/issues/92) 不随实施工单关闭。

## 工作树与审查基线

- 集成工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/Editor`，分支 `codex/plugin-api-community`。
- 批次固定基线：`452994e6d3d6da526803cc62db96ee722580e670`，只包含已批准方案、工单和发布映射。
- 02 工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-interaction/Editor`，分支 `codex/plugin-api-interaction`，从相同基线开始。
- 06 工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-cloud/Editor`，分支 `codex/plugin-api-cloud`，从已交付 02 的 `8506cc8f1f9baee3fca030d59b7d1d21b5227763` 开始。
- 03 工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-transactions/Editor`，分支 `codex/plugin-api-transactions`；保持行为的预重构从普通整合候选 `ef6fa2d` 开始，最终公开事务交付需接入下述已修正整合点。
- 05 工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-trees/Editor`，分支 `codex/plugin-api-trees`；从已验证并推送的整合点 `b54aa46931bc24021791456f47fbe894660742ce` 开始。
- 04 工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-language/Editor`，分支 `codex/plugin-api-language`；从已交付 03 的集成点 `3aa9177a20b428dbac5ebffbd0aa78e34a464eda` 开始。
- 原项目工作区已有其他任务的未提交改动，本批代码在独立工作树实施；这些已有改动不计入本批成果。
- 当前主分支加 01/02 的源码 SDK 整合点：`de5397645d05c5d91e8cd74108d46ad9d7a88ff3`，已普通推送并核对远端；当前包与原生连接、三项仓库检查和双轴复审均通过，03/05/06 普通合并该点后验证各自接缝。

## 当前进度

| 工单 | 议题 | 当前状态 | 交付条件 |
| --- | --- | --- | --- |
| 01 文档与只读资源 | [#93](https://github.com/T-miracle/Nanobug/issues/93) | 已验收交付，议题已关闭 | `7b50045a78dacafacb2bb7a76f0694c91a1b755d` 已推送并核对远端；双轴未解决项均为 0 |
| 02 命令与交互 | [#94](https://github.com/T-miracle/Nanobug/issues/94) | 已验收交付，议题已关闭 | `8506cc8f1f9baee3fca030d59b7d1d21b5227763` 已推送并核对远端；双轴未解决项均为 0 |
| 03 工作区事务 | [#95](https://github.com/T-miracle/Nanobug/issues/95) | 已验收交付，议题已关闭 | [03 验收记录](03-workspace-transactions.md)、[同一 profile 原生记录](03-workspace-transactions-native.md)；交付分支 `3205bb41f27f0ed61e6298c3c21bc5e7aa7e2ef5` 和集成点 `3aa9177a20b428dbac5ebffbd0aa78e34a464eda` 已推送并核对；既有 Settings UIA 退役崩溃仍按记录保留 |
| 04 诊断与语言 | [#96](https://github.com/T-miracle/Nanobug/issues/96) | 已验收交付，议题已关闭 | [04 主验收](04-diagnostics-and-language.md)、[双轴审查](04-diagnostics-and-language-review.md)、[生产原生记录](04-native-desktop.md)；`2d140d1c71d117046d907c600a18245f865dc696` 两分支已推送核对，2026-10-11T15:17:51Z 读回 closed/completed |
| 05 树与装饰 | [#97](https://github.com/T-miracle/Nanobug/issues/97) | 已验收交付，议题已关闭 | [05 验收记录](05-trees-and-decorations.md)、[修复版原生记录](05-trees-and-decorations-native.md)；交付 `694038055387d55513916d9781baefa9a970d271` 已普通推送核对，2026-10-11T11:01:31Z 读回 closed/completed；集成点联合验证另记 |
| 06 网络与私有状态 | [#98](https://github.com/T-miracle/Nanobug/issues/98) | 已验收交付，议题已关闭 | [06 验收记录](06-cloud-and-private-state.md)、[当前原生记录](native-06.md)；源码 `f2a907a`、最终文档分支 `72bdf8bab1c9f4d489118715f9f3a17c0f6d301c` 已推送核对；当前集成分支继续汇合，联合验证另记 |
| 07 激活与兼容 | [#99](https://github.com/T-miracle/Nanobug/issues/99) | 验收与双轴审查完成，待普通交付 | [07 主验收](07-activation-and-compatibility.md)、[双轴记录](07-activation-and-compatibility-review.md)、[Windows 原生记录](native-07.md)；Runtime 19、额外 Query 1、App 12 显式通过，非 UI 260 通过/246 ignored，三门禁 exit0，两轴未解决项0；远端尚未关闭 |

## 证据与复用规则

01 与 02 的[普通合并连接记录](01-02-integration.md)固定双方交付点，记录真实 SDK 菜单上下文与焦点回归、SDK / 包指纹及整合门禁。`325c9d7` 修正本地左栏精确目标，`91b7e09` 配对澄清两语文档；双轴未解决项均为 0，[新增原生连接](native-integration.md)通过。完整记录提交 `b54aa46931bc24021791456f47fbe894660742ce` 已普通推送，`git ls-remote` 核对远端同 SHA；后续 03 / 05 使用这个已修正整合点。批次其余工单没有因此标记完成。

01 的[逐单验收与双轴结论](01-documents-and-virtual-resources.md)及 [01 主代理原生验收](native-01.md)已经交付；2026-10-09 07:22:13 UTC，按既有 Git 认证更新并读回 #93 为 `closed / completed`。初轮失败保留原始结果，修复候选分别复核；父设计议题 #92 未修改。02 的原生证据见 [02 主代理原生验收](native-02.md)。

02 的[逐单验收与双轴结论](https://github.com/T-miracle/Nanobug/blob/8506cc8f1f9baee3fca030d59b7d1d21b5227763/docs/plugins/verification/plugin-api-community-foundation/02-commands-and-interaction.md)已经发布到独立分支，随后与 01 集成。2026-10-09 06:01:59 UTC，GitHub connector 更新返回 403；按既有工单授权改用既有 Git 认证直接更新，随即读回 #94 为 `closed / completed`。没有输出凭据或修改父设计议题。

2026-10-11 06:34:32 UTC，03 的两个普通推送均已读回完整 SHA，随后读取 #95 为 `closed / completed`。06 的当前 Native HTTP、密钥、缓存、取消及正常停止已记录；2026-10-11 06:28:28 UTC，读取 #98 为 `closed / completed`，交付状态随后用纯文档提交 `72bdf8b` 同步并推送。05 的修复版原生滚动、提示、重载与关闭验收已通过，交付 `694038055387d55513916d9781baefa9a970d271` 已推送核对，2026-10-11T11:01:31Z 读回 #97 为 `closed / completed`。上述关闭只适用于各自已验收输入；04 已随后完成联合回归、双轴与生产原生验收，普通交付 `2d140d1c71d117046d907c600a18245f865dc696` 已推送核对，2026-10-11T15:17:51Z 读回 #96 为 `closed / completed`。07 与父设计议题 #92 继续开放。

每单记录自己的实际源码版本、工具链、SDK、包版本与 hash、测试命令和结果、原生操作、审查结论及未验证部分。协议准入或某个子用例通过不等于整项验收通过，默认跳过的 ignored 测试也不计为通过。

详细用例依照工单目录的唯一主责分配；后单只增加连接测试，重跑实际修改所影响的既有用例。相同源码、SDK、清单、资源、工具链及参数的包可复用产物，每次运行使用独立数据目录。每单的三项仓库交付检查以及相关应用、原生和实际 WASM 验收仍须执行。

审查分别记录 Standards 和 Spec 两条轴，并使用固定提交范围。只有完整验收和审查通过、提交已推送并核对远端后，才更新对应实施议题为已关闭。最终组合验收归 07，不替代前六单的失败行为测试。
