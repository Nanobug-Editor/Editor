# 原生 UI 文档目录

本目录保存原生 UI 方案，以及 GPUI 控件、输入与布局相关调研。方案和调研按各自状态、日期与依赖版本理解，不代表相关功能已实现或验收。

- [项目首页、多窗口与 Windows 目录右键打开规格](specs/project-windows.md)：产品行为及应用级、Windows 系统集成测试边界已确认，已发布 [GitHub #102](https://github.com/T-miracle/Nanobug/issues/102)；首页与单项目闭环已实现，其余待实施。本次对话原型仅用于交互示意。
- [项目首页与多窗口实施工单](tickets/project-windows/README.md)：已发布 #103–#105，原生依赖为 103 → 104 → 105；01 已实现、验收、审查并推送，#103 已核对关闭，证据见[实施验收](verification/project-home.md)与[双轴审查](verification/project-home-review.md)。02、03 尚未实施，测试归属见工单索引。

- [宿主消息窗口规格](specs/host-messages.md)：[两张实施工单](tickets/host-messages/README.md)已实现、验收、审查并推送，GitHub #85、#86 已核对关闭；证据见 [实施验收](verification/host-messages.md)。
- [宿主消息来源盘点](verification/host-messages-sources.md)：已接入来源、插件归属及排除依据。
- [运行截图反馈修正](verification/host-messages-feedback.md)：日常消息范围、扫帚按钮、窗口工具组与底部文字移除的后续验收，不改写原始工单证据。

- [快捷键面板与用户绑定规格](specs/keyboard-shortcuts.md)：[三张实施工单 #81–#83](tickets/keyboard-shortcuts/README.md)已交付并核对关闭；实际证据与限制见[验收记录](verification/keyboard-shortcuts.md)。
- [资源管理器外部粘贴、拖放与文件操作撤销规格](specs/explorer-file-transfer.md)：[3 张工单 #78–#80](tickets/explorer-file-transfer/README.md)基础实现已合并；2026-10-12 Windows 复制/剪切、系统跨窗口多项拖入、联合操作、主题和退出清理均已复验，本次两项交互修复通过自动验证及审查。三票及父方案 #76 均已按 completed 关闭并读回核对，见[验收记录](verification/explorer-file-transfer.md)。

- [GPUI 鼠标滚轮惯性与顺滑滚动调研](research/GPUI滚轮惯性调研.md)：2026-09-23 的研究记录。

后续 UI 方案、工单与验收按需放入 `specs/`、`tickets/`、`verification/`；研究资料放入 `research/`，并更新本索引。

返回[文档总目录](../README.md)。
