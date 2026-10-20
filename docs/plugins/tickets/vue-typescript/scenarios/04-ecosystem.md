# Vue/TS S4：安装专门生态插件参与 Vue 分析

状态：已完成；[#124](https://github.com/Nanobug-Editor/Editor/issues/124) 已于 2026-10-20 读回 completed。三仓普通提交／main 推送均已核对，见[交付回执](../../../verification/vue-typescript-remaining-delivery-2026-10-20.md)。

逐条实际证据见[覆盖映射](../../../verification/vue-typescript-remaining-coverage-2026-10-19.md)；旧工单及父议题保持原状。

<!-- nanobug-vue-typescript-scenes-2026-10-15:S4 -->

## Parent

[#107 Vue 多版本插件与 TypeScript 协作方案](https://github.com/Nanobug-Editor/Editor/issues/107)

## Progress preserved

场景拆分已于 2026-10-15 获用户批准。本单只承接剩余实现和验收缺口，保留原工单 [#110](https://github.com/Nanobug-Editor/Editor/issues/110)、[#112](https://github.com/Nanobug-Editor/Editor/issues/112)、[#116](https://github.com/Nanobug-Editor/Editor/issues/116) 的已有代码、候选产物、测试结果及失败记录；不把旧状态的通过报告成最终工作树通过。用户后续明确本轮只执行新工单 #121–#126；旧 #108–#118 仅保留历史记录，不再推进或关闭。父设计议题保持不变。

## What to build

宿主从未认识的扩展插件可通过公开角色贡献转换、配置和类型等能力，安装、替换及移除后 Vue/TS 正确组合并撤销结果。

## Why separate

真实贡献协议尚未完成，与纯消费者不同；它有独立插件安装和可观察转换效果，适合独立交付。对现有公开项目契约作兼容扩展，避免阻塞 S2/S3 的既有调用路径。

## Acceptance criteria

- [x] S4-AC1：竞争单选提供者与独立可组合扩展分别发现；声明模板转换、样式处理、项目配置和类型适配角色。异步准备阶段汇集贡献，不在只读语言 hook 调服务、不把普通 Open 当广播。
- [x] S4-AC2：至少一个真实未知贡献插件产生可观察的转换/配置/类型贡献，并用角色夹具覆盖组合与竞争；不把现有 A/B 纯消费调用作为贡献验收。配置与生成文件不写用户项目。
- [x] S4-AC3：转换携带源快照与映射，组合后诊断、导航和安全编辑仍回到准确源位置；不可逆或过期必要映射全拒绝。自动化组合映射、乱序结果及原生结果观察，S6 验证与 S2 事务完整组合。
- [x] S4-AC4：模块集合可协商，ABI、引擎、顺序环或配置冲突显式报告，不按安装顺序覆盖；有竞争、兼容组合、不兼容组合与错误配置回归。
- [x] S4-AC5：未知插件安装、替换、移除不修改宿主或 Vue 核心路由；旧调用、映射、通道和模块进程退休，其他有效消费者保留。A/B 原始消费者退出与多跳调用链纳入生命周期回归。
- [x] S4-AC6：缺少专门插件只降级受影响角色，明确状态并保留其他功能；本场景不扩大为 Pug/Stylus/Nuxt 正式业务插件，后者仍由独立任务实现。

## Verification

每项 AC 已写明可观察行为、自动化或原生验证及风险边界。公开契约、组合/冲突与租约定向测试；独立构建的未知贡献包经公开 Manager 及宿主显式 ignored 联验；原生结果、强制检查和无专属分支审查。

沿用公开 Package → Manager/SDK → 受管服务，以及原生文档/编辑事务等已确认测试接缝；复用既有测试不豁免仓库门禁。记录实际源码/产物、命令、结果和未验证部分，先测试与审查，再普通提交、推送并读回对应工单状态。不创建 Release 或发布版本。

## Blocked by

- [S1 #121](https://github.com/Nanobug-Editor/Editor/issues/121)
