# Vue/TS S1：安装真实插件并打开混合项目

状态：行为验收、正式基线门禁与双轴审查已通过，普通提交、三仓推送及远端 SHA 核对已完成；[#121](https://github.com/Nanobug-Editor/Editor/issues/121) 的关闭状态待读回。实际结果见[交付记录](../../../verification/vue-typescript-s1-delivery-2026-10-19.md)。

<!-- nanobug-vue-typescript-scenes-2026-10-15:S1 -->

## Parent

[#107 Vue 多版本插件与 TypeScript 协作方案](https://github.com/Nanobug-Editor/Editor/issues/107)

## Progress preserved

场景拆分已于 2026-10-15 获用户批准。本单只承接剩余实现和验收缺口，保留原工单 [#108](https://github.com/Nanobug-Editor/Editor/issues/108)、[#109](https://github.com/Nanobug-Editor/Editor/issues/109)、[#110](https://github.com/Nanobug-Editor/Editor/issues/110)、[#111](https://github.com/Nanobug-Editor/Editor/issues/111)、[#112](https://github.com/Nanobug-Editor/Editor/issues/112) 的已有代码、候选产物、测试结果及失败记录；不把旧状态的通过报告成最终工作树通过。用户后续明确本轮只执行新工单 #121–#126；旧 #108–#118 仅保留历史记录，不再推进或关闭。父设计议题保持不变。

## What to build

独立 TypeScript 插件可单独使用；安装 Vue 插件后，同一工作区的 Vue 2.6、2.7、3 子项目获得正确的首轮语言分析。先修复现有编译断点、公开源版本元数据和首次提供者附着失败，再从真实包进入公开宿主验证，不重新实现已有底座。

## Why separate

实际安装、上下文解析、受管适配和首次同步共同组成后续场景的真实前置条件；这些步骤共享同一调用链，合并验收可以避免分别证明通用机制后仍无法启动实际插件。

## Acceptance criteria

- [x] S1-AC1：当前宿主和 SDK 可编译，控制请求按用途选择目标快照或关联上下文快照；公开消费者使用原生文档身份/revision，不猜内部 LSP wire version。自动化验证源元数据封印、伪造元数据拒绝及 resolve/attach 不因自身改变项目图而失效。
- [x] S1-AC2：有配置与无配置 JS/TS 项目均有可解释上下文；Vue 停用时 TS 仍提供七项基础能力。通过真实包的宿主测试观察结果，复用并核对既有 TS rename 证据，记录实际引擎与项目配置。
- [x] S1-AC3：固定的 Vue 2.6.14、2.7.16、3.5.22 与 TS 5.9.3 候选组合同时打开，正确分离后端和项目配置；模板补全、诊断、悬浮、跳转、引用指向真实源位置，无重复诊断或虚拟 URI 泄漏。真实 production Worker 联验必须进入适配和分析阶段，不能以进程启动代替。
- [x] S1-AC4：版本证据、显式选择和主提供者选择可追溯；未知/冲突版本不套用最新版，新提供者不覆盖有效选择。测试规范项目根、合法链接及越界链接、实例归属与错误项目绑定；按需准备不在只读 hook 内进行。
- [x] S1-AC5：未知消费者能通过公开契约 resolve/describe/analyze/release，查询上下文、能力与引擎版本，不能借用提供者私有句柄或权限；缺失 TS、缺失/不兼容引擎和错误显式工具路径产生结构化错误，不私建 TS 引擎、不自动安装插件或静默更换工具。
- [x] S1-AC6：模块仅进入受管分析进程，完整闭包、摘要、入口、ABI、来源/接收方与权限均校验；加载用途和原生运行方式纳入安装/更新授权，明确模块间不具备 WASM 隔离。相同字节可复用缓存，各消费者授权独立，不兼容组合使用独立工作进程。复用真实未知模块测试，并补齐双方、多消费者及进程隔离验收。
- [x] S1-AC7：未知语言包仍可复用上下文及模块能力；所有宿主 API 通用，不引入 Vue/TS/插件 ID 专属分支。测试项目文件与全局环境不变，缓存和生成视图不写项目。
- [x] S1-AC8：控制通道协商协议版本与方法范围，版本不匹配明确拒绝，请求和响应准确关联；帧/并发/积压预算、背压、超时与取消可观察。业务错误和取消不破坏共享连接，大结果有界返回或显式失败。运行交错请求、边界及压力回归，文档正文继续走原生同步。

## Verification

每项 AC 已写明可观察行为、自动化或原生验证及风险边界。受影响协议、运行时、TS 插件、真实包与宿主定向测试，以及仓库规定的格式、非 UI workspace 测试和 workspace check；实际 WASM 用例显式运行 ignored。首轮状态及结果以 Windows 原生宿主观察。

沿用公开 Package → Manager/SDK → 受管服务，以及原生文档/编辑事务等已确认测试接缝；复用既有测试不豁免仓库门禁。记录实际源码/产物、命令、结果和未验证部分，先测试与审查，再普通提交、推送并读回对应工单状态。不创建 Release 或发布版本。

## Blocked by

None (can start immediately).
