# 项目首页、多窗口与 Windows 目录右键打开规格

日期：2026-10-12

稳定标识：`ui-project-windows`

状态：产品行为与两个测试边界已确认（2026-10-12），待实施。本文不表示原生功能已实现或通过验收。

发布目标：`T-miracle/Nanobug` GitHub Issues，标签 `ready-for-agent`。旧跟踪器名称 `T-miracle/Editor` 已核对解析到同一仓库。本次仅整理和发布总规格，不自动拆分工单、提交代码或启动实现。

已发布：[设计规格 #102](https://github.com/T-miracle/Nanobug/issues/102)。标题、完整正文及标签已读回核对；真实映射见[发布记录](project-windows.publication.json)。

## Problem Statement

用户启动 Nanobug 时会直接进入进程的当前目录，不能先选择要处理的项目。从安装快捷方式启动时，工作目录可能是用户文档目录，这与用户想编辑的项目无关。当前没有完整的项目首页、最近项目列表和应用级打开项目流程。

用户需要同时处理多个项目，但当前应用把一个项目与整个应用的生命周期绑定在一起：部分语言提供者和插件贡献是全局的，关闭清理最终退出应用。简单再创建一个窗口，不能保证两个项目的文档、语言工具、插件资源、终端和配置互不干扰。

用户还希望在 Windows 文件资源管理器中右键打开文件夹，不必先启动编辑器再定位项目。外部打开请求需要与应用内行为一致，并能激活同项目的已有窗口，避免重复窗口和相互覆盖的会话状态。

## Solution

提供原生项目首页、多项目窗口和 Windows 目录右键打开入口。一个项目就是一个文件夹，无需 Git 仓库、专用项目文件或初始化操作；每个项目窗口管理一个单根工作区，同一项目只保留一个窗口。

无参数启动时显示首页，提供“打开文件夹”和最近项目；已有首页时激活它，否则新开首页，不替换现有项目窗口。显式传入目录时直接进入对应项目流程。从首页选择项目使用该首页窗口，从已有项目窗口或资源管理器打开其他项目默认新开窗口；重复打开同项目时激活已有窗口。

“关闭项目”回到首页，“关闭窗口”只结束当前窗口，“退出应用”协调全部窗口。未保存内容和运行程序通过统一的关闭确认处理。第一次打开没有信任记录的目录时，选择“信任并打开”“以受限模式打开”或“取消”。

最近项目显示项目名称和完整路径，按最近打开时间排序，可移除记录。重新打开受信任项目时恢复已保存的标签、活动文件和布局；普通启动不恢复上次的全部项目窗口，也不重启上次的程序。

## User Stories

1. As an editor user, I want a project home when starting without arguments, so that I can choose my work instead of editing an incidental current directory.
2. As an editor user, I want an existing home window activated on another argument-free launch, so that repeated launches do not create unnecessary home windows.
3. As an editor user, I want a new home window when only project windows exist, so that opening the launcher does not replace my work.
4. As an editor user, I want an explicit directory launch to open that project, so that command-line and desktop entry points remain useful.
5. As an editor user, I want any ordinary folder to serve as a project, so that I do not need a repository or an initialization step.
6. As an editor user, I want to select a folder through the native directory picker, so that I can navigate my computer using familiar controls.
7. As an editor user, I want cancelling folder selection to preserve the current view, so that browsing for a project has no unwanted effect.
8. As an editor user, I want a project selected on the home page to occupy that window, so that I can start work without leaving an unnecessary home window behind.
9. As an editor user, I want a different project opened from a project window to use a new window, so that both projects remain available.
10. As an editor user, I want different projects open at the same time, so that I can work across related codebases.
11. As an editor user, I want each project window to have one root folder, so that its files and tools have an unambiguous scope.
12. As an editor user, I want reopening a project to activate its existing window, so that I do not create competing editable sessions.
13. As an editor user, I want equivalent directory paths to identify the same project, so that path spelling does not create duplicate windows.
14. As an editor user, I want a failed project opening to leave existing windows intact, so that an inaccessible directory does not interrupt my work.
15. As an editor user, I want recent projects listed on the home page, so that I can resume work quickly.
16. As an editor user, I want recent projects sorted by last opening time, so that the most relevant projects are easy to find.
17. As an editor user, I want both project names and full paths shown, so that I can distinguish folders with the same name.
18. As an editor user, I want to remove a recent-project record without deleting its files, so that I can tidy the list safely.
19. As an editor user, I want missing recent-project directories reported without recreating them, so that moved or disconnected projects are not silently replaced.
20. As an editor user, I want missing-directory records retained until I remove them, so that a temporarily unavailable project is not forgotten.
21. As an editor user, I want recent projects retained between launches, so that the home page remains useful after restarting.
22. As a Windows user, I want “使用 Nanobug 打开” on a folder's context menu, so that I can open the selected folder directly.
23. As a Windows user, I want the same action on a directory's background context menu, so that I can open the directory I am currently viewing.
24. As a Windows user, I want context-menu actions to pass the intended path explicitly, so that the launched process's working directory does not select the wrong project.
25. As a Windows user, I want context-menu launches to reach the running application, so that reopening a project activates its existing window.
26. As a Windows user, I want project paths containing spaces or Chinese characters to open correctly, so that ordinary directory names work reliably.
27. As an editor user, I want a first-time trust choice for an unfamiliar project, so that opening a directory does not silently authorize its tools.
28. As an editor user, I want to edit in restricted mode without starting plugins or language tools, so that I can inspect an unfamiliar project.
29. As an editor user, I want an existing local trust choice preserved, so that known projects do not require repeated confirmation.
30. As an editor user, I want project configuration unable to grant trust or permissions, so that a project's files cannot authorize their own execution.
31. As an editor user, I want language providers and plugin contributions scoped to each project, so that opening another project does not change my current project's behavior.
32. As an editor user, I want terminal and tool resources owned by their project, so that stopping one project does not stop another project's work.
33. As an editor user, I want each document's content, input state and undo history preserved in its proper session, so that window changes do not mix editing state.
34. As an editor user, I want late results from closed or changed projects rejected, so that stale language or search results cannot affect another window.
35. As an editor user, I want user preferences and installed packages coordinated across windows, so that concurrent windows do not overwrite shared settings or contend for the same runtime store.
36. As an editor user, I want “关闭项目” to return the current window to the home page, so that I can choose another project without exiting the application.
37. As an editor user, I want the window close button to close only that window, so that other project windows keep working.
38. As an editor user, I want closing the last window to exit the application, so that no unrequested background application remains.
39. As an editor user, I want an application exit action to coordinate every window, so that I can end the whole session deliberately.
40. As an editor user, I want Save and Close, Discard Changes, and Cancel for unsaved documents, so that I control what happens to my work.
41. As an editor user, I want save failures to prevent closing, so that failed persistence is not treated as success.
42. As an editor user, I want pending input included in the unsaved check, so that the last input is not lost at a close boundary.
43. As an editor user, I want confirmation before stopping running project tasks or terminal programs, so that closing a window does not unexpectedly terminate active work.
44. As an editor user, I want an application-wide exit cancellation to retain all windows, so that an earlier window is not already gone when I cancel a later decision.
45. As an editor user, I want reopening a trusted project to restore saved tabs, its active file and layout, so that I can resume in a familiar workspace.
46. As an editor user, I want normal startup to remain on the home page rather than reopen all previous windows, so that I decide which projects to resume.
47. As an editor user, I want reopening a project to avoid restarting previous programs automatically, so that layout restoration does not repeat external actions.
48. As a Windows user, I want a default-selected installer option for both context-menu entries, so that I can enable or decline desktop integration.
49. As a Windows user, I want this integration limited to my account without elevation, so that installation follows the existing per-user model.
50. As a Windows user, I want upgrades to preserve my context-menu choice, so that an update does not undo my preference.
51. As a Windows user, I want uninstall to remove Nanobug's menu entries while preserving my settings and project data, so that cleanup respects ownership.
52. As a portable-edition user, I want launching the editor to avoid automatic context-menu registration, so that a portable application does not silently modify desktop integration.
53. As an editor user, I want the new controls usable in Chinese and English and in both themes, so that they match existing editor preferences.
54. As a keyboard and IME user, I want predictable focus, keyboard operation, scaling and input behavior, so that project selection and closing remain accessible.
55. As a plugin developer, I want isolated development profiles and command-line build tools to retain their behavior, so that normal application instance coordination does not intercept development commands.

## Implementation Decisions

### 领域与所有权

- 产品中的“项目”对应一个文件夹及其单根工作区；不引入专用项目文件、项目创建向导或 Git 前置条件。
- “窗口”在本文指操作系统级 Nanobug 窗口；现有宿主消息窗口、插件窗口等编辑器内面板继续遵守自己的定义，不被升级为独立系统窗口。
- 应用层管理首页与项目窗口登记、统一打开请求、最近项目、共享用户设置、插件安装协调以及应用退出。
- 项目作用域管理规范化根目录、信任、项目设置、有效贡献与提供者、工作区插件实例、语言服务、终端、监听及异步任务的归属和取消。
- 窗口层管理所展示的首页或项目、标签、焦点、IME、布局和弹窗。一个项目最多有一个窗口，不建立同项目多窗口共享可变文档模型。
- EditorState 和 DocumentSession 继续承担既有文档状态职责，不增加第二份可变文本或 Undo/Redo 栈。

### 启动与统一打开流程

| 触发 | 已确认结果 |
| --- | --- |
| 无参数启动且没有首页 | 创建首页；已有项目窗口保留 |
| 无参数启动且已有首页 | 激活已有首页 |
| 显式传入目录 | 校验目录并进入统一项目打开流程 |
| 首页选择未打开项目 | 在发起选择的首页窗口打开 |
| 项目窗口打开其他项目 | 创建新项目窗口，不替换原项目 |
| 资源管理器打开未打开项目 | 创建项目窗口，不替换已有项目窗口 |
| 任一入口再次打开同项目 | 激活已有项目窗口，不创建第二份项目会话 |
| 选择取消、路径无效或打开失败 | 保留原有窗口与文档；需要时展示明确错误 |

- 首页是没有项目的真实应用状态，不先构造当前目录工作区，也不为占位项目启动插件、语言工具、终端或目录监听。
- 复用既有 Workspace 目录校验和规范化能力。项目身份基于实际解析后的目录；路径大小写与别名处理遵从文件系统语义，不仅按显示字符串去重。
- 多个同时到达的打开请求也必须遵守同项目单窗口约束，不能先各自创建再覆盖会话。
- 普通 GUI 的外部打开请求由既有应用统一协调。插件构建、SDK、清理等工具命令，以及隔离的插件开发实例，不被转发成普通项目窗口请求。
- 保留已有显式文件路径启动行为：打开其所属工作区并定位文件；不把本次目录右键功能扩展为新的文件关联策略。
- 具体跨进程传输机制属于实现细化，不在本规格中锁定。外部路径必须保持参数边界，不通过 Shell 拼接执行。

### 最近项目、会话与信任

- 最近项目为应用级本机记录，显示名称、完整路径并按最近打开时间排序；再次打开更新同一条记录，不制造重复项目。
- 移除记录只影响列表，不删除项目目录、文件、历史、信任或插件数据。
- 缺失目录展示错误并保留记录，不自动创建替代目录，不静默删除记录。
- 复用按工作区保存的会话机制；受信任项目恢复已保存标签、活动文件与布局。会话恢复不等于恢复未保存文本、Undo/Redo 内存栈、进程或外部副作用。
- 无参数启动始终遵守首页规则，不自动批量恢复上次所有项目窗口；重新打开项目不自动重启上次任务或终端程序。
- 没有信任记录的新项目提供“信任并打开 / 以受限模式打开 / 取消”。选择在本机按项目保存，已有有效信任记录沿用，不根据项目配置或路径名称授予信任。
- 受限工作区允许编辑，但不启动插件和语言工具、不采用项目工具路径；当前默认信任新工作区的初始化行为需要由显式决策替代。
- 沿用现有用户数据位置与持久化标识，不能通过清空旧会话或插件数据实现迁移。新增最近项目及协调状态不写入用户项目目录。

### 多项目运行隔离

- 当前全局单工作区的语言提供者及有效插件贡献需要变为明确作用域的状态；打开、激活或关闭一个窗口不得整体覆盖另一个项目的有效配置。
- 插件包、依赖缓存等可共享；项目运行实例、进程、订阅、面板、请求和资源句柄必须具有可撤销的作用域归属。
- 保留现有共享存储锁、安装事务及私有数据保护；不能每窗复制完整管理器后争用同一个独占目录，也不能删除锁来掩盖竞争。
- 单个管理器的“切换并停放旧工作区”不等于两个工作区并行运行。本次实现必须使两个项目同时保持可用，而不是焦点切换时轮流暂停另一个项目。
- 用户配置、语言、主题及用户快捷键按应用级权威协调并传递到窗口；焦点、命令目标与项目设置保留各自作用域，不产生多份相互覆盖的用户配置快照。
- 异步结果在应用前校验目标窗口、工作区/实例存活性和文档 revision；已关闭项目的结果不得落入随后打开的项目。
- 所有插件继续使用公开、可复用的能力接口，不引入插件 ID、语言或服务器白名单。若实现确实需要公开契约扩展，按既有版本、权限与消费者同步规则处理，不预设必须增加专属多窗口 API。

### 关闭项目、窗口与应用

| 动作 | 成功后的状态 |
| --- | --- |
| 关闭项目 | 释放当前项目资源，当前窗口回首页，其他窗口保留 |
| 关闭窗口 | 只关闭当前窗口；其他项目继续运行 |
| 关闭最后一个窗口 | 完成资源清理并退出应用 |
| 退出应用 | 收集全部窗口的关闭决定后统一完成退出 |

- 关闭判断之前处理待应用输入，确保 IME 与待同步文本进入真实文档状态。
- 存在未保存内容时提供“保存并关闭 / 放弃修改 / 取消”；保存失败或冲突时阻止关闭并保留可编辑状态。
- 项目仍有运行任务或终端程序时，确认后停止所属程序并完成必要清理；拒绝停止或取消则保留项目。
- 应用级退出先汇总全部决定，不在仍可能收到取消时提前销毁窗口、丢弃文档或停止运行资源。保存完成不等于必须关闭；退出取消不承诺撤回已经成功写入磁盘的显式保存。
- 确认收集期间状态发生变化时，关闭前重新校验，不能用过期确认丢弃后续输入。
- 收尾覆盖项目监听、语言服务、终端、插件实例及所属请求、订阅和传输；项目级关闭不能直接触发整个应用退出，也不能停止其他项目或应用作用域的共享资源。
- 不承诺对已停止程序的外部副作用进行回滚。资源清理失败应明确报告，不能假称全部关闭成功。

### Windows 目录右键与原生界面

- 文件夹对象和文件夹空白处均提供“使用 Nanobug 打开”：前者传选中目录，后者传当前目录；英文界面保留对应翻译。
- 两个入口均向统一打开流程提交明确路径，支持中文和空格等合法目录名称，不依赖快捷方式或子进程的当前工作目录。
- 安装器增加默认勾选的可选任务，同时控制两个菜单入口；维持当前用户安装，不引入管理员提权或全局环境修改。
- 升级保留已有选择，卸载仅移除本产品拥有的菜单项，并保留既有用户设置、插件、历史及项目数据；便携版不自动注册菜单。
- 首页、最近项目列表与确认弹窗使用 GPUI 原生实现，以 gpui-base 行为和项目本地 UI 外观为基础，保留双语、深浅主题、键盘、焦点、IME、滚动和缩放行为。
- 对话原型仅表达首页、独立项目窗口、右键入口和确认流程；不把场景切换按钮做成产品导航，不把简化的编辑区或资源管理器菜单当作现有界面的替换要求。

### 与历史基线的关系

- 本次用户明确要求替代历史“首版不做多窗口”的限制；继续保留每个工作区单根、原生 GPUI、唯一文档状态、revision 校验、动态 grammar 及通用插件边界。
- 本规格扩展的是多个工作区同时运行的产品能力，不否定旧插件平台对工作区隔离、权限和资源回收的规定。
- 新工作区默认信任改为显式信任选择；既有有效本机记录及私有数据继续保留。
- 其他未明确替代的产品限制继续有效，不顺带扩展为多根工作区、自由停靠或调试系统。

## Testing Decisions

### 已确认的测试接缝

采用一个主要应用级接缝，加一个不可由应用内测试替代的 Windows 系统集成边界：

1. **应用级项目生命周期：打开请求 → 真实 GPUI 首页/项目窗口 → 文档及项目资源 → 关闭与会话恢复。** 复用现有 GPUI 应用测试、Workspace、DocumentSession、会话存储和公开插件管理入口。新的统一打开/关闭协调入口既服务生产请求，也作为高层测试入口；不另造只供测试的窗口工厂、插件注入 API 或每个子模块一套薄包装。
2. **Windows 安装与外部启动：安装器 → 真实目录右键 → 外部启动转发 → 同一应用级流程。** 使用隔离安装/用户数据环境验证安装、升级和卸载，再从实际文件资源管理器验证目录与空白处两个入口。进程内模拟不能代替注册表归属、路径传参和已有实例激活的系统验收。

用户已于 2026-10-12 明确确认采用这两个测试边界。插件包、可控语言服务、文件系统和进程夹具是主路径的已有依赖边界，不再为其增加本功能专属入口。

### 良好测试与现有先例

- 测试外部可观察行为：窗口是否出现及激活、文档内容是否保留、文件是否保存、提示是否可见、所属资源是否结束、另一个项目是否仍能编辑和运行；不把私有映射字段、辅助函数调用次数或实现枚举当作主要验收。
- 复用现有原生终端测试通过 GPUI 创建真实窗口、模拟按钮点击并检查绘制结果的方式。
- 复用已安装语言与 SVG 组合测试中的临时工作区、真实包、文档编辑、语言能力、重启恢复验证；保留其从包到宿主的高层驱动，不照搬仅断言内部结构的部分。
- 复用运行生命周期测试中可控原生程序产生就绪/清理证据的方式；原先“切换后停止旧项目”用例不能证明本次要求的两个项目同时活跃，需要增加相应行为覆盖。
- 会话、路径、信任及取消边界可补针对性回归，但不能代替双窗口联合测试。
- 测试使用临时目录、隔离配置和受控夹具，不改动用户真实项目、已安装插件或全局工具链。

### 验收矩阵

| 编号 | 场景 | 可观察通过标准 |
| --- | --- | --- |
| P01 | 无参数启动与再次启动 | 首页显示；不加载当前目录；已有首页被激活；只有项目窗时新开首页且原项目不变 |
| P02 | 首页打开与取消 | 原生选择器选定后当前首页成为项目窗口；取消保持首页及已有项目 |
| P03 | 项目窗打开其他项目 | 两个系统窗口同时存在，原文档、焦点目标和运行资源不被替换 |
| P04 | 同项目重复及并发打开 | 应用内、外部启动、规范化等价路径均只产生一个项目窗口并正确激活 |
| P05 | 无效或不可访问路径 | 清楚报告失败，已有窗口和未保存文档保留，不错误回退到当前目录 |
| P06 | 最近项目 | 名称和完整路径、排序、去重及跨启动保留正确；移除不删除文件或项目数据 |
| P07 | 最近项目目录缺失 | 提示并保留记录，不创建替代目录、不自动删除记录 |
| P08 | 第一次打开的信任决定 | 信任、受限和取消各自产生正确结果；取消不启动该项目的工具 |
| P09 | 受限项目与信任恢复 | 受限项目可编辑，无插件/语言工具启动及项目工具路径采用；已有有效记录沿用 |
| P10 | 双项目语言与插件 | 不同配置和提供者在两个窗口同时生效，焦点切换不覆盖或停放另一项目 |
| P11 | 双项目文档与异步结果 | 输入、IME、Undo/Redo 独立；关闭后迟到的扫描、搜索或语言结果不污染另一个项目 |
| P12 | 共享设置及安装协调 | 用户设置变更一致可见且不互相覆盖；第二窗口不因争用共享运行时锁而失去插件能力 |
| P13 | 关闭项目 | 回到同一窗口的首页，所属资源撤销；其他窗口和应用级共享资源继续有效 |
| P14 | 关闭窗口与最后窗口 | 仅当前窗口消失；最后窗口关闭才退出应用，无所属后台资源遗留 |
| P15 | 未保存三种选择 | 保存后关闭、明确放弃后关闭、取消保持编辑状态；包含待应用输入 |
| P16 | 保存失败或冲突 | 不关闭，不丢弃未保存文本，错误可见且能够继续编辑 |
| P17 | 运行任务和终端 | 关闭确认后只停止所属项目的程序；取消时保持运行；不把停止描述为副作用回滚 |
| P18 | 应用退出汇总 | 任一窗口取消时所有窗口仍在；收集期间新增输入需重新校验；不存在提前丢弃或停止 |
| P19 | 项目重新打开 | 受信任项目恢复标签、活动文件和布局；不恢复程序；普通启动不自动打开全部旧窗口 |
| P20 | 右键文件夹和空白处 | 实际资源管理器传递各自的正确目录；中文、空格路径可用；重复打开激活已有窗口 |
| P21 | 外部实例与隔离工具 | 已运行 GUI 接收打开请求；独立插件开发 profile 和工具 CLI 不被错误接管 |
| P22 | 安装、升级和卸载 | 当前用户可选且首次默认勾选两个入口；升级保留选择；卸载仅清理菜单等安装归属内容 |
| P23 | 便携版 | 普通运行不自动注册右键菜单，目录参数打开仍遵守相同项目规则 |
| P24 | 原生交互 | 中英文、深浅主题、焦点、键盘、IME、缩放及最近项目滚动均有真实可见结果 |

### 执行与报告要求

- 实现阶段先运行受影响模块的针对性测试，再执行仓库规定的 Rust 格式、非 UI workspace 测试和 workspace 编译检查，并追加相关 editor-app 测试及 Windows 原生交互验收。
- 依赖真实 WASM 或原生程序的 ignored 用例按当前直接构建入口准备夹具，显式运行；未运行、跳过或缺少工具的场景分别记录，不能算通过。
- 原型中已检查的场景切换、示例窗口复用、保存/取消、受限打开及窄屏布局，只是原型验证，不是 GPUI、多进程、插件隔离或安装器的验收证据。
- 本次规格整理仅检查文档结构、链接与差异，不运行无关的完整 Rust 构建；上述新增验收项均尚未执行。

## Out of Scope

- 一个窗口包含多个根目录的多根工作区，专用项目文件、项目初始化向导或必须依赖 Git 的项目模型。
- 同一项目同时打开多个窗口，以及跨窗口共享同一份可变文档的协同模型。
- 默认替换当前项目、每次询问窗口去向、自动恢复上次全部项目窗口等未采用的打开策略。
- 崩溃后恢复未保存文本、持久化会话 Undo/Redo 或重建原生程序内存与外部副作用。
- 便携版自动注册右键菜单、全局管理员安装、新的文件类型关联及全局 PATH 修改。
- 自由停靠、编辑组、调试系统、新任务系统或与本次多项目运行无关的新插件能力。
- 将原型场景导航、示例数据或简化编辑器界面直接交付到产品；本功能继续使用既有编辑器界面。
- 本轮直接实现功能、发布安装包、提交或推送 Git、自动拆分/关闭工单。

## Further Notes

### 依据与现状

- [需求基线](../../project/需求整理.md)、[开发计划](../../project/开发计划.md)：作为历史基线；本规格明确替代其中的多窗口排除项，其他约束继续有效。
- [领域约定](../../agents/domain.md)、[插件平台规格](../../plugins/specs/plugin-api-platform.md)：沿用宿主、贡献、提供者、实例、资源句柄、工作区作用域和文档 revision 术语。仓库未检索到独立 ADR 或 CONTEXT/GLOSSARY 文档，不虚构已存在的决策记录。
- [UI 文档入口](../README.md)、[安装器说明](../../../installer/README.md)：实施时继续沿用本地控件和直接构建工具约束。
- [议题跟踪器](../../agents/issue-tracker.md)、[标签约定](../../agents/triage-labels.md)：发布到已核对的现行仓库，使用 `ready-for-agent`；本轮不进行其他分诊或指派。
- 现有 Workspace 能验证并规范化目录；项目会话按工作区分别保存；原生目录选择器已有使用先例。这些是复用基础，不等于首页或多窗口已存在。
- 现有关闭流程缺少统一的未保存守卫，语言提供者与贡献存在单工作区全局状态，插件运行目录采用独占锁。以上是实现前需要解决的事实，不因已有切换工作区接口而视为多窗口完成。
- 现有安装器只有按用户安装和快捷方式，没有目录右键注册；安装保护 mutex 也不是普通 GUI 的单实例打开请求协调机制。

### 建议实施顺序

1. 建立无项目首页、窗口登记和统一打开请求的纵向流程，保留文件参数、工具命令与隔离开发实例的分流。
2. 使两个真实项目同时运行，完成语言提供者、贡献、运行资源与共享存储的正确归属和协调。
3. 接通关闭项目、关闭窗口、应用退出、未保存保护、信任和会话恢复，覆盖失败与取消。
4. 完成首页与最近项目的原生交互、安装器右键注册及真实 Windows 联合验收。

该顺序是落实总规格的建议，不等同于已批准的工单拆分，也不授权将只会打开第二窗口的中间状态标记为功能完成。

### 发布状态

- 已确认：本次对话中的产品范围、窗口路由、关闭、信任、最近项目和安装器选项。
- 已确认：本文 Testing Decisions 提出的一个应用级主接缝及 Windows 系统集成边界。
- 已发布：[GitHub #102](https://github.com/T-miracle/Nanobug/issues/102)，标签 `ready-for-agent`；2026-10-12 已读回核对标题、完整正文和标签。发布前已按稳定标识及标题检查重复议题。
- 未实施：不将原型或文档状态作为原生功能交付证明。

<!-- ui-project-windows:spec:2026-10-12 -->
