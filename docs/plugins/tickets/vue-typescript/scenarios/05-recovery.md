# Vue/TS S5：在离线、更新或后端故障后恢复使用

状态：已完成；[#125](https://github.com/Nanobug-Editor/Editor/issues/125) 已于 2026-10-20 读回 completed。三仓普通提交／main 推送均已核对，见[交付回执](../../../verification/vue-typescript-remaining-delivery-2026-10-20.md)。

逐条实际证据见[覆盖映射](../../../verification/vue-typescript-remaining-coverage-2026-10-19.md)；旧工单及父议题保持原状。

<!-- nanobug-vue-typescript-scenes-2026-10-15:S5 -->

## Parent

[#107 Vue 多版本插件与 TypeScript 协作方案](https://github.com/Nanobug-Editor/Editor/issues/107)

## Progress preserved

场景拆分已于 2026-10-15 获用户批准。本单只承接剩余实现和验收缺口，保留原工单 [#109](https://github.com/Nanobug-Editor/Editor/issues/109)、[#110](https://github.com/Nanobug-Editor/Editor/issues/110)、[#112](https://github.com/Nanobug-Editor/Editor/issues/112)、[#114](https://github.com/Nanobug-Editor/Editor/issues/114)、[#117](https://github.com/Nanobug-Editor/Editor/issues/117) 的已有代码、候选产物、测试结果及失败记录；不把旧状态的通过报告成最终工作树通过。用户后续明确本轮只执行新工单 #121–#126；旧 #108–#118 仅保留历史记录，不再推进或关闭。父设计议题保持不变。

## What to build

准备失败、离线、包更新、某一后端崩溃及撤销授权时，用户看到准确状态，可保留或恢复已可用组合，无关项目继续工作。

## Why separate

恢复涉及受管依赖、安装事务、模块租约和上下文生命周期，具有独立可验证的失败注入场景；可以基于 S1 的实际包并行补齐。S4 的扩展更新与 S2 的脏编辑组合在 S6 集成验证，避免把建议顺序当成硬阻塞。

## Acceptance criteria

- [x] S5-AC1：候选准备失败保留旧实例；激活失败恢复旧包及受管私有数据，并可重新启动旧兼容组合。用真实更新/回滚流程验证，不声称恢复进程内存或跨包原子安装。
- [x] S5-AC2：TS/Vue 更新不兼容时保留可选择的可用组合或拒绝切换；不静默降低 Vue 2 等历史已验收能力。实际切换测试重跑保留矩阵，未知未来版本不自动获得正式支持。
- [x] S5-AC3：下载取消、坏摘要、首次/缓存/离线准备、并发去重、独立共享租约及回滚收据均有真实测试；不修改全局环境、项目依赖或静默升级插件。
- [x] S5-AC4：一个版本后端退出只影响所属上下文，重试有预算且可停止，另一 Vue 版本仍可继续补全/诊断；合法业务错误与取消不触发无关重启。
- [x] S5-AC5：导出方/接收方卸载、更新、崩溃、撤信任或工作区关闭时，先终止相关受管进程树，再释放租约；无旧通道、请求、映射、诊断和可用引用。多消费者及私有缓存清理有完整联验。
- [x] S5-AC6：配置或上下文变化只退休相关绑定，乱序准备不回写旧状态；关闭重开文档使用新身份，有限恢复后不泄漏上下文引用或积压通知。
- [x] S5-AC7：中英文阶段、失败原因、版本证据、受影响项目及恢复入口准确；正常请求不反复弹权限确认。验证准备、启动、索引、失败、恢复状态及相应原生交互。

## Verification

每项 AC 已写明可观察行为、自动化或原生验证及风险边界。依赖、更新事务、模块/消费者撤销与上下文故障回归，真实包故障注入、进程/租约归零、Windows 状态交互及仓库强制检查。S6 负责多场景叠加后的最终回归。

沿用公开 Package → Manager/SDK → 受管服务，以及原生文档/编辑事务等已确认测试接缝；复用既有测试不豁免仓库门禁。记录实际源码/产物、命令、结果和未验证部分，先测试与审查，再普通提交、推送并读回对应工单状态。不创建 Release 或发布版本。

## Blocked by

- [S1 #121](https://github.com/Nanobug-Editor/Editor/issues/121)
