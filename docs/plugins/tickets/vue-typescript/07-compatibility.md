# 07 历史写法与类型兼容

状态：已发布 [#114](https://github.com/Nanobug-Editor/Editor/issues/114)，待依赖验收。用户已授权自主安排顺序、并行实施与子代理；验收完成后才提交完成状态。

## What to build

为首批 Vue 版本建立按写法、工具和引擎组合区分的完整兼容矩阵，修复历史后端缺陷。类组件、装饰器、JSX/TSX 与 Vue 2 Composition API 扩展进入正式验收，不靠上游笼统支持声明代替结果。

## Acceptance criteria

- [ ] Vue 2.6、2.7、Vue 3 的 Options、Composition、script setup、JS/TS 按实际合法范围验收。
- [ ] 类组件、装饰器、JSX/TSX 各明确依赖范围，并验证七项语言能力及组件属性/插槽推断。
- [ ] 必要兼容修复归属 Vue 后端或公开契约，不写成宿主白名单。
- [ ] 实际产物、传递依赖、摘要与已知限制完整记账；生成缓存不写项目目录。
- [ ] Vue 3 或 TS 工具升级后保留 Vue 2 全部已验收行为；未知新版本不自动获得正式支持标记。

## Blocked by

06 跨文件重构与自动导入。
