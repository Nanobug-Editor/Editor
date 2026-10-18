# 原生 UI 文档目录

本目录保存原生 UI 方案，以及 GPUI 控件、输入与布局相关调研。方案和调研按各自状态、日期与依赖版本理解，不代表相关功能已实现或验收。

- [资源管理器预览标签](verification/explorer-preview-tabs.md)：单击共用预览，双击或实际编辑后固定；包含焦点、会话恢复、PNG、IME、无变化输入、双轴审查及原生验证记录。

- [项目下拉与空启动](specs/project-menu-startup.md)：当前方案，移除独立启动页，将项目管理合并至编辑器左上角下拉；允许空启动，保留全局设置与插件管理，关闭项目关闭窗口，再次启动激活最后使用的窗口；[实施验收](verification/project-menu-startup-2026-10-14.md)记录 40 项项目交互测试、真实包验证、双轴增量审查和 Windows 原生结果，以及完整 UI 的既有失败。
- [独立欢迎窗口重新设计（历史）](verification/welcome-window-redesign-2026-10-14.md)：保留当时原生截图和验收证据；产品行为已由项目下拉与空启动方案替代。

- [底部问题面板](verification/problems-dock-panel.md)：原生停靠面板按文件分组，支持折叠及文件类型图标，状态栏隐藏零计数；包含定位、滚动、拖动、布局恢复及快速修复的验证记录。

- [标题栏应用菜单调整](verification/application-menu.md)：左上角菜单入口及项目操作、快捷键、退出三组菜单的实现与验证记录。
- [项目标题与项目切换菜单](verification/project-switcher.md)：项目名下拉、独立窗口切换、最近项目及菜单入口更名；相关测试、双轴增量审查与 Windows 原生验收通过，完整 UI 保留 7 个既有基线失败。
- [Windows 完整路径显示修正](verification/windows-path-display.md)：统一隐藏磁盘路径扩展前缀，正确显示 UNC；覆盖项目、文件提示和宿主诊断，保留内部路径权威及实际原生验收记录。

- [项目首页、多窗口与 Windows 目录右键打开规格](specs/project-windows.md)：三单已实现、验收、审查并推送，P01–P24 已核对；#103–#105 及总 [#102](https://github.com/T-miracle/Nanobug/issues/102) 全部读回 closed/completed。本次对话原型仅用于交互示意。
- [项目窗口方案主分支集成验证](verification/project-windows-main-integration.md)：已合并并推送 main，SHA 已读回核对；记录快进、原有行尾差异、并发工作保留、编译缓存实际诊断，以及 7 项既有 UI 基线失败。
- [项目首页与多窗口实施工单](tickets/project-windows/README.md)：原生依赖为 103 → 104 → 105，三单均已核对关闭；见[首页验收](verification/project-home.md)、[并行窗口验收](verification/concurrent-project-windows.md)、[Windows 外部打开验收](verification/windows-project-entry.md)及各自双轴审查。最后一单完成真实安装器、资源管理器和原生双窗口联合验收；测试质量、去重和覆盖归属见工单索引。

- [宿主消息窗口规格](specs/host-messages.md)：[两张实施工单](tickets/host-messages/README.md)已实现、验收、审查并推送，GitHub #85、#86 已核对关闭；证据见 [实施验收](verification/host-messages.md)。
- [宿主消息来源盘点](verification/host-messages-sources.md)：已接入来源、插件归属及排除依据。
- [运行截图反馈修正](verification/host-messages-feedback.md)：日常消息范围、扫帚按钮、窗口工具组与底部文字移除的后续验收，不改写原始工单证据。

- [快捷键面板与用户绑定规格](specs/keyboard-shortcuts.md)：[三张实施工单 #81–#83](tickets/keyboard-shortcuts/README.md)已交付并核对关闭；实际证据与限制见[验收记录](verification/keyboard-shortcuts.md)。
- [资源管理器外部粘贴、拖放与文件操作撤销规格](specs/explorer-file-transfer.md)：[3 张工单 #78–#80](tickets/explorer-file-transfer/README.md)基础实现已合并；2026-10-12 Windows 复制/剪切、系统跨窗口多项拖入、联合操作、主题和退出清理均已复验，本次两项交互修复通过自动验证及审查。三票及父方案 #76 均已按 completed 关闭并读回核对，见[验收记录](verification/explorer-file-transfer.md)。

- [GPUI 鼠标滚轮惯性与顺滑滚动调研](research/GPUI滚轮惯性调研.md)：2026-09-23 的研究记录。

后续 UI 方案、工单与验收按需放入 `specs/`、`tickets/`、`verification/`；研究资料放入 `research/`，并更新本索引。

返回[文档总目录](../README.md)。
