# Vue 与 TypeScript 实施工单

> **2026-10-20 23:47 旧工单关闭回执：** 用户明确要求确认功能完成后关闭旧工单。核对 64 项旧要求由已交付六场景的 43 项验收承接后，旧 #108–#118 已全部读回 completed；原正文、进度及失败记录保留，父 #107 仍开放。详见[核对与实际回执](../../verification/vue-typescript-legacy-closure-2026-10-20.md)。下方“不再关闭旧工单”段落是此前历史授权，已由本次明确授权覆盖；新场景完成状态不变。

> **2026-10-20 新六场景 6/6 交付完成。** #122–#126 已完成实现、最终双包／Windows 验收、三仓普通提交与 main 推送，并实际读回 completed；35 条要求／14 项矩阵见[交付记录](../../verification/vue-typescript-remaining-delivery-2026-10-20.md)和[覆盖映射](../../verification/vue-typescript-remaining-coverage-2026-10-19.md)。#121 既有完成状态保留；旧 #108–#118 及父 #107 的正文、状态和更新时间不变。未创建 Release。

> **2026-10-20 15:00 已按用户“继续执行”恢复新 #122–#126。** 三仓全部暂停字节与状态核对一致，沿用既有范围及交付授权，见[恢复记录](../../verification/vue-typescript-resumed-2026-10-20.md)。下方暂停段落保留为历史检查点，不覆盖此次继续指令；已通过的专项不重列未执行，未完成的最终交付不提前关闭。

> **2026-10-20 05:08 最新状态：全部工单已按最新要求暂停。** 三仓未提交实现、既有通过和失败全部保留，恢复入口为[暂停检查点](../../verification/vue-typescript-paused-2026-10-20-0508.md)。#121 既有交付保留；本轮未提交、推送或关闭 #122–#126，旧工单及父议题不变。下方“连续实施／待执行”描述是此前时点，不授权自动恢复。

> **2026-10-20 当前状态：新工单 [#121 / S1](scenarios/01-real-projects.md) 已验收、完成三仓推送并读回关闭；用户通过 `execute-plan` 恢复全部剩余新工单 #122–#126，连续实施已跨日。旧 #108–#118 只作历史，不执行、不改写、不关闭。** 本轮连续实施见[剩余场景记录](../../verification/vue-typescript-remaining-2026-10-19.md)，S1 证据见[交付记录](../../verification/vue-typescript-s1-delivery-2026-10-19.md)。
>
> 下方保留 2026-10-15 原索引及进度；历史暂停状态不能覆盖上述最新授权。场景表随实际实施更新，只有验收和推送完成才更新验收勾选及关闭回执，全部要求保留。

## 2026-10-15 保存的历史索引

日期：2026-10-15。

当前执行状态：用户已批准[剩余工作六场景方案](replan-proposal.md)，正式场景工单 #121–#126 已发布，父子与原生阻塞关系已读回核对，见[场景 publication 映射](scenarios/publication.json)。22:51 按最新指令暂停所有工单，原代码、候选包、测试与失败证据均保留，见[最新暂停检查点](../../verification/vue-typescript-paused-2026-10-15-2251.md)。恢复时只执行新 #121–#126；旧 #108–#118 保留历史进度，不再推进或关闭。

## 剩余场景执行入口

| 场景 | 正式工单 | 必要阻塞 | 当前状态 |
| --- | --- | --- | --- |
| S1 | [安装真实插件并打开混合项目](scenarios/01-real-projects.md)（[#121](https://github.com/Nanobug-Editor/Editor/issues/121)） | 无 | 已完成；三仓推送及关闭已核对，见[记录](../../verification/vue-typescript-s1-delivery-2026-10-19.md) |
| S2 | [在混合项目中编辑、补全和跨文件重构](scenarios/02-edit-and-refactor.md)（[#122](https://github.com/Nanobug-Editor/Editor/issues/122)） | #121 | 已完成；三仓提交／推送和 completed 关闭读回已核对 |
| S3 | [使用首批 Vue 写法、SFC 高亮和块格式化](scenarios/03-sfc-language.md)（[#123](https://github.com/Nanobug-Editor/Editor/issues/123)） | #121 | 已完成；固定范围验收、三仓推送和 completed 读回已核对 |
| S4 | [安装专门生态插件参与 Vue 分析](scenarios/04-ecosystem.md)（[#124](https://github.com/Nanobug-Editor/Editor/issues/124)） | #121 | 已完成；四角色／冲突／撤销验收及交付读回已核对 |
| S5 | [在离线、更新或后端故障后恢复使用](scenarios/05-recovery.md)（[#125](https://github.com/Nanobug-Editor/Editor/issues/125)） | #121 | 已完成；内外层恢复／回滚／离线及交付读回已核对 |
| S6 | [完成跨插件联合验收与可审核交付](scenarios/06-joint-delivery.md)（[#126](https://github.com/Nanobug-Editor/Editor/issues/126)） | #122、#123、#124、#125 | 已完成；依赖、新双包、普通桌面、双轴及三仓 Git／关闭回执全部闭合 |

以下旧工单仅保留原编号与历史证据，后续不再执行或关闭；剩余工作由新 #121–#126 承接，映射仅供追溯。已验证、已写未验证与尚未实现的证据不混用。

状态：已发布父议题 [#107](https://github.com/Nanobug-Editor/Editor/issues/107) 与实施议题 #108–#118，并写入 [publication 映射](publication.json)，原生阻塞关系已建立。实施依据为[已确认方案](../../specs/vue-multi-backend.md)。用户已授权自主选择实施顺序、并行和子代理；正式插件、宿主与原生联合验收仍在进行，不继承其他任务的完成状态。

每张工单以可观察行为验收，包含公开契约、运行时/插件接入与回归。当前按 S1–S6 推进剩余场景，依赖未验收时允许准备独立实现，但不提前关闭下游工单。

| 序号 | 工单 | 阻塞项 | 端到端交付 |
| --- | --- | --- | --- |
| 01 | [TypeScript 单项目分析与公开上下文](01-typescript-project.md) | 无 | 安装独立 TS 插件后，JS/TS 语言能力可用，Vue 消费者能查询同一上下文 |
| 02 | [子项目语言绑定与按需准备](02-project-bindings.md) | 01 | 同工作区的不同子项目使用各自服务与工具计划 |
| 03 | [适配模块加载与联合撤销](03-adapter-lifecycle.md) | 01 | TS 工作进程加载其他插件导出的不可变模块，并正确撤销 |
| 04 | [Vue 多后端首次宿主分析](04-vue-analysis.md) | 02、03 | 一个 Vue 插件在宿主中为 Vue 2.6、2.7、3 选择对应后端并返回基础语言结果 |
| 05 | [关联脏文档同步与结果隔离](05-unsaved-documents.md) | 04 | Vue 与 TS 分析读取相同未保存快照，拒绝过期结果 |
| 06 | [跨文件重构与自动导入](06-source-edits.md) | 05 | 映射后的源文件编辑完整预览、提交、取消与分组撤销 |
| 07 | [历史写法与类型兼容](07-compatibility.md) | 06 | 类组件、装饰器、JSX/TSX 等首批写法逐项验收 |
| 08 | [SFC 高亮与块格式化](08-sfc-formatting.md) | 04 | SFC 及 CSS/SCSS/Sass/Less 按公开能力高亮与格式化 |
| 09 | [专门生态插件组合](09-ecosystem.md) | 03、05 | 未知生态测试插件可参与转换/配置/类型贡献，不修改宿主路由 |
| 10 | [更新与上下文故障恢复](10-recovery.md) | 02、03、05 | 更新、离线、撤权和失败恢复不破坏其他上下文或历史组合 |
| 11 | [首批联合验收与交付准备](11-acceptance.md) | 06、07、08、09、10 | 真实包、公开管理器和 Windows 原生交互通过完整矩阵 |

工具层初步证据见[2026-10-15 P0 记录](../../verification/vue-typescript-p0-2026-10-15.md)。该记录中的直接 tsserver 结果不代替正式插件或宿主验收；组件属性差异、历史工具写入生成类型文件等问题必须在对应工单处理。

按[议题工作流](../../../agents/issue-tracker.md)执行测试、两轴审查、普通提交、推送及对应议题状态读回。未经实际推送与验收，不标记工单完成。父设计议题保持开放；版本发布未获授权。
