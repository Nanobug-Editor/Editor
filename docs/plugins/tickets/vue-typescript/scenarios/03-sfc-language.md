# Vue/TS S3：使用首批 Vue 写法、SFC 高亮和块格式化

状态：已完成；[#123](https://github.com/Nanobug-Editor/Editor/issues/123) 已于 2026-10-20 读回 completed。三仓普通提交／main 推送均已核对，见[交付回执](../../../verification/vue-typescript-remaining-delivery-2026-10-20.md)。

逐条实际证据见[覆盖映射](../../../verification/vue-typescript-remaining-coverage-2026-10-19.md)；旧工单及父议题保持原状。

<!-- nanobug-vue-typescript-scenes-2026-10-15:S3 -->

## Parent

[#107 Vue 多版本插件与 TypeScript 协作方案](https://github.com/Nanobug-Editor/Editor/issues/107)

## Progress preserved

场景拆分已于 2026-10-15 获用户批准。本单只承接剩余实现和验收缺口，保留原工单 [#111](https://github.com/Nanobug-Editor/Editor/issues/111)、[#114](https://github.com/Nanobug-Editor/Editor/issues/114)、[#115](https://github.com/Nanobug-Editor/Editor/issues/115) 的已有代码、候选产物、测试结果及失败记录；不把旧状态的通过报告成最终工作树通过。用户后续明确本轮只执行新工单 #121–#126；旧 #108–#118 仅保留历史记录，不再推进或关闭。父设计议题保持不变。

## What to build

完成首批三种 Vue 候选组合的写法语义、编辑提案、SFC 高亮和块格式化；工具及受管包层分别验证七项能力，安全编辑的宿主事务组合与正式支持标记须在 S2+S3 的 S6 联合验收通过后完成。

## Why separate

本场景共享兼容夹具、后端映射和 SFC 块处理，合并旧 07/08 的剩余工作；其主要实现位于独立 Vue 包，可以在稳定的 S1 契约上与重构、生态协作并行。复杂重构沿用 S2 的通用事务，最终组合在 S6 验证，不把 S2 机械设为本场景全部工作开始前的阻塞。

## Acceptance criteria

- [x] S3-AC1：Options/Composition、script setup、JS/TS 及 Vue 2 Composition API 扩展的每个声明组合均按目标版本合法范围覆盖七项适用语言能力；非法语法正确诊断，合法样例无误报。每项固定依赖组合及适用/不适用原因。
- [x] S3-AC2：类组件、装饰器、JSX/TSX 各自覆盖七项语言能力、props/slots 推断、自动导入及跨文件引用/重命名。既有 props 样例作为回归，不外推成全部能力已验收；S3 验证工具/受管包的语义与编辑提案，历史组合的安全应用、预览/取消/Undo 和正式支持标记须等待 S2+S3 在 S6 联合通过。
- [x] S3-AC3：外部 JS/TS src、不同合法导出形式与路径解析不依赖开发目录；对不支持的动态语义给出准确限制或完成补充实现，不把有限静态样例泛称全覆盖。密封受管 loader 与真实包均有证据。
- [x] S3-AC4：宿主实际 Tree-sitter loader 加载插件 WASM grammar 和查询；Vue、JS/TS/JSX/TSX、CSS/SCSS/Sass/Less 的注入及可见高亮正确，停用后撤销。缺少块 grammar 时不冒充其他语言。
- [x] S3-AC5：整文件与适用块范围格式化保留 SFC 结构、属性及外部 src；CSS/SCSS/Sass/Less 的已承诺块能力通过。独立格式化选择与配置生效，安装 Vue 不覆盖有效提供者；测试脏文本、CRLF/Unicode 和过期结果全拒绝。
- [x] S3-AC6：缺失块语言或专门插件时准确显示能力不足，其他能力继续；深浅主题、焦点、键盘、中文 IME、滚动及缩放按改动范围原生验收。
- [x] S3-AC7：兼容修复属于插件或通用公开契约，不写宿主白名单；受管闭包不命中开发依赖，来源、许可、传递依赖、摘要、生成缓存与真实包对应。工具或插件升级后的保留回归由 S5/S6 验证。

## Verification

每项 AC 已写明可观察行为、自动化或原生验证及风险边界。Vue/TS 工具层兼容全集、密封 loader、宿主 grammar 和 SFC 定向测试，真实包原生输入/可见结果及强制检查。范围格式化和历史完整能力是现存缺口，不能降格为仅高亮或诊断支持。

沿用公开 Package → Manager/SDK → 受管服务，以及原生文档/编辑事务等已确认测试接缝；复用既有测试不豁免仓库门禁。记录实际源码/产物、命令、结果和未验证部分，先测试与审查，再普通提交、推送并读回对应工单状态。不创建 Release 或发布版本。

## Blocked by

- [S1 #121](https://github.com/Nanobug-Editor/Editor/issues/121)
