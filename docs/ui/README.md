# 原生 UI 文档目录

本目录保存原生 UI 方案，以及 GPUI 控件、输入与布局相关调研。方案和调研按各自状态、日期与依赖版本理解，不代表相关功能已实现或验收。

- [本地比较迁移到 imara-diff](verification/imara-diff-2026-10-28.md)：统一差异标记和原生行对齐，解除旧矩阵限制；验证状态见记录。

- [GPUI Kit 依赖迁移与退出修复](verification/gpui-kit-upstream-2026-10-26.md)：PR #3691 后的接口适配、临时 Fork 验证，以及 #3697 合并后回到官方 main 的记录。

- [警告提示点与消息标题分隔线修正](verification/message-severity-appearance-2026-10-26.md)：依据实际截图纠正偏红的警告橙，以及停靠标题的额外顶部抗锯齿边缘；检查状态见记录。

- [主分支 Release 消息橙点实测](verification/message-severity-release-2026-10-25.md)：主分支工作区强制重编译、实际 Release 的孤立警告验证，以及新旧 EXE 的桌面对照。

- [宿主与插件共享 UI 分阶段实施方案](../plugins/specs/shared-ui-roadmap.md)、[S0／S1 契约](../plugins/specs/shared-ui-s0-s1.md)、[本地工单](../plugins/tickets/shared-ui-foundation/README.md)：规划／待实施；原生模块集中在 `editor-ui`，宿主直接调用、插件经声明式适配器复用，补齐目标／焦点、定向文本、剪贴板和拖放的状态、事件、授权及联合验收。

- [提交面板顶部高度分割条](verification/commit-panel-split-resize-2026-10-24.md)：隐藏槽位导致的可见邻居索引错误、即时尺寸更新与释放后保存。

- [Hover 内容层级与统一外观](verification/editor-hover-appearance-2026-10-24.md)：定义与诊断卡片的圆角、柔和外框、级别提示，以及正文／代码／来源的排版。

- [Hover 与边框拖动的交互边界](verification/editor-hover-gestures-2026-10-24.md)：分隔条拖动误弹、晚到请求取消、弹框手势保留，以及真实 Windows/D3D 验收。

- [消息按钮三色提醒](verification/message-severity-2026-10-24.md)：普通信息蓝点、警告橙点、危险／错误红点，保留最高未查看等级及溢出入口一致性；实际检查见记录。

- [内部布局横向拖动与实时换行测量](verification/internal-resize-performance-2026-10-23.md)：保留补丁撤回前的历史；[统一 Editor 后诊断](../plugins/verification/unified-editor-2026-10-25.md)确认瓶颈来自发布版 Base 全文换行。[2026-10-25 修复与验收](verification/internal-resize-performance-2026-10-25.md)按用户明确授权维护锁定 Base 0.8.0 本地补丁，真实字体及原生分隔线门禁通过，保留实时换行与慢帧边界说明。

- [分层菜单方向图标](verification/action-menu-icons-2026-10-23.md)：Git 顶部分支菜单等通用分层菜单复用目录树的方向图标，验证范围与结果见记录。

- [编辑器文件标签浮起留位拖动](verification/editor-tab-drag-2026-10-23.md)：浮层、等宽空槽、相邻标签让位、松手提交及取消；实际验证与限制见记录。

- [编辑器标签显示方式](verification/editor-tab-placement-2026-10-23.md)：编辑区四边与“已打开文件”停靠窗口、用户级设置、两轴拖动及文件页图标；实际验证见记录。

- [文件标签文字下缘裁切修复](verification/editor-tab-text-2026-10-24.md)：原生字体行高与标题裁切框不匹配造成的下划线缺失，以及 DirectWrite 回归检查。

- [标签字形与图标的视觉对齐](verification/editor-tab-optical-alignment-2026-10-24.md)：以原生字形像素重心校正标题基线，替代仅核对布局框居中的旧验收。

- [底栏窗口按钮顺序与长按拖动](verification/window-toolbar-order-2026-10-23.md)：默认前三窗口、插件安装顺序、按工作区保存的拖动排列及溢出菜单，实际检查状态见记录。

- [窗口拖动优化历程](verification/window-drag-optimization-summary-2026-10-23.md)：从最初约 60 次/秒到用户反馈明显更顺畅，逐步解释保存合并、图标规则、树行成本和发布锁竞争修复，以及每轮实测与剩余验收。

- [窗口与内部布局拖动刷新检查](verification/window-drag-refresh-2026-10-23.md)：上述优化的详细证据，包含控件夹具、运行产物核对、真实坐标／绘制采样、共享锁时序及普通 Release 复测边界。

- [通用平滑滚动](verification/smooth-scroll-2026-10-21.md)：统一接入编辑器、文件树、列表与标签；后续[惯性与原生高刷](verification/inertial-scroll-2026-10-21.md)复用上游物理求解器，跟随显示器刷新节奏，记录真实帧率与性能边界。

- [比较视图的有界原生布局](verification/comparison-viewport-scroll-2026-10-21.md)：进一步限制可见副本与原始输入层的源布局，保留原生编辑、中文 IME 和屏幕外定位；原生测量、回归及剩余完整布局边界见记录。

- [校验慢帧、折叠来源范围与插件平滑滚动](verification/scroll-validation-folds-2026-10-22.md)：合并原生校验与字形准备，限制折叠区的输入包络，插件容器、标签和画布复用通用滚动组件；实际验收状态见记录。

- [比较视图原生换行映射与滚动峰值](verification/comparison-native-map-2026-10-22.md)：复用 Base 0.8 的只读显示行映射；两轮两栏／三栏／收起区域矩阵在本机接近 144 Hz，输入回归通过，首次准备仍有开销。

- [启动恢复上次项目](verification/startup-project-restore-2026-10-21.md)：默认开启的用户级开关、成功退出记录及显式启动目标优先级；验证结果与限制见记录。

- [editor-ui 统一入口与锁文件清理](verification/editor-ui-extraction-2026-10-21.md)：保留 Kit 正式入口，将本地控件集中到独立 crate；测试功能直接接入 Base/GPUI，并从锁文件移除组件运行时。实际验证状态见记录。

- [剩余 14 项宿主测试排查](verification/remaining-tests-2026-10-21.md)：区分过时行为断言、按需激活夹具与 Windows 会话路径问题，记录逐项修复及全量验证。

- [统一滚动条宽度动画](verification/scrollbar-width-2026-10-21.md)：Git 提交图视口轨道、宿主与插件共用的两秒保留／宽度收缩，以及原生输入、拖动和主题验证。

- [长代码差异滚动性能](verification/diff-scroll-performance-2026-10-21.md)：缓存验证后的原生行几何、保留对齐布局、让整行底色复用只读元数据；复现、实际计时、验证和候选状态见记录。

- [比较区单滚动条、等宽换行与顶部背景](verification/comparison-chrome-2026-10-20.md)：最右侧共享垂直轨道、平衡原生行号槽占宽、工具栏与来源标题统一背景，以及移除工具栏关闭按钮；当前验证与候选状态见记录。

- [比较工具栏四个箭头](../plugins/verification/git-comparison-arrows-2026-10-20.md)：左右文件导航、上下差异导航及移除统一／同步开关，实际状态见记录。

- [比较视图整行底色与默认同步](verification/diff-row-backgrounds-2026-10-20.md)：左侧外边距、完整改动行及内侧行号槽底色，移除同步开关；验证和候选交付状态见记录。

- [项目选择后的窗口居中](verification/project-window-centering-2026-10-19.md)：恢复项目尺寸时在当前显示器可用区域居中整个窗口；包含原生边框、缩放换算、初始化后内容区同步及真实 Windows 回归记录。

- [Base 控件边界与本地 UI 迁移](verification/base-only-components-2026-10-19.md)：`gpui-component` 仅作实现参考；依赖边界、控件迁移与本轮验证结果见记录。

- [工具窗口显隐统一布局](verification/dock-reveal-2026-10-23.md)：终端、大纲及其他窗口共用实际区域显隐，保留普通组合和独占切换的布局；本轮验证状态见记录。
- [四侧组合拖放与单侧唯一占用](verification/dock-group-fallback-2026-10-19.md)：一级成员拖回本侧时按完整组合检查占用并整组移动；空侧和具体窗口保留单独拖动，嵌套子窗口不提升。
- [单侧唯一组合与子窗口拖放（前一阶段）](verification/dock-single-side-2026-10-19.md)：保留四侧占用、局部分栏和实际验证记录；整组触发规则按后续修正执行。
- [停靠窗口嵌套布局与拖动范围（前一阶段）](verification/dock-nested-layout-2026-10-19.md)：一级方向、内部嵌套与 `>=` 尺寸边界；保留当时的隐式组合拖动及实际验证记录。
- [停靠区域方向限制（前一阶段）](verification/dock-regional-axis-2026-10-19.md)：保留当时的区域方向实现、组合换向与验证；层级边界按后续嵌套规则执行。
- [标记预览式窗口拖放](verification/marked-dock-placement-2026-10-19.md)：原布局保持不动，黄色来源、蓝色目标、标题小块与松手提交；采用拖动前实际尺寸，宿主和插件复用同一机制。
- [窗口实时拖放与通用停靠模块（历史）](verification/live-dock-placement-2026-10-19.md)：保留上一版实时让位的验证记录，交互已由标记预览替代；`editor-ui` 迁移现状见[统一入口与锁文件清理](verification/editor-ui-extraction-2026-10-21.md)。

- [差异比较内侧行号槽](verification/diff-inner-gutters-2026-10-19.md)：左右来源的行号槽集中到编辑器之间；本地原生像素组合、输入投影与验证范围见记录。

- [编辑器局部列与底部跨度](verification/editor-row-drop-2026-10-19.md)：资源管理器拖到编辑区左侧后仅占上方行，底部 Git 横跨两者；保留工具列宽度与保存恢复。

- [编辑区左边缘拖放修正（首轮记录）](verification/editor-edge-drop-2026-10-19.md)：保留首轮宽度验证；其全高左栏策略已由局部列修正替代。

- [左右侧栏独立宽度](verification/independent-sidebar-width-2026-10-19.md)：右侧窗口打开时保持左侧宽度，仅缩小中间编辑区；包含历史会话转换、用户调整与单独占据窗口的回归记录。

- [资源管理器预览标签](verification/explorer-preview-tabs.md)：单击共用预览，双击或实际编辑后固定；包含焦点、会话恢复、PNG、IME、无变化输入、双轴审查及原生验证记录。

- [资源管理器标题按钮的焦点淡入淡出](verification/explorer-title-actions-fade-2026-10-24.md)：四个按钮随面板焦点以约 140 ms 淡入淡出，固定标题槽位，并核对减少动画与现有按钮交互。

- [项目下拉与空启动](specs/project-menu-startup.md)：当前方案，移除独立启动页，将项目管理合并至编辑器左上角下拉；允许空启动，保留全局设置与插件管理，关闭项目关闭窗口，再次启动激活最后使用的窗口；[实施验收](verification/project-menu-startup-2026-10-14.md)记录 40 项项目交互测试、真实包验证、双轴增量审查和 Windows 原生结果，以及完整 UI 的既有失败。
- [独立欢迎窗口重新设计（历史）](verification/welcome-window-redesign-2026-10-14.md)：保留当时原生截图和验收证据；产品行为已由项目下拉与空启动方案替代。

- [问题面板默认右侧与首次布局](verification/problems-right-default-2026-10-24.md)：无保存布局时右侧宽度约 360px，左侧全高，底部横跨其余区域；[底部默认方案的历史验收](verification/problems-dock-panel.md)记录文件分组、定位、拖动、布局恢复及快速修复。

- [标题栏应用菜单调整](verification/application-menu.md)：左上角菜单入口及项目操作、快捷键、退出三组菜单的实现与验证记录。
- [项目标题与项目切换菜单](verification/project-switcher.md)：项目名下拉、独立窗口切换、最近项目及菜单入口更名；相关测试、双轴增量审查与 Windows 原生验收通过，完整 UI 保留 7 个既有基线失败。
- [Windows 完整路径显示修正](verification/windows-path-display.md)：统一隐藏磁盘路径扩展前缀，正确显示 UNC；覆盖项目、文件提示和宿主诊断，保留内部路径权威及实际原生验收记录。

- [项目首页、多窗口与 Windows 目录右键打开规格](specs/project-windows.md)：三单已实现、验收、审查并推送，P01–P24 已核对；#103–#105 及总 [#102](https://github.com/T-miracle/Nanobug/issues/102) 全部读回 closed/completed。本次对话原型仅用于交互示意。
- [项目窗口方案主分支集成验证](verification/project-windows-main-integration.md)：已合并并推送 main，SHA 已读回核对；记录快进、原有行尾差异、并发工作保留、编译缓存实际诊断，以及 7 项既有 UI 基线失败。
- [项目首页与多窗口实施工单](tickets/project-windows/README.md)：原生依赖为 103 → 104 → 105，三单均已核对关闭；见[首页验收](verification/project-home.md)、[并行窗口验收](verification/concurrent-project-windows.md)、[Windows 外部打开验收](verification/windows-project-entry.md)及各自双轴审查。最后一单完成真实安装器、资源管理器和原生双窗口联合验收；测试质量、去重和覆盖归属见工单索引。

- [宿主消息窗口规格](specs/host-messages.md)：[两张实施工单](tickets/host-messages/README.md)已实现、验收、审查并推送，GitHub #85、#86 已核对关闭；证据见 [实施验收](verification/host-messages.md)。
- [宿主消息来源盘点](verification/host-messages-sources.md)：已接入来源、插件归属及排除依据。
- [消息按钮选中状态](verification/messages-button-selection-2026-10-19.md)：选中外观跟随面板实际显隐，记录主题、缩放回归及修复前后共有的四项消息测试失败。
- [消息顶部隐藏停靠项分隔线修复](verification/message-header-hidden-split-2026-10-27.md)：修正隐藏前驱停靠项给首个可见消息面板叠加的顶线，保留正常分隔与缩放。
- [运行截图反馈修正](verification/host-messages-feedback.md)：日常消息范围、扫帚按钮、窗口工具组与底部文字移除的后续验收，不改写原始工单证据。

- [快捷键面板与用户绑定规格](specs/keyboard-shortcuts.md)：[三张实施工单 #81–#83](tickets/keyboard-shortcuts/README.md)已交付并核对关闭；实际证据与限制见[验收记录](verification/keyboard-shortcuts.md)。
- [资源管理器外部粘贴、拖放与文件操作撤销规格](specs/explorer-file-transfer.md)：[3 张工单 #78–#80](tickets/explorer-file-transfer/README.md)基础实现已合并；2026-10-12 Windows 复制/剪切、系统跨窗口多项拖入、联合操作、主题和退出清理均已复验，本次两项交互修复通过自动验证及审查。三票及父方案 #76 均已按 completed 关闭并读回核对，见[验收记录](verification/explorer-file-transfer.md)。

- [GPUI 鼠标滚轮惯性与顺滑滚动调研](research/GPUI滚轮惯性调研.md)：2026-09-23 的研究记录。

后续 UI 方案、工单与验收按需放入 `specs/`、`tickets/`、`verification/`；研究资料放入 `research/`，并更新本索引。

返回[文档总目录](../README.md)。
