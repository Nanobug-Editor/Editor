# 插件系统文档目录

- [Canvas 协议扩展](specs/canvas-protocol-extension.md)：四个顺序实施的垂直切片；[工单与依赖](tickets/canvas-protocol-extension/README.md)、[实施验证](verification/canvas-protocol-extension-2026-10-24.md)。1.4–1.6 前置已交付，01 已验收，02–04 待实施；全部完成后统一合并独立分支至 main。

- [Canvas 1.4–1.6 前置基线](verification/canvas-baseline-2026-10-25.md)：TextBox、原生文字测量与列宽拖动的独立整理及验证记录。

- [父 #107 剩余范围与 Vue 历史版本适配计划](specs/vue-legacy-adaptation-plan-2026-10-21.md)：2026-10-21 核对已知 17 张实施工单均 completed；整理 2.5→2.0→1.x→0.x、HTML/CDN 与持续适配的分批计划。附[历史一手资料调研](specs/vue-legacy-research-2026-10-21.md)和[官方标签快照](specs/vue-legacy-version-inventory-2026-10-21.json)，不代表新增版本已实现或验收。

- [Vue/TypeScript 旧工单功能核对与关闭](verification/vue-typescript-legacy-closure-2026-10-20.md)：用户补充授权后，64 项旧要求逐项核对完成；#108–#118 已读回 completed，原正文和失败历史保留，父 #107 继续开放。

- [Vue/TypeScript 剩余场景交付](verification/vue-typescript-remaining-delivery-2026-10-20.md)：新 #122–#126 已验收、三仓推送并读回 completed，连同 #121 六场景 6/6 完成；最终 .15/.12 双包、普通桌面和门禁有据。旧工单与父议题不变，固定支持范围和 AccessKit 退出限制保留，未创建 Release。

- [Vue/TypeScript 15:00 恢复记录](verification/vue-typescript-resumed-2026-10-20.md)、[05:08 暂停检查点](verification/vue-typescript-paused-2026-10-20-0508.md)、[剩余场景双轴审查](verification/vue-typescript-remaining-review-2026-10-20.md)：保留恢复、失败检查点和后续修复过程；最终状态以上述交付记录为准。

- [取消后的独立委托请求](verification/vue-typescript-deferred-control-cancel-2026-10-20.md)、[S3 历史事务与 S4 生态宿主验收](verification/vue-typescript-s3-s4-native-acceptance-2026-10-20.md)、[剩余场景实施](verification/vue-typescript-remaining-2026-10-19.md)、[TypeScript 共享上下文选择](verification/vue-typescript-context-selection-2026-10-20.md)：保存固定包、原生交互、事务和请求生命周期的实际验证边界。

- [Git 面板隔离与原生帧滚动](verification/git-panel-isolation-2026-10-22.md)：Git 0.1.29 隔离普通交互、索引刷新和诊断；通用 `ui.canvas 1.3` 保留绘图缓冲，避免每帧等待 WASM 回包。[公开能力设计](specs/canvas-retained-scroll.md)。

- [Git 提交目录勾选与索引锁反馈](verification/git-commit-checkbox-lock-2026-10-22.md)：0.1.28 保留整目录批量暂存语义，限制提交区错误显示高度，并关闭后台查询的可选索引写入；回归与环境恢复证据见记录。

- [Git 提交窗口改造](verification/git-commit-panel-2026-10-22.md)：IDEA 式勾选文件树与固定提交区；Git 0.1.27 通过通用 ui.tree 1.7 接入立即暂存／取消暂存，实际包、检查结果与原生验收状态见记录。

- [Git 大提交列表滚动与加载开销](verification/git-history-performance-2026-10-22.md)：0.1.26 增量维护分块分支线和 OID 索引，滚动不再遍历全部提交，未变化的定时观察只检查一页；复现数据、实际包和原生验证见记录。

- [工作区文件着色与顶部插件入口](specs/workspace-presentation.md)：提供者选择、可调整颜色、多个入口排序及 IDEA 式分支菜单统一使用公开能力；Git 0.1.25 与非 Git 消费者、两组 Windows 原生行为已验收，未发布。全仓检查的并行改动限制及候选身份见[验收记录](verification/workspace-presentation-2026-10-22.md)。

- [Java 插件首版方案与后续插件计划](specs/java-plugin.md)：2026-10-22 已确认并开始顺序实施；本轮仅 Java 插件及必要通用宿主能力，Lombok、Maven、Gradle、JUnit 为后续独立插件。[实施记录](verification/java-plugin-2026-10-22.md)区分实际进展、验收及限制。

- [官方插件兼容核验与 SDK 助手接入方案](specs/official-plugin-sdk-adoption-2026-10-21.md)：已获用户授权，由三个独立会话并行实施 TypeScript 兼容修复、Git ResourceGroup、Markdown EditorIntent；进度见[实施调度记录](verification/official-plugin-sdk-adoption-execution-2026-10-21.md)。此前市场七项／八个发布包静态准入通过，固定证据见[兼容核验](verification/official-plugin-compatibility-2026-10-21.md)及[源码调研](specs/official-plugin-sdk-adoption-sources-2026-10-21.md)；当前未宣称改造已验收完成。

- [分支树双击选择与菜单悬停](verification/git-branch-gestures-2026-10-21.md)：公开 ui.tree 1.5 选择手势、Git 0.1.23 接入，以及原生灰色悬停和菜单期间保留高亮；实际验证状态见记录。

- [比较首次定位、居中导航与双折叠按钮](verification/comparison-navigation-2026-10-21.md)：从改动文件打开首处差异，前后差异按真实视口居中，折叠／展开独立置灰；实施与验证状态见记录。

- [SDK 生命周期助手、运行诊断与兼容验证工具](specs/plugin-developer-tooling.md)：三项已实施并完成本批功能验收，兼容插件热安装保留进程、未保存内容及撤销/重做；实际检查及共享工作区格式限制见[实施验证记录](verification/plugin-developer-tooling.md)。

- [安装确认框全局启用选项](verification/install-global-option-2026-10-21.md)：首次安装时在左下角提供默认未勾选的全局选项；更新保留原启用范围，实际验证状态见记录。

- [多种插件系统对照与改进建议](specs/plugin-system-improvements-research-2026-10-21.md)：基于当前公开接口与外部一手资料的调研建议；区分已有基础、SDK 易用性、持续兼容验证和运行诊断，不代表已批准实施或完成验收。

- [宿主与插件统一滚动条](../ui/verification/scrollbar-width-2026-10-21.md)：修正通用画布轨道定位，统一编辑器、树、列表、终端及插件界面的宽度隐藏动画；公开协议和独立 Git 包不变。

- [插件与宿主共用平滑滚动](../ui/verification/scroll-validation-folds-2026-10-22.md)：普通滚动容器、横向标签与带范围的画布共用 UI 求解器；内容 epoch、精确滚轮、确认及比较区折叠输入的验证见记录。

- [Git 分支缓存与快速筛选](verification/git-branch-fast-switch-2026-10-21.md)：独立 0.1.22 的有界首屏／预览缓存、已知祖先投影、只读查询抢占与历史面板局部发布；实际原生帧检查和候选身份见记录。

- [通用画布连续滚动与绘制复用](verification/canvas-scroll-2026-10-21.md)：公开 `ui.canvas 1.2` 内容 epoch、原生连续滚轮、16ms 同步与矢量平移复用；独立 Git 0.1.21 的实际验证及候选状态见记录。

- [Git 分支查询白屏与增量刷新](verification/git-incremental-refresh-2026-10-20.md)：保留完整日志／预览与缓存目录，公开 `ui.tree 1.4` 结构增量及比较单按钮切换；0.1.21 冻结候选与最终验证状态见记录。

- [比较区单滚动条、等宽换行与顶部背景](../ui/verification/comparison-chrome-2026-10-20.md)：通用原生比较控件的轨道、占宽与顶部呈现修正；独立 Git 0.1.19 和公开契约不变，实际状态见记录。

- [Git 比较无差异区段折叠／展开](verification/git-unchanged-folding-2026-10-20.md)：复制到独立插件的资源管理器图标、公开 `editor.diff 1.4` 与双／三栏同步折叠；当前候选及实际验收状态见记录。

- [比较工具栏箭头与原提交文件导航](verification/git-comparison-arrows-2026-10-20.md)：四个原生箭头、固定于原提交的文件序列及公开 `editor.diff 1.3`；当前实施、实际验收与配套 0.1.18 包状态见记录。

- [比较视图整行底色与默认同步](../ui/verification/diff-row-backgrounds-2026-10-20.md)：通用宿主控件保留左侧阅读边距、差异行尾和行号槽底色，默认同步并移除开关；独立 Git 0.1.17 包不变，当前验证及交付状态见记录。

- [Git 首次选择与文件状态颜色](verification/git-default-selection-2026-10-19.md)：独立 0.1.17 默认定位当前分支、预览第一条提交并按增删改着色；公开 `ui.tree 1.3`、字节分页修正、最终包原生回归及规定门禁通过，产物身份见记录。

- [Vue/TypeScript S1 正式基线复验](verification/vue-typescript-s1-delivery-2026-10-19.md)：2026-10-19 新工单 #121 已验收、三仓推送并读回关闭；正式基线门禁、真实插件与相关原生交互通过。此前过程见[恢复记录](verification/vue-typescript-s1-resume-2026-10-19.md)；当次范围止于 S1，后续进度见剩余场景记录，旧 #108–#118 只作历史，未发布版本。

- [Git 面板间距与文件图标](verification/git-panel-polish-2026-10-19.md)：独立 0.1.16 调整外层留白及预览内边距，配套宿主复用资源管理器文件图标并隐藏空菜单按钮；实际包原生回归、产物身份及既有格式检查例外见记录。

- [Git 分支树保留与菜单状态更新](verification/git-branch-retention-2026-10-19.md)：独立 0.1.15 使用公开 `ui.tree 1.2` 缓存更新；分支筛选和面板显隐保留已加载页、选择及滚动，真实 ref 变化继续刷新。配套宿主、实际包／原生回归及规定检查通过，产物身份见记录。

- [比较窗口内侧行号槽](../ui/verification/diff-inner-gutters-2026-10-19.md)：通用宿主比较标签及原生插件代码组共用呈现，独立 Git 仍使用 0.1.14 包；实际原生验证范围见记录。

- [Git 内嵌提交预览与历史比较标签](verification/git-inline-history-preview-2026-10-19.md)：独立 0.1.14 候选使用右侧文件树／提交信息预览，文件在主编辑区打开双／三栏只读标签；通用 `editor.diff 1.2`、原生验证及最终身份见记录。

- [Git 分支树图标](verification/git-tree-icons-2026-10-19.md)：独立 0.1.13 为目录和分支发布文件夹／分支图标；通用树行保留文件夹原始配色。实际资源、颜色、真实包及原生回归见记录。

- [Git 列表布局精简](verification/git-history-layout-2026-10-19.md)：独立 0.1.12 移除搜索行、表头及底部信息栏，拓扑增加左侧留白，底部功能迁入分支右键菜单；配套宿主修正通用后台资源清理权限。原生布局、真实定时器及规定检查通过，交付身份与范围见记录。

- [Git 分支树右键操作](verification/git-branch-context-menu-2026-10-18.md)：独立 0.1.11 撤下顶部工具栏，采用分支原生右键菜单及通用 ui.tree 1.1 禁用规则；真实 Git、原生菜单及全工作区检查通过。配套宿主、包身份与验收范围见记录，未自动安装或发布。

本目录索引实际维护的插件资料。跨插件平台方案放在本目录的 specs、tickets 与 verification；插件自身的历史专题资料保留在对应包内，不复制正文或继承其他任务的完成状态。

- [Git 面板图标与底栏入口](verification/git-panel-icons-2026-10-18.md)：独立 0.1.10 使用用户指定 SVG，底栏只保留暂存提交与 Git 两个入口。通用入口开关、实际图标绘制、101 项访客测试及当时规定检查有据；长时原生闲置的前台焦点限制、后续并行改动和已保存的宿主／包身份见记录。

- [内部窗口单独占据](specs/exclusive-dock-panels.md)：公开 `exclusive` 面板属性及区域切换语义；[实现与验证](verification/exclusive-dock-panels-2026-10-18.md)。

- [四侧组合拖放与单侧唯一占用](../ui/verification/dock-group-fallback-2026-10-19.md)：宿主与插件共用本侧整组联动、空侧单窗口、四侧占用及局部分栏；[前一阶段验证](../ui/verification/dock-single-side-2026-10-19.md)保留历史结果。
- [通用窗口标记预览拖放](../ui/verification/marked-dock-placement-2026-10-19.md)：保留原布局并以黄色来源和蓝色目标预览，松手后提交；`editor-ui` 提取在拖放完成后进行。
- [停靠窗口嵌套布局与拖动范围](../ui/verification/dock-nested-layout-2026-10-19.md)：宿主和插件共用一级区域方向限制、局部四方向嵌套、子窗口独立拖动与 `>=` 尺寸边界；[前一阶段方向验证](../ui/verification/dock-regional-axis-2026-10-19.md)保留历史证据。

- [Git 低频静默刷新回归](verification/git-silent-refresh-2026-10-18.md)：独立 0.1.9 将后台检查改为至少间隔 30 秒，真实变更也不整体禁用控件；101 项访客测试、3 项实际包回归、跨实际定时器的原生连续审查及规定工作区检查通过。新 ZIP 与 53 文件源码快照已生成，未替用户安装或发布。

- [Git 暂存提交面板归属调整](verification/git-sidebar-panel-2026-10-18.md)：已改为资源管理器、大纲同类的共享侧栏；17 项回归、旧布局一次迁移／用户位置恢复、深中100%／浅英150%原生拖动与隐藏重开通过，宿主便携包已生成。Git 仍为独立0.1.8；根工作区已有检查问题与实际范围见记录。

- [Git 工作窗口标题栏点击回归](verification/git-window-caption-2026-10-18.md)：已修复通用标题栏 Drag 覆盖原生按钮；真实拖动与三按钮、已加载提交详情的深色 100%／浅色 150% 两组合、关闭后单次主窗刷新及窗口退役通过。新宿主便携包已生成，Git 仍为独立 0.1.8，规定检查、旧格式偏差及验收范围见记录。

- [Git 周期检查打断审查回归](verification/git-auto-refresh-2026-10-18.md)：无变化检查清空树页和禁用控件已分别复现；独立 0.1.8 本地包完成真实包、两段原生闲置与错误恢复回归，规定工作区检查及 53 文件账本有据。现有根格式偏差、未重跑的矩阵及既存原生请求超时清理限制见记录。

- [Git 加载与工作窗口超时回归](verification/git-loading-window-timeout-2026-10-18.md)：2026-10-18 新反馈独立记录；已修复过期树分页残留 loading，Package/Manager、两种无插桩原生恢复、正常仓库完整流程及工作区检查通过，本地修复版已生成。容器目录与实际仓库入口、原工作区两处格式偏差及尚未独立复现的整窗卡死边界见记录。

- [Vue 多版本插件与 TypeScript 协作方案](specs/vue-multi-backend.md)、[已批准的六场景与当前执行范围](tickets/vue-typescript/README.md)、[历史工具层探针](verification/vue-typescript-p0-2026-10-15.md)：正式新工单 #121–#126 已交付并读回 completed；当前版本、范围和限制以剩余场景交付记录为准。原 11 张工单和 2026-10-15 暂停、测试记录保留追溯，不继承为当前源码或候选包的通过证据。
- [Vue/TypeScript 六场景历史检查点](verification/vue-typescript-scenes-2026-10-15.md)、[S4 生态组合历史证据](verification/vue-typescript-s4-2026-10-15.md)：从旧工作区保留的 2026-10-15 过程记录，包含当时的失败与未验收项；后续状态见[2026-10-20 联合交付](verification/vue-typescript-remaining-delivery-2026-10-20.md)，恢复原文不代表重新验收。
- [Git 插件范围与原型方案](specs/git-plugin.md)、[实施验证记录](verification/git-plugin-2026-10-15.md)、[历史暂停现场](verification/git-plugin-paused-2026-10-16.md)、[场景工单与最终进度](tickets/git-plugin-remaining/README.md)：#119、#120 均已验收、普通推送并读回 closed／completed，100%。独立 0.1.7／52 项源码，访客 101/101、实际业务 43＋修正夹具 1、五窗口 5/5、正式原生完整流程／八组合、54 图核验及规定宿主检查有据，最终双轴无未解决项。原全量旧夹具失败按真实身份保留。Git 由宿主外独立项目和显式 ZIP 交付，不随宿主内置或自动恢复；系统 IME 候选弹窗仍未验证。
- [独立插件市场规格与工单](https://github.com/Nanobug-Editor/Plugin-Marketplace/tree/main/docs)：统一维护静态注册、版本/统计和官方插件迁移的执行依据；[宿主工单 02 验收](verification/plugin-marketplace-02-2026-10-11.md)记录本分支实际测试和交付边界。
- [插件管理器共用布局](specs/plugin-manager-shared-layout.md)、[2026-10-13 验收记录](verification/plugin-manager-shared-layout-2026-10-13.md)：已安装与市场共用窗口框架，搜索框内置左侧筛选，详情按数据分区；浏览版本与实际安装实例的操作目标分别明确。实现与测试记录不继承市场服务工单的交付状态。
- [更新入口与详情呈现优化](verification/plugin-manager-presentation-2026-10-14.md)：已安装显示审核新版本，不可执行操作隐藏；资料分组、版本列表及进行中反馈集中验证，沿用已有授权和事务入口。
- [冷启动插件预览误报与首次激活回归](verification/plugin-preview-cold-callbacks-2026-10-14.md)：排查启用状态下的 FilePreview/Preview disabled 误报，保留同文档编辑期间的首次激活意图；真实 ZIP、原生输入与错误边界结果独立记录。
- [已验收的 Rust 合并方案](specs/rust-plugin-consolidation.md)与[历史验收](verification/rust-plugin-consolidation-2026-10-09.md)：市场工单 03 的外部准入依据；迁移与本次修复的结果另记，不将其他任务成果计为新实现。
- [市场工单 03 宿主集成记录](verification/plugin-marketplace-03-2026-10-11.md)：七个新命名仓库、稳定 Release、完整 ZIP 摘要、七包原生安装与代表功能已验证；PR #6 已按本批授权合并，正式 Pages、统计及客户端线上安装验收通过。
- [市场工单 04 删除前盘点](verification/plugin-marketplace-04-inventory-2026-10-11.md)：实际消费者与替代输入对应关系；源码迁出和无随附 Windows 交付已验收，证据见 [04 最终记录](verification/plugin-marketplace-04-2026-10-11.md)。

- [内置终端与运行、调试统一面板方案](specs/builtin-terminal-run-debug.md)、[四张工单与测试安排](tickets/builtin-terminal-run-debug/README.md)：2026-10-09 范围、拆分和测试接缝已批准并发布为 #87–#91，四张工单已逐步完成验收。上游 Alacritty、内置终端、构建和调试统一面板及旧用户迁移已接通；语言与调试器仍由插件提供，平台与历史测试夹具限制见最终记录。
- [内置终端 02 验收](verification/builtin-terminal-02-2026-10-09.md)：统一构建/运行任务、真实公开消费者、关闭与复用及 Windows 插件开发操作的阶段证据。
- [内置终端 03 验收](verification/builtin-terminal-03-2026-10-09.md)：唯一真实调试目标的输入、同面板检查、诊断归组和停止清理的阶段证据。
- [内置终端 04 与最终验收](verification/builtin-terminal-04-2026-10-09.md)：有限旧数据迁移、发行退役、当前包消费者与实际 Windows 组合操作，逐项汇总 N01–N26 和未验证部分。

- [插件打包与隔离开发运行](specs/plugin-development-packaging.md)、[本次验收](verification/plugin-development-packaging.md)：宿主模板与 CLI 共享项目描述，自动 ZIP、输出选择和无 ZIP 的独立开发运行。
- [插件开发工作区整合主工作区改动](verification/plugin-development-main-integration.md)：2026-10-08 用户授权整合全部未提交改动，包含两侧备份、冲突处理与重新验证；未提交或推送。
- [缺失项目描述修复与主工作区 Release 交付](verification/plugin-development-main-delivery.md)：截图错误的回归测试、主工作区整合与 Release 启动证据。

- [基础社区生态：通用插件 API 完善方案](specs/plugin-api-community-foundation.md) / [#92](https://github.com/Nanobug-Editor/Editor/issues/92)、[七张实施工单 #93–#99 与测试安排](tickets/plugin-api-community-foundation/README.md)、[实施与验收入口](verification/plugin-api-community-foundation/README.md)：2026-10-12，七单全部验收交付、普通提交和推送，#93–#99均已读回closed/completed。最后07源码交付 `8ee28952014a6533253fc83e17c3410d88a76e48` 已推送核对，最终两轴未解决项均0；源码SDK与主分支整合的历史输入见[恢复记录](verification/plugin-api-community-foundation/2026-10-11-resume.md)。稳定核心＋实验扩展，AI暂不实现，父设计议题#92保持开放且未修改。
- [01 / 02 普通合并连接验证](verification/plugin-api-community-foundation/01-02-integration.md)：真实 SDK 菜单、精确左文档版本、焦点退役回归及新增原生连接已验证，双轴未解决项均为 0；为 03 / 05 提供已修正整合点。
- [03 工作区计划与会话恢复验收](verification/plugin-api-community-foundation/03-workspace-transactions.md)、[Windows 原生记录](verification/plugin-api-community-foundation/03-workspace-transactions-native.md)：真实脚手架与分页转换消费者、混合脏文本与磁盘计划、取消及整组 Undo、精确 Native Save、Dark/字号布局和正常停止通过；既有 Settings UIA 崩溃及最终交付状态按记录核对。
- [04 多来源诊断与共享语言结果验收](verification/plugin-api-community-foundation/04-diagnostics-and-language.md)：独立检查器、直接结果与标准 LSP 的原生呈现、事务和资源退役已验收交付，#96 已读回关闭；49 文件源码 SDK、六包、17 项连接回归、规定门禁及两语言生产键盘流程通过，本机默认快捷键限制保留在记录中。
- [工单 06：云查询、密钥与私有状态验收](verification/plugin-api-community-foundation/06-cloud-and-private-state.md)：独立云翻译/托管查询 SDK 包、实际 HTTP/TLS/CONNECT、系统 Credential、四项原审查修复、Temporary 归属续修与纯源码 SDK 证据；当前完成状态及限制以记录为准。
- [07 按需激活、热配置与兼容升级验收](verification/plugin-api-community-foundation/07-activation-and-compatibility.md)、[Windows 原生记录](verification/plugin-api-community-foundation/native-07.md)、[双轴审查](verification/plugin-api-community-foundation/07-activation-and-compatibility-review.md)：实际新旧SDK、按需启动、配置热应用、退役清理与T35包组合通过；Runtime19、额外Query1、App12、最终三门禁与Windows新增原生交互已实际通过，两轴均零未解决项；源码交付已推送核对，#99已关闭。
- [05 通用树与编辑器装饰验收](verification/plugin-api-community-foundation/05-trees-and-decorations.md)、[Windows 原生记录](verification/plugin-api-community-foundation/05-trees-and-decorations-native.md)：四个独立正式包、菜单参数、编辑/Undo、屏外标记撤销、树开发重载与失败恢复通过；独立双轴归零、交付分支 `6940380` 已推送核对，#97 已读回关闭。集成点的资源和 SDK 联合验证另记。
- [通用插件接口完整性审计](specs/plugin-api-ecosystem-audit/README.md)：2026-10-08 的 58 项历史调查；语言接口已随后整合，当前实施范围见基础社区生态方案，不等于所有检查项获准实施。
- [XML 插件与通用语言编辑能力方案](specs/xml-language-tools.md) / [#72](https://github.com/Nanobug-Editor/Editor/issues/72)，[三个实施工单 #73–#75](tickets/xml-language-tools/README.md)：全部验收、推送并核对关闭；包含可替换格式化、XML/HTML 标签编辑及宿主大纲与停靠，父设计议题保持不变。
- [XML 语言工单 01 验收记录](verification/xml-language-tools/01-xml-language.md)：记录实际包、原生输入与服务结果，当前状态以记录为准。
- [格式化与标签编辑工单 02 验收记录](verification/xml-language-tools/02-format-and-tags.md)：独立提供者、XML/HTML 配对编辑及原生输入顺序。
- [宿主大纲工单 03 验收记录](verification/xml-language-tools/03-outline-and-docking.md)：结构与图标契约、原生树、折叠及四向停靠恢复。
- [02/03 共同交付记录](verification/xml-language-tools/02-03-delivery.md)：一次共同检查、SVG/XML 短组合、双轴审查及准确议题交付状态。
- [大纲点击崩溃修复](verification/xml-language-tools/04-outline-click-crash.md)：2026-10-08 用户窗口回归、标题栏实体借用修复及真实点击验证。
- [解析取消后的原生撤销回归](verification/xml-language-tools/05-parser-cancellation.md)：实际 XML 组合复验捕获的独立解析器生命周期缺陷与恢复验证。
- [大纲单一高亮与临时展开](verification/xml-language-tools/06-cursor-highlight-and-expansion.md)：光标位置唯一选中背景、默认两层及移动后收起旧自动路径。
- [Windows 插件状态原子替换恢复](verification/plugin-state-atomic-replacement.md)：启动写入短暂占用回归、有限重试、原数据保护和持续失败定位。
- [XML 分支合并与工作区推送](verification/xml-language-tools/07-main-merge-and-publication.md)：各工作区分支发布、XML 先推送再合并，以及宿主打包与快捷键的集成验证。

- [插件安装进度弹窗尺寸调整](verification/plugin-install-progress-size.md)：紧凑窗口、长进度信息滚动与底部按钮布局回归。
- [插件 UI 解耦与文件显示布局总方案](specs/plugin-ui-decoupling.md)、[五个实施工单（#62–#66）](tickets/plugin-ui-decoupling/README.md)。已在 `codex/plugin-ui-decoupling` 完成实现、验收及双轴审查，五张工单均已推送并核对关闭。
- [工单 01：Image 与文件显示验收](verification/plugin-ui-decoupling/01-image-file-views.md)。
- [Image 窗口尺寸回放修复](verification/image-resize-verification.md)：连续画布尺寸合并，保留滚轮输入与居中。
- [Image 普通图片滚轮缩放与 SVG 240px 最小初始尺寸](verification/image-zoom-verification.md)：Image 0.5.1 与通用原生图片控件验收。
- [通用可视视口能力](specs/visual-viewport.md)、[Image 0.6.0 验收](verification/image-viewport-verification.md)：缩放策略移回插件，文件图片、资源图片与画布共用公开能力。
- [工单 02：可组合布局与提供者选择验收](verification/plugin-ui-decoupling/02-composable-layouts.md)。
- [工单 03：两组底栏与共享偏好验收](verification/plugin-ui-decoupling/03-tools-and-preferences.md)。
- [工单 04：真实插件与历史显示偏好迁移验收](verification/plugin-ui-decoupling/04-plugin-migration.md)。
- [工单 05：最终契约与集成验收](verification/plugin-ui-decoupling/05-integration-and-contract.md)。
- [本地 main 合并与原有改动整合](verification/plugin-ui-decoupling/06-local-main-merge.md)：合并提交 `db53d2d`，保留合并前未提交内容与备份，按用户要求暂不推送；当前工作区的验证结果与环境限制见该记录。

- [Markdown 使用说明](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/README.md)、[总方案](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/spec.md)、[执行工单](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/tickets/README.md)、[逐单验收](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/verification/README.md)、[AI 执行入口](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/AGENTS.md)。
- [插件平台规格](specs/plugin-api-platform.md)、[平台实施工单](tickets/README.md)、[运行与构建](runtime-plugins.md)、[公开协议与 SDK](../../crates/plugin-protocol/README.md)。
- [插件管理与日志方案](specs/plugin-management-logs.md)、[对应工单](tickets/plugin-management-logs/README.md)、[管理页验收](verification/plugin-management-tabs-verification.md)。
- [运行、调试与构建父议题](https://github.com/Nanobug-Editor/Editor/issues/48)、[接手终验与工单处置](verification/run-debug-build-completion-2026-10-06.md)、[B1 UI 修复与复验](verification/run-debug-build-ui-fix-2026-10-06.md)、[历史验收](verification/run-debug-build.md)、[整批初审记录（2026-10-05）](verification/run-debug-build-review-2026-10-05.md)。当前进度以终验及复验记录为准。
- [本批调试器、受控传输与单目标握手决定](specs/run-debug-build-debugger.md)。
- [运行顶栏图标与选择语义](specs/run-debug-build.md#3-顶栏与选择语义)：2026-10-14 按用户提供的四个 SVG 统一线条粗细；运行、调试默认绿色，无活动执行、准备或调试目标时隐藏停止按钮。
- [构建与运行配置 B2：IDEA 参考设计提案](specs/run-config-idea-design.md)（用户已选 A / 方案 1）；[B3：双栏简洁版](specs/run-config-simple-design.md)（已落地原生弹窗，支持插件默认目标）；[B3 实现与验收](verification/run-config-simple-2026-10-06.md)。设计图与真实验证的范围在对应记录中区分。
- [运行配置重构：插件模板、原生表单与配置树](specs/run-config-plugin-tree.md)／[规格 #67](https://github.com/Nanobug-Editor/Editor/issues/67)（2026-10-07 产品基线及测试入口已确认）。新规格替代 B3 的表单、目录组织、提交和存储约定，不将旧验收计作新功能通过。
- [运行配置重构实施工单](tickets/run-config-plugin-tree/README.md)：4 个端到端切片 #68–#71 的实现与验收均完成，普通提交及 GitHub 交付记录见各工单。30 个验收场景的实际包、Windows 与真实调试证据见 [验收记录](verification/run-config-plugin-tree.md)。2026-10-07 用户另行授权合并主分支并推送，独立候选的冲突处理和验证见 [主分支集成记录](verification/run-config-main-merge-2026-10-07.md)。

Windows 原生验收已通过；macOS/Linux 未实测或交叉构建。七个正式插件包已在本地构建和验收，未发布 GitHub Release。完整结果以[最终契约验收](verification/plugin-api-contract-verification.md)为准。

## 方案、工单与使用文档

新增方案：[插件 UI 解耦与文件显示布局总方案](specs/plugin-ui-decoupling.md)，父议题 [#61](https://github.com/Nanobug-Editor/Editor/issues/61) 保持 open；[5 张粗粒度工单 #62–#66](tickets/plugin-ui-decoupling/README.md) 已完成并关闭，[发布记录](tickets/plugin-ui-decoupling/publication.json)保存映射。保留仅列文件的 Tab 栏，插件安排中心布局并可省略原生编辑面板；左侧窗口按钮组与右侧插件工具按钮组分离，图标、功能和显示偏好由插件提供，同工作区同类文件一起切换。[工单 01 的 Image 插件升级](tickets/plugin-ui-decoupling/01-file-views.md)已实施：由现有 SVG 扩展，SVG 保留编辑，其他支持图片只读预览，默认原尺寸、超区等比缩小。分支交付证据与本地 main 合并验证分别记录，不把原工作区的未提交内容计作分支成果。

新增方案：[运行、调试与构建总方案](specs/run-debug-build.md)，已发布为 [#48](https://github.com/Nanobug-Editor/Editor/issues/48)，对应 [12 张工单 #49–#60](tickets/run-debug-build/README.md)（产品范围、B1 紧凑表单、拆分与测试接缝均已确认，待实施；16 条原生阻塞边已核对）。覆盖顶栏统一配置／会话入口、独立配置并行、顺序启动前步骤、Rust 完整启动调试及公开执行／调试接口。

Markdown 插件：[总方案](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/spec.md)、[工单目录](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/tickets/README.md)、[AI 执行入口](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/AGENTS.md)。完整宿主能力与 Markdown 0.11.1 已合回主项目 `main`；SVG 0.3.0 同样采用底栏三个视图按钮。主目录构建、原生回归及发行文件见[主项目集成验收](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/verification/12-main-project-integration.md)。原 0.11.0 工单及完整验收保留历史证据，早期[仅迁入源码的记录](verification/markdown-source-repackage.md)不代表后续宿主集成状态。

后续缺陷：[源码区只有一行的布局修复](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/verification/13-source-editor-height.md)，直接验证工具栏下方的实际输入高度、空文档、SVG、主题与窗口重排。

后续更新：[Markdown 0.12.0 图标工具栏与首次滚动修复](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/verification/14-icon-toolbar-and-first-scroll.md)，采用统一 SVG 与 29 px 单行工具栏，底栏面板图标移至同步开关右侧，验证同版本启动刷新保留手动滚动位置。

当前后续范围：[Markdown 0.13.0 GitHub 配色、六级标题、工具栏显隐与小幅滚动](https://github.com/Nanobug-Editor/Markdown/blob/31516790679c2ab591105d11e6fd15e974b72505/docs/verification/15-github-headings-toolbar-scroll.md)。底栏图标改为仅控制顶部工具栏，旧记录中面板开关语义保留为历史说明。

已完成批次：[插件管理与运行日志改造方案](specs/plugin-management-logs.md)（[#22](https://github.com/Nanobug-Editor/Editor/issues/22) 已于 2026-10-05 核对完成并关闭），对应[3 张实施工单](tickets/plugin-management-logs/README.md)，#23–#25 均已完成并关闭。验收见[分栏验证](verification/plugin-management-tabs-verification.md)、[运行日志验证](verification/plugin-runtime-logs-verification.md)和[底栏摘要验证](verification/plugin-status-popover-verification.md)；后续本地日期与日志降序修正也已通过[补充验收](verification/plugin-log-time-order-verification.md)。本批次独立于下表已完成的平台重构。

启动修正：[Windows 插件管理器访问拒绝验收](verification/plugin-manager-access-verification.md)（2026-10-04 已完成）。确认仓库继承的 Low 完整性标签导致 `os error 5`，本机已恢复正常目录标签并移除临时启动包装；直接 `cargo run` 与 `cargo run --release` 均通过原生验收，安装状态保持完整。

后续复发：[默认构建目录访问拒绝复发验收](verification/plugin-manager-access-recurrence.md)（2026-10-06）。再次确认原仓库根和 EXE 被标为 Low，恢复根目录及默认构建输出的 Medium 继承；新 Cargo 输出和原目录 release 插件管理器均通过，5 条安装记录保留。标签外部来源尚未确定，历史已修复状态不能替代当前文件标签检查。

| 分类 | 入口 | 状态 |
| --- | --- | --- |
| 平台设计 | [插件平台方案](specs/plugin-api-platform.md) | 已完成实施，正文保留设计基线 |
| 实施工单 | [20 张工单、议题映射及依赖图](tickets/README.md) | 全部已完成 |
| 最终验收 | [T01–T26 契约验收](verification/plugin-api-contract-verification.md) | 已完成，含真实执行证据与限制 |
| 运行与构建 | [运行时插件平台](runtime-plugins.md) | 当前使用参考，随平台维护 |
| 声明式主题 | [主题插件格式](主题插件格式.md) | 格式参考，随契约维护 |
| 公开 SDK | [协议与 SDK 源码注释](../../crates/plugin-protocol/src/lib.rs) | 只导出源码、Cargo、WIT 与 JSON；Markdown 保留为维护资料 |

## 分阶段验收记录

以下记录均对应已完成工单。正文保留执行当时的范围、结果和限制；早期“后续工单”“旧协议暂存”等描述是历史阶段说明，最终行为以完整契约验收和当前 SDK 为准。

- [bootstrap](verification/plugin-api-bootstrap-verification.md)
- [composable-ui](verification/plugin-api-composable-ui-verification.md)
- [contract](verification/plugin-api-contract-verification.md)
- [data-migration](verification/plugin-api-data-migration-verification.md)
- [dependencies](verification/plugin-api-dependencies-verification.md)
- [execution-service](verification/plugin-api-execution-service-verification.md)
- [hot-update](verification/plugin-api-hot-update-verification.md)
- [installed-upgrade](verification/plugin-api-installed-upgrade-verification.md)
- [installers](verification/plugin-api-installers-verification.md)
- [language-migration](verification/plugin-api-language-migration-verification.md)
- [language](verification/plugin-api-language-verification.md)
- [lsp](verification/plugin-api-lsp-verification.md)
- [process](verification/plugin-api-process-verification.md)
- [recovery](verification/plugin-api-recovery-verification.md)
- [requests](verification/plugin-api-requests-verification.md)
- [scopes](verification/plugin-api-scopes-verification.md)
- [services](verification/plugin-api-services-verification.md)
- [settings](verification/plugin-api-settings-verification.md)
- [terminal-migration](verification/plugin-api-terminal-migration-verification.md)
- [ui-migration](verification/plugin-api-ui-migration-verification.md)

## 插件包说明

- [内置终端、运行与调试](specs/builtin-terminal-run-debug.md)：终端不再独立发行插件包；语言规则和调试器仍通过通用插件接口接入。
- [Image for Nanobug](https://github.com/Nanobug-Editor/Image)（安装 ID 为 `svg`）
- [Rust for Nanobug](https://github.com/Nanobug-Editor/Rust)
- [TOML for Nanobug](https://github.com/Nanobug-Editor/TOML)
- [HTML for Nanobug](https://github.com/Nanobug-Editor/HTML)
- [JavaScript for Nanobug](https://github.com/Nanobug-Editor/JavaScript)
- [XML for Nanobug](https://github.com/Nanobug-Editor/XML)
- [Markdown for Nanobug](https://github.com/Nanobug-Editor/Markdown)
- [Markdown 内嵌 HTML 原生预览](verification/markdown-embedded-html.md)（0.18.0 本地候选：原生 HTML 子集、受控图片与链接、真实包验证；未发布市场）。
- [Markdown HTML 图片尺寸修复](verification/markdown-image-dimensions.md)（0.18.1 本地候选与匹配宿主：width/height、单边比例、窄栏缩小和说明布局；83 项单元及 3 项原生场景通过）。
- [Markdown GitHub README 呈现](verification/markdown-github-readme-presentation.md)（0.18.2 本地候选：align、alt 无障碍语义、公开原生样式；验证范围和状态见记录）。
- [Markdown 输入与增量预览验收](https://github.com/Nanobug-Editor/Markdown/blob/main/docs/verification/17-incremental-input-viewport.md)（0.15.0：用户实测输入仍卡顿，性能验收重新打开）
- [Markdown 源码输入与过期视口通知](https://github.com/Nanobug-Editor/Markdown/blob/main/docs/verification/18-source-input-and-stale-viewport.md)（宿主跟进修复与实测限制）
- [冻结 Package/SDK 验收夹具](../../crates/plugin-runtime/tests/fixtures/packages/README.md)：保留示例覆盖，不属于应用、安装器、SDK 或市场发行目录。
- [插件市场 04 删除盘点](verification/plugin-marketplace-04-inventory-2026-10-11.md)：源码迁移与受影响消费者的对应关系。
- [插件市场 04 最终实施与验收](verification/plugin-marketplace-04-2026-10-11.md)：无随附产物、686 项应用和 188 项实际 WASM 回归、失败历史及 M01–M26 汇总；GitHub 交付状态以所链接 PR 和工单读回为准。
- [社区公开 SDK 消费夹具](../../crates/plugin-runtime/tests/fixtures/community/README.md)：保留本批文档、语言、树、云查询与生命周期的构建源码和固定版本ZIP，不进入宿主、安装器、SDK或市场发行。
- [2026-10-12 社区分支合并主分支](verification/plugin-api-community-foundation/main-integration-2026-10-12.md)：双方契约、无随附发行、当前 SDK 构建输入、合并回归和原工作区恢复记录。

## 后续管理约定

插件系统的新方案、扩展、迁移和缺陷工单均归入本目录：`specs/` 保存方案，`tickets/` 保存工单与议题映射，`verification/` 保存验收。使用能区分主题或批次的文件名或子目录，保留本轮 01–20 的历史编号，并更新本索引。

每份方案和工单标明实际状态、对应规格、相关工单和验收入口；完成后同步状态与验收项。新任务不继承本轮“已完成”状态或提交、推送、关闭授权。历史记录不覆盖最终完成状态，也不把未验证的平台或未发布的资产标成完成。

返回[文档总目录](../README.md)。

已补齐 `specs/`、`tickets/`、`verification/` 中的平台历史资料；原 `docs/specs/` 保留旧路径记录，不以旧副本覆盖现行管理与日志文档。内容不同的早期资料见[2026-10-03 历史入口](archive/2026-10-03/README.md)。
