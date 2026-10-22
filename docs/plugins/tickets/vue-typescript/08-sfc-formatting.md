# 08 SFC 高亮与块格式化

状态：已发布 [#115](https://github.com/Nanobug-Editor/Editor/issues/115)，待依赖验收。用户已授权自主安排顺序、并行实施与子代理；验收完成后才提交完成状态。

## What to build

Vue SFC 使用插件提供的 Tree-sitter WASM grammar 和查询，组合所需块语言能力；标准模板及 CSS、SCSS/Sass、Less 可以按已选公开格式化提供者工作。

## Acceptance criteria

- [ ] Vue 版本差异、嵌入 JS/TS 与首批样式高亮可观察，停用后正确撤销，不恢复宿主内建 grammar。
- [ ] 整文件及适用块范围格式化保留块结构、属性和外部 src，不损坏其他块。
- [ ] 独立格式化选择与用户配置有效，不因 Vue 安装覆盖其他提供者。
- [ ] 缺失块语言或专门插件时准确提示，不宣称相应生态已正式支持。
- [ ] 深浅主题、中文 IME、滚动、缩放及版本变化后的旧结果拒绝通过原生验收。

## Blocked by

04 Vue 多后端首次宿主分析。
