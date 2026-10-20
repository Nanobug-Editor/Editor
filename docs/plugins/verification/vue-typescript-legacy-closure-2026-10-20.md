# Vue / TypeScript 旧工单功能核对与关闭

> **实际终态：旧 #108–#118 共 11 张均已读回 closed/completed，关闭于 2026-10-20 23:46:55–23:47:02（Asia/Shanghai）。** 父 #107 继续开放；所有原 issue 正文及标题保持原样，#121–#126 状态不变。以下保留关闭前核对，实际回执见末节。

日期：2026-10-20，Asia/Shanghai。

用户在六个新场景完成交付后，明确要求“如果确认功能已完成，关闭这些工单”。本记录据此核对旧 #108–#118；此前“不再推进或关闭旧工单”的历史限制由本次明确关闭授权覆盖。父 #107 继续开放；不重写原方案、原进度及失败记录，不执行新的功能实现，不创建 Release。

## 核对结论

已逐张读取 GitHub 当前旧工单 Acceptance criteria，并对照用户批准的[旧验收项逐项承接](../tickets/vue-typescript/replan-proposal.md#旧验收项逐项承接)、新六场景和实际交付记录核对。旧 **64 项要求均有承接且在已约定的首批范围内完成**；新 S1 的 8 条和 S2–S6 的 35 条，共 **43 条**均有验收记录。GitHub #121–#126 已实际读回 closed/completed，关闭前旧 #108–#118 均为 open，随后由本次普通提交关联关闭；当前实际终态见末节。

此判断不只来自勾选或 issue 状态：已读取[首轮真实包与混合项目交付](vue-typescript-s1-delivery-2026-10-19.md)、[最终 35AC／14V 证据映射](vue-typescript-remaining-coverage-2026-10-19.md)、[最终三仓／双包／Windows 交付](vue-typescript-remaining-delivery-2026-10-20.md)和[独立双轴审查](vue-typescript-remaining-review-2026-10-20.md)。旧要求中的语言能力、权限及上下文隔离、脏文档、历史写法、原生事务、生态映射、回滚／退出恢复和构建交付分别对应这些记录，原失败和重试按各自源码／产物时点保留。

## 原 AC 顺序到已交付场景的逐项映射

AC 序号为旧工单原始列表顺序，与已批准承接表一致。多处承接共同证明该要求；不是只验证其中一项便计完成。

| 旧工单 | AC 数 | 原 AC → 新场景 AC | 已交付承接工单 |
| --- | ---: | --- | --- |
| [#108](https://github.com/Nanobug-Editor/Editor/issues/108) | 6 | 1→S1-AC2；2→S1-AC2；3→S1-AC5；4→S1-AC5；5→S2-AC5/S2-AC6/S3-AC2；6→S1-AC7/S3-AC7 | [#121](https://github.com/Nanobug-Editor/Editor/issues/121)、[#122](https://github.com/Nanobug-Editor/Editor/issues/122)、[#123](https://github.com/Nanobug-Editor/Editor/issues/123) |
| [#109](https://github.com/Nanobug-Editor/Editor/issues/109) | 6 | 1→S1-AC3；2→S1-AC4/S1-AC7；3→S1-AC4/S2-AC1；4→S1-AC4/S5-AC3；5→S2-AC2/S5-AC6；6→S1-AC7 | [#121](https://github.com/Nanobug-Editor/Editor/issues/121)、[#122](https://github.com/Nanobug-Editor/Editor/issues/122)、[#125](https://github.com/Nanobug-Editor/Editor/issues/125) |
| [#110](https://github.com/Nanobug-Editor/Editor/issues/110) | 6 | 1→S1-AC6；2→S1-AC6；3→S1-AC8；4→S1-AC6/S5-AC2/S5-AC5；5→S5-AC5；6→S1-AC6/S1-AC7/S5-AC3 | [#121](https://github.com/Nanobug-Editor/Editor/issues/121)、[#125](https://github.com/Nanobug-Editor/Editor/issues/125) |
| [#111](https://github.com/Nanobug-Editor/Editor/issues/111) | 6 | 1→S1-AC3；2→S1-AC4；3→S1-AC3/S2-AC4；4→S1-AC5；5→S3-AC2；6→S5-AC5 | [#121](https://github.com/Nanobug-Editor/Editor/issues/121)、[#122](https://github.com/Nanobug-Editor/Editor/issues/122)、[#123](https://github.com/Nanobug-Editor/Editor/issues/123)、[#125](https://github.com/Nanobug-Editor/Editor/issues/125) |
| [#112](https://github.com/Nanobug-Editor/Editor/issues/112) | 6 | 1→S2-AC1；2→S1-AC1/S1-AC3/S2-AC4；3→S2-AC2；4→S2-AC3；5→S2-AC4；6→S2-AC1/S2-AC8 | [#121](https://github.com/Nanobug-Editor/Editor/issues/121)、[#122](https://github.com/Nanobug-Editor/Editor/issues/122) |
| [#113](https://github.com/Nanobug-Editor/Editor/issues/113) | 6 | 1→S2-AC6；2→S2-AC5；3→S2-AC7；4→S2-AC6/S2-AC8；5→S2-AC7；6→S2-AC8 | [#122](https://github.com/Nanobug-Editor/Editor/issues/122) |
| [#114](https://github.com/Nanobug-Editor/Editor/issues/114) | 5 | 1→S3-AC1；2→S3-AC2；3→S3-AC7；4→S3-AC7/S6-AC4；5→S5-AC2/S6-AC4/S6-AC6 | [#123](https://github.com/Nanobug-Editor/Editor/issues/123)、[#125](https://github.com/Nanobug-Editor/Editor/issues/125)、[#126](https://github.com/Nanobug-Editor/Editor/issues/126) |
| [#115](https://github.com/Nanobug-Editor/Editor/issues/115) | 5 | 1→S3-AC4；2→S3-AC5；3→S3-AC5；4→S3-AC6；5→S3-AC6/S2-AC2 | [#122](https://github.com/Nanobug-Editor/Editor/issues/122)、[#123](https://github.com/Nanobug-Editor/Editor/issues/123) |
| [#116](https://github.com/Nanobug-Editor/Editor/issues/116) | 5 | 1→S4-AC1；2→S4-AC1；3→S4-AC3；4→S4-AC4/S4-AC5；5→S4-AC2/S4-AC5/S4-AC6 | [#124](https://github.com/Nanobug-Editor/Editor/issues/124) |
| [#117](https://github.com/Nanobug-Editor/Editor/issues/117) | 6 | 1→S5-AC1；2→S5-AC2；3→S5-AC3；4→S5-AC4；5→S5-AC5；6→S5-AC7 | [#125](https://github.com/Nanobug-Editor/Editor/issues/125) |
| [#118](https://github.com/Nanobug-Editor/Editor/issues/118) | 7 | 1→S6-AC1；2→S6-AC2；3→S6-AC3；4→S6-AC4；5→S6-AC5；6→S6-AC4/S6-AC6；7→S6-AC7 | [#126](https://github.com/Nanobug-Editor/Editor/issues/126) |
| 合计 | **64** | **无缺失映射、无未验收承接项** | **#121–#126** |

核对工具还逐项验证旧正文 AC 数与映射长度一致、每个被引用的新 AC 已验收，未以文件名或编号猜测覆盖关系。实际语义和原生行为证据以上述映射及交付文档为准，不将计数校验本身当作动态测试。

## 对应实现与支持边界

实际已推送的实现为 Editor `d80c5708d7ce5e016e5e1a30e40c4b5c0b77b813`、TypeScript `2bc5be0301fc485d3b684641104e4dd596d3456e`、Vue `173c265e4c19298ca2fdb86b849ae90b6e3e4043`；Editor 后续纯文档回执 `e705747bc6c898b22d120e3ba8c924504d8173ff` 已推送。原始包、SDK 和运行身份沿用最终交付记录，不重建或更改产物。

正式范围仍为 **Windows x86-64 / TypeScript 5.9.3 / Node 24.19.0 / Vue 2.6.14、2.7.16、3.5.22 的 26 个固定合法写法组合**。更早或未来版本、其他未验收版本／平台、HTML/CDN 以及专门生态业务插件不因旧工单关闭而自动获得支持。已知 AccessKit 窗口退出 abort 保留，不声称本次修复或普通退出 exit0。父 #107 继续承载长期目标。

## 执行与校验

本轮只有文档及工单元数据变化，不运行无关完整 Rust 构建，不将未执行测试写成新通过。检查本地链接、JSON 元数据、映射完整性及 `git diff --check`。用户主工作区存在其他任务的改动；本次仅使用现有干净集成工作树，未暂存、合并或覆盖主工作区。

GitHub 连接器读取正常，但直接更新 #108 返回 **403 Resource not accessible by integration**，没有关闭或改写该 issue。本次沿用已授权且先前 #122–#126 验证成功的普通提交关联机制：把本核对记录推送 origin/main，以精确 `Closes #108` 至 `Closes #118` footer 关联已验收功能；再核对远端 SHA 并逐项读回 state/state_reason/closed_at。未经实际读回，不把关闭意图写成成功回执。原 issue 正文保留，不额外发送评论。

上文为关闭前核对记录；实际读回结果已追加如下。本页只关闭上述十一张旧实施工单，父 #107 与已完成 #121–#126 不改写。

## 实际关闭回执

普通提交 [`ab549c26b86c9c1b41f05e686c87eabf1ccb53b7`](https://github.com/Nanobug-Editor/Editor/commit/ab549c26b86c9c1b41f05e686c87eabf1ccb53b7) 只新增本核对文档，已正常推送 origin/main，并用 ls-remote 核对同一完整 SHA。其 footer 仅关闭 #108–#118；核对远端后，再通过 GitHub 连接器逐项实际读取终态：

| 旧工单 | 实际 state / state_reason | 关闭时间（Asia/Shanghai） |
| --- | --- | --- |
| [#108](https://github.com/Nanobug-Editor/Editor/issues/108) | closed / completed | 2026-10-20 23:46:55 |
| [#109](https://github.com/Nanobug-Editor/Editor/issues/109) | closed / completed | 2026-10-20 23:46:57 |
| [#110](https://github.com/Nanobug-Editor/Editor/issues/110) | closed / completed | 2026-10-20 23:46:57 |
| [#111](https://github.com/Nanobug-Editor/Editor/issues/111) | closed / completed | 2026-10-20 23:46:58 |
| [#112](https://github.com/Nanobug-Editor/Editor/issues/112) | closed / completed | 2026-10-20 23:46:58 |
| [#113](https://github.com/Nanobug-Editor/Editor/issues/113) | closed / completed | 2026-10-20 23:46:59 |
| [#114](https://github.com/Nanobug-Editor/Editor/issues/114) | closed / completed | 2026-10-20 23:47:00 |
| [#115](https://github.com/Nanobug-Editor/Editor/issues/115) | closed / completed | 2026-10-20 23:47:00 |
| [#116](https://github.com/Nanobug-Editor/Editor/issues/116) | closed / completed | 2026-10-20 23:47:01 |
| [#117](https://github.com/Nanobug-Editor/Editor/issues/117) | closed / completed | 2026-10-20 23:47:02 |
| [#118](https://github.com/Nanobug-Editor/Editor/issues/118) | closed / completed | 2026-10-20 23:47:02 |

所有十一张旧工单的 body 和 title 均与关闭前原文逐字符串相等；未清空原进度、未改写旧失败，也未重新勾选历史正文。父 #107 以及新 #121–#126 共七个对照项的 state、state_reason、updated_at 和 body 前后完全相同。父议题继续开放，新六场景保持此前 completed。

实际机器收据位于 `target/vue-typescript-legacy-closure-receipt.json`；关闭 commit、远端核对、64→43 承接和完整终态分别记账。旧工单按 **功能已完成** 关闭，功能由新场景交付；并非取消需求。更早／未来版本及已知平台限制仍按原长期计划跟踪。本轮后续只同步 publication 与索引，不改变功能、包或 SDK，不创建 Release。
