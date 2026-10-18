# Vue 与 TypeScript 工具层协作初步验证

日期：2026-10-15。

状态：已启动实施，工具层探针通过；完整 P0、正式插件、公开宿主接入与原生 UI 尚未验收。依据为[多后端方案](../specs/vue-multi-backend.md)；正式切片见[工单拆分草案](../tickets/vue-typescript/README.md)。

## 范围与环境

在隔离的 `target/vue-typescript-p0/` 中运行真实 TypeScript 服务和不同版本的 Vue TS 适配模块。三个项目并行使用独立进程，通过 tsserver 的公开协议打开未保存文本、查询语言结果并更新关联 TS 文件。没有对用户实际项目运行工具，也没有修改宿主 Rust 实现或创建正式插件包。

开始时宿主 HEAD 为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`，工作区已有大量未提交改动，本记录不将其算入本次成果。

| 工具或输入 | 实际版本 |
| --- | --- |
| Windows Node | v24.19.0，来自已配置 Codex runtime |
| pnpm | 11.25.0，调用该 runtime 已有的入口；未安装或升级包管理器 |
| Rust / Cargo | rustc 1.98.1、cargo 1.98.1；已安装 wasm32-wasip2 与 Windows MSVC target；本次未编译 |
| TypeScript | 5.9.3 |
| Vue 2.6 项目 | vue 2.6.14；该实验另固定 @vue/runtime-dom 3.2.47 |
| Vue 2.7 项目 | vue 2.7.16 |
| Vue 3 项目 | vue 3.5.22 |
| 历史 Vue TS 适配模块 / 服务 | @vue/typescript-plugin、@vue/language-server 3.0.11 |
| Vue 3 TS 适配模块 / 服务 | @vue/typescript-plugin、@vue/language-server 3.3.13 |

工具版本均固定，传递依赖的版本与完整性记录在实验锁文件中。这里安装语言服务包作为实验输入，但实际执行和验证的是 tsserver 与 TS 适配模块，没有把 Vue LSP 的启动或完整转发列为已通过。

## 实际命令与结果

从隔离实验目录调用已存在的 pnpm 入口：

```powershell
# 只准备实验依赖，禁用依赖安装脚本；不更新全局工具。
node C:/Users/Tmiracle/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/pnpm/bin/pnpm.mjs install --ignore-scripts --config.node-linker=hoisted --config.strict-peer-dependencies=false --reporter=append-only
# 运行可观察的真实协议探针；三个独立服务均在 finally 中退出。
node probe.cjs
# 检查探针 JavaScript 语法。
node --check probe.cjs
```

依赖准备退出 0，149 个包，实际耗时 50.6 秒；探针最终运行退出 0，24 项断言通过；语法检查退出 0。网络速度提示和 Vue 2 的上游弃用提示不计为功能失败，也不据此取消历史兼容目标。

| 观察项 | Vue 2.6 | Vue 2.7 | Vue 3 |
| --- | --- | --- | --- |
| 初始样例语义错误数 | 0 | 0 | 0 |
| 模板字符串成员补全含 toUpperCase | 是 | 是 | 是 |
| Vue 导入定义定位到 model.ts | 是 | 是 | 是 |
| 符号重命名结果包含 Vue 与 TS 源文件 | 是 | 是 | 是 |
| TS 内存值改为 42 后，Vue 模板返回错误 2339 | 是 | 是 | 是 |
| TS 磁盘文本仍为原字符串值 | 是 | 是 | 是 |
| 对应独立服务真实退出 | 是 | 是 | 是 |

每项目的八项断言分别是初始有效、未保存类型诊断、模板补全、定义目标、重命名包含 Vue、重命名包含 TS、磁盘不变及进程退出。结果只证明这些样例和工具组合，不能推导任意 patch、所有写法或完整语言特性已支持。

## 已发现限制

1. 组件属性查询存在差异：当前样例中旧适配模块返回空属性列表，没有对数字传入字符串 title 属性报告错误 2322；Vue 3 样例返回 title 属性并报告该错误。原因尚未确定，不能认定是已定位的上游缺陷或本次适配正确性已达标。
2. Vue 3 声明式插槽样例返回 default。Vue 2 样例没有构造同等插槽声明，因此其空列表不能作为“不支持 Vue 2 插槽”的结论；还需补适用的 scoped-slot 样例。
3. 历史工具会在实验项目的 `node_modules/.vue-global-types` 中生成辅助声明。正式实现必须验证私有缓存或虚拟视图策略，同时保持模块解析正确，不能把这一默认副作用带入用户项目。
4. 未验证自动导入的完整附加编辑、跨文件实际应用/Undo、类组件/装饰器、JSX/TSX、样式、取消/超时、插件授权与租约或进程树联合撤销。直接进程退出检查不等于宿主已具备跨插件生命周期治理。
5. 原始实验只通过源代码导入验证相关 TS 类型；尚未建立正式 TS 项目契约、Vue 消费方、资源导出、关联文档流和宿主上下文绑定，不能宣称完整 P0 或 V01–V14 已完成。

## 本机证据

实验输入、锁文件、源码、tsserver 日志与[原始结果](../../../target/vue-typescript-p0/observations.json)位于忽略目录，不属于发行产物或 SDK。探针为可丢弃验证代码，不作为最终跨插件实现直接交付。

| 文件 | SHA256 |
| --- | --- |
| probe.cjs | 76D7F5B0583DDCB4F1BDB1A46190CFAE957500D293AB8C2A7EAAC20CC374E2F1 |
| pnpm-lock.yaml | DFBDC3F7691684097304327142F41C03FD25D153A42AF72CA462CCAF288A9675 |
| observations.json | 69E7FC22B8D9204464868484DA9643426DED465C21ED56A8857CAF3688E79DD1 |

本次还生成了本地工单拆分草案，未发布议题、提交、推送或关闭工单。未改 Rust 生产实现，未运行完整 Cargo 构建或 UI 验收；这些仍是正式实现阶段的交付门槛。
