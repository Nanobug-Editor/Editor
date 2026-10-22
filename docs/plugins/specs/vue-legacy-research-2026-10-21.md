# Vue 1.x 与 0.x 历史适配调研

日期：2026-10-21。状态：**调研与规划输入，未实施、未运行、未验收，不是支持声明。**

本页只整理官方归档、官方源码标签与工具文档。标签存在、模板曾可运行、构建器能处理 `.vue`，都不能证明 Nanobug 已具备该版本的完整语言能力。本轮未安装依赖、构建或运行历史 Vue，也未创建工单、修改父议题或提交推送。

现有[多版本规格](vue-multi-backend.md#1-已确认范围)仍是边界：一个 Vue 插件、多兼容后端；TypeScript 插件统一拥有项目上下文和类型分析；补全、诊断、悬浮、跳转、引用、重命名、格式化经适用场景验收后才正式支持。1.x／0.x 与 HTML/CDN 是后续覆盖目标。本页不改变已交付的精确版本矩阵，也不把历史写法转为现代 Vue 的合法写法。

## 1 可核对的精确标签样本

2026-10-21 对[官方 Vue Git 仓库](https://github.com/vuejs/vue)执行只读 `git ls-remote --tags`，当前可见标签快照见[版本清册](vue-legacy-version-inventory-2026-10-21.json)。清册中有 177 个无预发布标识的 SemVer 标签、114 个预发布标签；其中 0.x 有 56 个，1.0 有 29 个。这里的“稳定样本”仅指无预发布标识且 `package.json.version` 对应的标签，**不等于完整 npm 发行史、当前维护承诺或本产品支持**；未调查到的更早发行不能据此断言不存在。

下列八个样本的标签拼写、提交和官方 `package.json` 版本均已读回。带注释标签 `v1.0.28` 使用 peeled commit，其余使用远端直接报告的提交。

| 精确标签 | 提交 | package 版本的一手来源 |
| --- | --- | --- |
| `0.6.0` | `218557cdec830a629252f4a9e2643973dc1f1d2d` | [0.6.0/package.json](https://raw.githubusercontent.com/vuejs/vue/0.6.0/package.json) |
| `v0.7.6` | `bf5c14b9e69e1069caebabe03feef565da065fc7` | [v0.7.6/package.json](https://raw.githubusercontent.com/vuejs/vue/v0.7.6/package.json) |
| `v0.8.8` | `a701b9f402f33bc14f7847253ac919664f6b1da8` | [v0.8.8/package.json](https://raw.githubusercontent.com/vuejs/vue/v0.8.8/package.json) |
| `v0.9.3` | `e1aad8fc83cd923703dc2bfa335b32879aa7be44` | [v0.9.3/package.json](https://raw.githubusercontent.com/vuejs/vue/v0.9.3/package.json) |
| `v0.10.6` | `cf37f7efd6d63c0aa46b50c624816e645ddd7edd` | [v0.10.6/package.json](https://raw.githubusercontent.com/vuejs/vue/v0.10.6/package.json) |
| `0.11.10` | `d257c81a5889d45012f6df39873fba3f8697f0cc` | [0.11.10/package.json](https://raw.githubusercontent.com/vuejs/vue/0.11.10/package.json) |
| `0.12.16` | `d53c40ab698942c891441d16bce1374982842d60` | [0.12.16/package.json](https://raw.githubusercontent.com/vuejs/vue/0.12.16/package.json) |
| `v1.0.28` | `a8d6330d7e6b30c252aa753f99c7cb73bfc67a70` | [v1.0.28/package.json](https://raw.githubusercontent.com/vuejs/vue/v1.0.28/package.json) |

`0.6.0`、`0.11.10`、`0.12.16` 不带 `v`，不能机械拼接下载地址。上表是复现锚点，不以每条分支最后一个可见标签替代该分支全部 patch 的验收。

## 2 已确认的语法与语义边界

下表只陈述各归档明确展示的写法。0.12→1.0 的对照来自两代官方归档，**不是已找到一份穷尽所有变更的迁移清单**。归档通常描述分支末期能力；具体标签是否合法仍应回到对应源码和测试。

| 边界 | 官方事实 | 对适配的直接影响 |
| --- | --- | --- |
| 0.10 的旧指令与 partial | `v0.10.6` 源码注册 `component`、`repeat`、`model`、`with`、`partial`、`view`；partial 实现明确警告旧 `{{>yield}}` 已弃用，应改用 `<content>`。[指令注册](https://github.com/vuejs/vue/blob/cf37f7efd6d63c0aa46b50c624816e645ddd7edd/src/directives/index.js)、[partial 实现](https://github.com/vuejs/vue/blob/cf37f7efd6d63c0aa46b50c624816e645ddd7edd/src/directives/partial.js) | 0.10 不能由 0.12 的规则反推；需另查旧作用域与 partial 插入位置。本轮未穷尽 0.6–0.10 语法。 |
| 0.11→0.12 组件入口 | 0.11 入门使用 `<div v-component="my-component">`；0.12 入门使用注册后的 `<my-component>` 自定义元素。[0.11 入门](https://011.vuejs.org/guide/index.html)、[0.12 入门](https://012.vuejs.org/guide/) | 组件识别、名称补全、F12 与引用不能只搜索现代自定义标签。 |
| 0.12→1.x 指令与列表 | 0.12 有 `v-attr="width:w"`、`v-repeat="user : users"`；`user in users` 从 0.12.8 加入。1.x 使用 `v-bind:href`／`:href`、`v-on:click`／`@click` 与 `v-for`。[0.12 指令](https://012.vuejs.org/api/directives.html)、[1.x 语法](https://v1.vuejs.org/guide/syntax.html)、[1.x 入门](https://v1.vuejs.org/guide/) | 要解析旧参数、列表别名及其作用域；不能把旧指令报成拼写错误，或格式化为新语法。 |
| 0.12→1.x props | 0.12 动态 prop 使用 `msg="{{parentMsg}}"`，双向用 `{{@ parentMsg}}`，一次性用 `{{* parentMsg}}`。1.x 对应绑定使用 `:msg`、`:msg.sync`、`:msg.once`，默认仍是单向。[0.12 组件](https://012.vuejs.org/guide/components.html)、[1.x props](https://v1.vuejs.org/guide/components.html#Props) | 必须区分父表达式与子 prop，重命名需保留该版本的绑定方向。 |
| 内容分发与 partial | 0.12 有 `<content select="li">`、`<partial name="{{partialId}}">`；1.x 使用 `<slot>` 分发内容，仍有 `Vue.partial` 与 `<partial :name="partialId">`。[0.12 特殊元素](https://012.vuejs.org/api/elements.html)、[1.x API](https://v1.vuejs.org/api/#Special-Elements) | partial 是注册模板入口，fragment 是实例渲染形态，两者不能混为一种组件声明。动态名字不能静态猜成唯一目标。 |
| 1.x 插值和 filter | 1.x 允许属性 mustache、`{{{raw_html}}}`、`{{* msg}}`，filter 管道参数以空格分隔；自定义 filter 可定义 `read`／`write` 双向转换。[1.x 语法](https://v1.vuejs.org/guide/syntax.html)、[双向 filter](https://v1.vuejs.org/guide/custom-filter.html#Two-way-Filters) | 表达式解析不能把 filter 管道直接当普通 JS 位运算；写回目标、filter 名称与参数需分别建模。 |
| 1.x 表单绑定 | `v-model` 使用 `lazy`、`number`、`debounce="500"` 参数属性。[1.x 表单](https://v1.vuejs.org/guide/forms.html#Param-Attributes) | 不能统一套用现代 modifier 或删除这些属性；需覆盖读与写两种方向。 |
| 1.x fragment | 1.x 允许多顶层节点等 fragment instance；`$el` 此时是锚点，非 prop 属性及过渡等有特有限制。[1.x fragment](https://v1.vuejs.org/guide/components.html#Fragment-Instance) | 套用 Vue 2 单根诊断会错误拒绝合法的 1.x 模板；也不能照搬 Vue 3 fragment 行为。 |
| 同一分支内部变化 | 0.12.8 才增加列表的 `in` 写法；0.12.11 才加入资源名称的 camelCase／PascalCase 转换。[0.12 指令](https://012.vuejs.org/api/directives.html#v-repeat)、[0.12 资源命名](https://012.vuejs.org/guide/components.html#Asset-Naming-Convention) | 即使最终 patch 探针成功，也不能自动把其全部语法能力授予更早 patch。 |

### 1.x→2.0 的迁移边界

官方 [Migration from Vue 1.x](https://v2.vuejs.org/v2/guide/migration.html) 明确列出：fragment instance、partial、双向 filter、内建 filter、属性插值的移除；双向 props 和表单参数的变化；列表参数顺序与 `track-by` 的变化。它说明这是语义迁移边界，不只是把 Vue 版本号改成 1。

适配探针应至少把 1.x 的多根、partial、filter 写回、`.sync`／`.once` 和表单参数做成合法样例，再用相应 2.0 样例验证版本隔离。这里描述的是 **1.x→2.0 当时的迁移**，不由此推断后来所有 Vue 2 小版本都不再有同名语法，也不将迁移助手的改写建议作为旧项目必须升级的要求。

## 3 文件形态：SFC 与 HTML/CDN 都有历史依据

**SFC 不是 Vue 2／3 才出现的形态。** 0.11 与 0.12 的官方 Building Larger Apps 均有 Single File Components 专节，描述把模板、JavaScript、CSS 放入一个文件，经 Vueify（Browserify）或 vue-loader（Webpack）预处理打包。[0.11 文档](https://011.vuejs.org/guide/application.html#Single-File-Components)、[0.12 文档](https://012.vuejs.org/guide/application.html#Single-File-Components)

Vueify 官方当前说明明确区分：9.x 对应 Vue 2，8.x 对应 Vue 1。其 [8.x README](https://raw.githubusercontent.com/vuejs/vueify/8.x/README.md) 展示 `.vue` 的 `<template>`／`<script>`／`<style>`、预处理语言与 style `src`；这能支持历史 SFC 的调查方向，不能证明每个更早 Vue 标签都兼容 Vueify 8.x，也不能从该例推断任意外部块形式均已验证。[版本说明](https://raw.githubusercontent.com/vuejs/vueify/master/README.md)

**HTML 与无打包器入口也不能遗漏。** 0.11 官方安装页明确通过 `<script>` 暴露全局 `Vue` 并提供 CDN 路径；1.x 安装页同样支持独立脚本和 CDN，NPM／SFC 是另一条路径。[0.11 安装](https://011.vuejs.org/guide/installation.html)、[1.x 安装](https://v1.vuejs.org/guide/installation.html) 0.12 API 还明确根据 `el`／`$mount()` 遍历实际 DOM 编译。[0.12 初始化](https://012.vuejs.org/api/#Initialization)

由以上事实提出的规划建议，尚未实施：

- 从第一轮历史探针就收集两类输入：SFC，以及 HTML 中的 DOM 模板与关联脚本。版本证据应覆盖本地包与固定版本脚本，不以“缺 package.json”判定不是 Vue 项目。
- 若实施按 SFC 先行，可先明确标记“SFC 限定的精确版本支持”。在宣称覆盖该历史版本的 HTML/CDN 用法前，必须增加 HTML 文档归属、挂载作用域、脚本关联与对应七能力验收；不能把现代 SFC 结果外推过去。
- CDN 是历史使用形态，不代表编辑器分析时应执行远端代码。探针可固定 URL、内容摘要与离线副本；浮动地址或冲突证据应报告不确定，不静默选择最新 Vue。
- DOM 解析限制和预处理后的 SFC 模板需要分别验证；已有 HTML 提供者与 Vue 模板区域的协作应沿用公开机制，不新增宿主插件白名单。此边界继续遵守[平台规格](plugin-api-platform.md#15-与现有规范的关系)。

## 4 可复用工具的证据强度

| 工具／资料 | 本轮确认到什么 | 不能据此承诺什么 |
| --- | --- | --- |
| Vueify 8.x | 官方明确适配 Vue 1.x；是 Browserify 的 SFC transform，暴露编译 API。8.x 文档也列出编辑器语法高亮。[版本说明](https://raw.githubusercontent.com/vuejs/vueify/master/README.md)、[8.x README](https://raw.githubusercontent.com/vuejs/vueify/8.x/README.md) | 构建转换、高亮与模板语法校验不等于完整七能力，不保证适用于 0.x 或现代 Node。 |
| 当前 Vue language-tools | 官方 README 列出 language-server、language-service、TypeScript plugin、虚拟代码等模块。[官方 README](https://raw.githubusercontent.com/vuejs/language-tools/master/README.md) | 在本轮已读资料中未找到明确承诺 Vue 1.x／0.x 完整支持的依据；不能直接把现代模块作为已兼容后端。 |
| Vetur／VLS | 官方描述模板插值转换成虚拟 TypeScript，并提供映射后的诊断、补全、悬浮、定义、引用等能力。[插值服务文档](https://vuejs.github.io/vetur/guide/interpolation.html)、[官方仓库](https://raw.githubusercontent.com/vuejs/vetur/master/README.md) | 这些描述不证明旧 `v-repeat`、双向 filter、partial 与各 patch 语义受支持。本轮未取得 1.x／0.x 全七能力的明确支持承诺。 |
| Vue migration helper | 官方定位是 1.x→2.0 的扫描与迁移提示，且说明不能可靠捕获所有弃用写法。[官方 README](https://github.com/vuejs/vue-migration-helper#readme) | 不是旧版本语言服务，也不能用迁移成功替代保持旧语义的验收。 |

这里的“未找到”限定于本页列出的资料，并非证明不存在任何可复用实现。下一步应对候选后端做可失败的最小能力探针，再决定复用、保留历史分支或补充模板前端；不先承诺换一个版本参数即可完成。

历史包的开发依赖也不构成现代运行时保证：例如 [0.12.16 清单](https://raw.githubusercontent.com/vuejs/vue/0.12.16/package.json) 包含 Grunt 0.4 与 PhantomJS 1.9，[1.0.28 清单](https://raw.githubusercontent.com/vuejs/vue/v1.0.28/package.json) 使用当时的 Babel／Webpack 工具。**本轮没有证明这些工具能在 Node 24.19.0、TypeScript 5.9.3 或当前 Windows 直接构建运行，也没有证明必须使用某一历史 Node 版本。** 编辑器的受管分析工具与重建旧应用是两件事，需分别锁定兼容组合。

## 5 建议的最小探针与验收接缝

以下均为**未执行的建议**，供父计划取舍，不是新增工单或已满足的 AC。

1. **先证实 1.0.28，再进入 0.12.16／0.11.10。** 每个锚点至少包含一个标准 SFC 和一个 HTML 挂载例；先观察候选解析器是否保留上表合法语法，再测语义。0.10.6 单独验证旧指令、partial 与作用域，随后按 0.9／0.8／0.7／0.6 的官方源码差异逐段推进，不能从 0.12 覆盖结果一次扩大到“0.x”。这是一种降低未知量的顺序建议，不改变长期保留历史能力的要求。
2. **用少量有区分力的合法／非法对照。** 0.11 `v-component`、0.12 列表别名和双向 prop、1.x 多根／filter／partial／表单参数各保留一例；同一文件在不同精确版本上下文中应产生符合该版本的结果。另选 patch 边界两侧样本验证“版本证据真的影响规则”，如 0.12.7 与 0.12.8 的 `in` 写法。
3. **七能力落在真实源码。** 在声明的文件形态上逐项观察补全、诊断、悬浮、F12、引用、重命名、格式化。组件／prop／filter／partial 名称及列表局部变量要区分；未保存 JS 与模板同时改变后，旧结果必须拒绝。重构需保留既有跨文件预览、取消、应用与整组 Undo，不把虚拟文件名或生成代码编辑交给用户。
4. **保持已有项目与生命周期机制。** Vue 插件贡献历史模板语义及映射，复用 TS 提供者的统一上下文与受管分析进程；混合旧／新项目仍须隔离版本、共享同一份脏源。未知版本、工具缺失、拒绝准备、后端退出与切换不能借“历史模式”绕过权限、旧引用失效、有限恢复或已验收现代项目的健康性。[现行职责与接缝](vue-multi-backend.md#3-职责与模块接缝)
5. **锁定实际可交付输入。** 先记录 Vue、解析／映射模块、TS、Node、平台、SFC 构建器（如适用）、资源及许可的精确身份，再运行公开包与原生行为验收。历史预处理器的存在不自动扩大内置范围；Pug／Stylus 等仍按[现行生态边界](vue-multi-backend.md#1-已确认范围)由专门插件参与。

## 6 必须单列的未知项

| 未知／未验证 | 下一份有用证据 |
| --- | --- |
| 0.6–0.9 的完整模板、作用域、组件及表单边界；0.10 的完整语义 | 对精确标签逐段阅读 compiler／directive 源码和既有测试，提取最小合法／非法样例；本轮只核对了前四个版本的 package，不声称完成其语法调查。 |
| 最早 SFC 可用的精确版本与 loader／Vueify 配对 | 0.11／0.12 官方文档已证明当时存在该形态，但不证明引入的首个 patch；仍需工具标签、依赖范围及真实编译输入互证。 |
| 当前候选语言工具是否可保留旧语义并提供全七能力 | 固定版本后执行上节探针，记录每项实际结果；不能把本轮文档阅读报告为通过。 |
| 历史 JS／TS、装饰器、JSX 等写法与现代 TS 的交集 | 对真实声明组合单独调查；旧 Vue 有 Options 对象或某构建器有自定义 TS hook，都不等于当年的完整 TypeScript 类型工具已被支持。 |
| 旧构建依赖的当前可获取性、许可闭包、平台与运行时兼容性 | 只读清点候选工件与依赖后再授权准备，不全局安装，也不擅自运行项目配置／构建 hook。 |
| HTML 全局构造器、多个挂载根、动态模板或动态 partial 的静态解析上限 | 精确说明可追踪的声明形式与不能静态唯一解析的情况，验收错误提示与手动选择；不声称可静态推导任意运行时 JS。 |

官方归档是历史语义依据，当前工具 README 是调研当日的工具说明；后续实现应将真正采用的源码、工具版本与摘要另行固定。本页没有将任何历史版本提升为 supported，也没有给全部 semver、全部平台或未来发行作出兼容保证。
