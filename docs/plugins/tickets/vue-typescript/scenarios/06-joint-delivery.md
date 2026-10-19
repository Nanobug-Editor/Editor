# Vue/TS S6：完成跨插件联合验收与可审核交付

状态：ready-for-agent；已发布 [#126](https://github.com/Nanobug-Editor/Editor/issues/126)，原生阻塞关系与父子关联已读回核对。等待依赖验收，可准备独立实现，不提前关闭。

<!-- nanobug-vue-typescript-scenes-2026-10-15:S6 -->

## Parent

[#107 Vue 多版本插件与 TypeScript 协作方案](https://github.com/Nanobug-Editor/Editor/issues/107)

## Progress preserved

场景拆分已于 2026-10-15 获用户批准。本单只承接剩余实现和验收缺口，保留原工单 [#108](https://github.com/Nanobug-Editor/Editor/issues/108)、[#109](https://github.com/Nanobug-Editor/Editor/issues/109)、[#110](https://github.com/Nanobug-Editor/Editor/issues/110)、[#111](https://github.com/Nanobug-Editor/Editor/issues/111)、[#112](https://github.com/Nanobug-Editor/Editor/issues/112)、[#113](https://github.com/Nanobug-Editor/Editor/issues/113)、[#114](https://github.com/Nanobug-Editor/Editor/issues/114)、[#115](https://github.com/Nanobug-Editor/Editor/issues/115)、[#116](https://github.com/Nanobug-Editor/Editor/issues/116)、[#117](https://github.com/Nanobug-Editor/Editor/issues/117)、[#118](https://github.com/Nanobug-Editor/Editor/issues/118) 的已有代码、候选产物、测试结果及失败记录；不把旧状态的通过报告成最终工作树通过。用户后续明确本轮只执行新工单 #121–#126；旧 #108–#118 仅保留历史记录，不再推进或关闭。父设计议题保持不变。

## What to build

在同一宿主、SDK、真实插件包及 Windows 环境下，完整证明首批范围可用，交付准确的支持清单、构建产物、审查记录及推送结果。

## Why separate

存在独立集成门槛：SFC 历史矩阵、跨文件事务、生态转换和更新恢复会共同改变项目图、映射及实例代次，单场景通过不足以证明其组合正确。本单承担跨工单验证与最终交付，不另拆纯格式检查或单个测试工单。

## Acceptance criteria

- [ ] S6-AC1：原方案 V01–V14 全部适用场景有明确的命令、实际结果、源码/产物版本、摘要和原生证据。覆盖 S2+S3 的历史写法重构、S2+S4 的转换映射事务、S2/S4+S5 的更新或撤销期间旧结果全拒绝。
- [ ] S6-AC2：受影响定向测试、`cargo fmt --check`、`cargo test --workspace --exclude editor-app` 和 `cargo check --workspace` 通过；涉及真实 WASM 的 ignored 用例构建后显式执行。未运行、忽略或失败不记作通过。
- [ ] S6-AC3：真实编辑、补全选择、诊断、悬浮、跳转、引用、重命名预览/取消/撤销、格式化及混合项目上下文切换有 Windows 原生输入与可见结果验收，不能只凭编译或工具层测试。
- [ ] S6-AC4：两个独立插件从当前宿主内嵌 SDK 缓存构建，不复制 SDK、不依赖宿主内部路径、不调用历史 scripts；清单版本、能力/协议版本、Cargo.lock、归档摘要、许可与兼容账本对应实际产物。支持范围如实区分正式、基础、待适配和限制，早期及未来版本仍按已确认长期准入保留和扩展，不冒称全部已支持。
- [ ] S6-AC5：完成规范与规格双轴审查；每个发现有修复或明确依据。本任务变更与原始工作区其他改动隔离；最终检查覆盖最终源码而非旧候选包。
- [ ] S6-AC6：历史后端、离线和回滚行为文档准确，已有用户数据保留；未实测平台明确列为未验收，后续增加版本须跑保留矩阵。
- [ ] S6-AC7：逐项记录新 #121–#126 的验收与提交关系，普通提交后推送 origin、核对远程并读回对应 issue 状态；只关闭已完成的新工单，旧 #108–#118 不再推进或关闭，父设计议题保持不变。交付构建候选，不创建 Release 或发布版本。

## Verification

每项 AC 已写明可观察行为、自动化或原生验证及风险边界。执行 AC2 的全部仓库强制检查，并按 AC1/AC3 验证原生交互与跨场景组合；未执行与失败如实记录。

沿用公开 Package → Manager/SDK → 受管服务，以及原生文档/编辑事务等已确认测试接缝；复用既有测试不豁免仓库门禁。记录实际源码/产物、命令、结果和未验证部分，先测试与审查，再普通提交、推送并读回对应工单状态。不创建 Release 或发布版本。

## Blocked by

- [S2 #122](https://github.com/Nanobug-Editor/Editor/issues/122)
- [S3 #123](https://github.com/Nanobug-Editor/Editor/issues/123)
- [S4 #124](https://github.com/Nanobug-Editor/Editor/issues/124)
- [S5 #125](https://github.com/Nanobug-Editor/Editor/issues/125)
