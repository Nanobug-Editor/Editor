# Zed 终端参考基线与 Nanobug 差异盘点

日期：2026-10-24（Asia/Shanghai）

状态：已完成源码调研；下表记录本轮实施开始时的差异，不代表新增功能已经通过验收。后续实现状态、命令结果及 Windows 原生操作必须由本轮验证记录补充，不能用这份盘点声明功能完成。

范围：内置终端、受控 PTY、终端内容交互及与现有运行/调试的连接。不把 Zed 的多工作区、远程开发、AI、REPL、编辑组等产品能力整体引入 Nanobug。

## 1. 固定参考与依据

| 对象 | 本轮实际固定值 | 依据 |
| --- | --- | --- |
| Zed 主仓库 | `d93bf740852672fb299fcbdba3af1d5fd0253e92`，提交时间 2026-10-24 13:25:58 UTC | [提交][Z-Commit]；本轮通过 GitHub 连接读取 main 后固定，后续引用不跟随 main 漂移 |
| Zed Alacritty 依赖 | `https://github.com/zed-industries/alacritty`，`rev = 4c129667ce56611becdc82de6e28218c80e2e88f` | [Cargo.toml 第 535 行][Z-Cargo]；[Cargo.lock][Z-Lock] 的包版本为 `0.26.1-dev` |
| Nanobug 既有核心 | crates.io `alacritty_terminal = 0.26.0`；包内 `.cargo_vcs_info.json` 指向 `94e7c8874e526b1e67b349d9ba30ddf81669119e`，目录 `alacritty_terminal` | 本机实际 Cargo 注册表包；对应 [Alacritty 0.17.0 提交][A-Release] |
| Nanobug 本轮实现前工作树 | HEAD `561aa5b2006c38668a083482156646a16cd02b3e`，另有大量未提交修改 | 本轮读取 git 状态与实际文件；这些已有修改不作为终端增强的交付成果 |
| 产品行为说明 | Zed 官方终端文档；固定 commit 的同页 Markdown 另作稳定参考 | [在线文档](https://zed.dev/docs/terminal)、[固定文档源码][Z-Docs] |

历史检索成功，实际相关项为 `memory_id=f8f1878b-fdfd-4e58-ac21-17f35ba6f1b7`（无 document_id）：🧠 From Hindsight memory (Nanobug) 提到 Alacritty 与宿主 UI 职责分开、搜索与 Vi 底层能力已有。这里只把它作为查找线索；下面结论均重新核对当前代码和一手来源，不沿用早期 WASM 终端决策。

本轮用户明确要求将实际使用的 Alacritty 实现代码融入 `editor-terminal`，不允许整包复制到 `vendor`，也不保留独立上游终端包或分支。此要求替代[原内置终端规格](builtin-terminal-run-debug.md)中“直接采用上游依赖”的旧方案。其余信任、受控进程、独立任务、现有 Ctrl+C/V、存储与迁移规则继续有效。

## 2. 核心与宿主的边界

Alacritty 核心负责 VT 解析、网格、滚动历史、选择、搜索与 Vi 光标算法，提供渲染数据和损坏区域查询。它不提供 Nanobug 或 Zed 的 GPUI 搜索栏、命令路由、分屏容器、链接打开、编辑器跳转和任务编排。[核心 Term][A-Term]、[搜索][A-Search]、[Vi][A-Vi]

本轮分别读取并比较两个固定 commit 的 `alacritty_terminal/src/term/mod.rs`、`term/search.rs`、`vi_mode.rs`；Nanobug 既有发布基线与 Zed fork 的这三个文件内容完全相同。因此搜索和 Vi 集成无需移植另一套算法，也无需仅为这两个能力升级至 Zed 的开发包版本。[上游 Term][A-Term]、[Zed fork Term][AZ-Term]、[上游搜索][A-Search]、[Zed fork 搜索][AZ-Search]、[上游 Vi][A-Vi]、[Zed fork Vi][AZ-Vi]

Zed 的核心 fork 确有原生 PTY 维护：其 Unix Options 增加可选 child signal mask，创建子进程时应用前台线程捕获的 mask，且有后台线程阻塞信号后的实际子进程回归。固定 fork 同时包含 PTY 设为非阻塞失败时返回错误的上游修复。这属于 Unix 进程启动路径，不是搜索、Vi 或 GPUI 界面的来源。GitHub compare 的文件列表是从共同祖先生成，不能把“16 ahead / 4 behind”误读为两个 tip 的逐文件差异。[fork 比较][A-Compare]、[Unix PTY][AZ-Unix]

Nanobug 已有受控进程资源、ConPTY 和插件公开执行入口，应沿用它们，不因引入本地核心再次复制 PTY 管理或绕过信任/归属。参考 Zed 的行为，由 Nanobug 现有模块自行实现宿主连接，避免将 Zed 的项目模型、workspace 类型或特定语言策略一起复制。[现有终端模块](../../../crates/editor-app/src/terminal/mod.rs)、[任务连接](../../../crates/editor-app/src/terminal/tasks.rs)、[插件平台规格](plugin-api-platform.md)

## 3. 实施开始时的功能矩阵

“已有”指找到当前实现入口；不等于本轮已经重跑原生验收。“缺失”指已有核心能力未在当前终端宿主接入，或没有对应用户入口。“差异”需要兼容设置或明确新入口，不能覆盖既有行为。

| 主题 | Zed 固定基线 | Nanobug 实施开始时 | 落地边界与验收重点 |
| --- | --- | --- | --- |
| 内容搜索与定位 | SearchableItem 接入通用搜索栏，literal 转义、regex、匹配高亮、匹配导航、从选择预填；后台调用核心 RegexIter。该终端实现明确关闭独立 case/word/replacement/selection/select-all 等选项。[视图搜索][Z-Search]、[核心适配][Z-Alacritty] | **缺失**：Engine 无搜索状态，终端没有搜索栏或匹配定位入口。[Engine](../../../crates/editor-terminal/src/engine.rs)、[视图](../../../crates/editor-app/src/terminal/view.rs) | 接入 Alacritty search，不另造字符串到格子的反向索引；覆盖软换行、中文宽字符、历史滚动、无结果/非法 regex、输出/resize 后过期坐标。若提供 case/whole-word，为 Nanobug 新增便利，不称为 Zed 已有开关。 |
| Vi 导航与选择 | 宿主映射 h/j/k/l、w/b/e、%/0/^/$、H/M/L、段落、g/G、页/半页滚动、v/V、y、Esc、i；算法调用核心。[Vi 宿主][Z-Vi]、[核心 Vi][A-Vi] | **缺失**：没有切换、导航、Vi 选择或光标渲染。[交互](../../../crates/editor-app/src/terminal/interaction.rs) | 保留普通 Shell 快捷键，显式启用 Vi；Vi 模式中按键不得送给子进程；退出后恢复输入、光标和焦点。 |
| URL、OSC 8 与路径 | 识别 URL、OSC8、路径/行列；Windows/Unix 路径风格；悬停与点击分开解析；支持鼠标报告模式下的显式链接手势。[识别][Z-Links]、[视图目标][Z-Paths] | **缺失**：未读取 cell.hyperlink，无链接命中或打开路由；选中文本仅用于复制。[Engine](../../../crates/editor-terminal/src/engine.rs)、[交互](../../../crates/editor-app/src/terminal/interaction.rs) | URL 通过已有外部打开边界；文件跳转复用编辑器入口。覆盖引号、括号、尾部标点、空格/中文路径、盘符/UNC、:line:column、Python traceback、软换行；打开前核对真实目标，按来源 cwd 解析相对路径。 |
| 历史输出的 cwd | 模型记录 cwd 变更对应网格位置，按被点击行选择目录；达到历史上限或重建输出时废弃无法证明的旧坐标。[Zed 模型][Z-Model] | **部分已有**：PowerShell prompt 发 cwd 元数据，Session 只保存当前 cwd；其他 shell 没有等价注入，历史行没有 cwd 关联。[shell](../../../crates/editor-terminal/src/shell.rs)、[IO](../../../crates/editor-app/src/terminal/io.rs) | 至少使用当前会话 cwd/工作区作为有限回退，并明确历史目录变更的限制；若维护行级 cwd，需要处理滚动淘汰、重排、恢复与新任务轮次。不要用当前 cwd 假称精确定位旧输出。 |
| 光标形状与闪烁 | 支持 block/bar/underline/hollow、设置与终端控制的闪烁，视图管理失焦、暂停闪烁和 IME。[设置][Z-Settings]、[视图][Z-View]、[渲染][Z-Element] | **差异/缺失**：核心默认 Beam、不闪烁；drawing 固定画 1.5px 竖线，忽略核心 cursor_style 与 Vi cursor。[Engine](../../../crates/editor-terminal/src/engine.rs) | 默认继续竖线且不闪烁；显式设置与 DECSCUSR 使用核心有效样式；覆盖 hide cursor、宽字符、失焦、滚动到历史、IME caret 和 reduce-motion。 |
| SGR、字体与装饰 | bold/italic/dim/inverse、underline/undercurl、strikeout；相同样式文字聚合；可配置字体、fallback、features、weight、line-height；最小对比度按设置调整且对部分精确色/装饰字符保留。[渲染][Z-Element]、[设置][Z-Settings] | **部分已有**：粗体、暗色、反色、隐藏、划线、RGB/256/动态颜色、宽字/组合符；Italic 没传绘图，所有 underline 变体都是相同实线。只有字体 family/size 设置。[Engine](../../../crates/editor-terminal/src/engine.rs)、[设置](../../../crates/editor-terminal/src/config.rs) | 通用绘图需要样式时与正在进行的 Canvas 契约工作协调；不加入终端专属插件 Paint。保持已有 palette 默认与主题 token。不要宣称 Zed 区分全部下划线变体，其当前 TextRun 主要区分 wavy。 |
| 键盘编码 | APP_CURSOR、Xterm modifiers、F1–F20、Ctrl caret、Shift+Enter 等；普通文本由平台输入路径交付。[映射][Z-Keys] | **部分已有**：APP_CURSOR、导航、F1–F12、Ctrl 字符、Alt、Shift+Tab；缺 F13–F20、部分 caret 别名；Shift+Enter 保留既有 CR。[input](../../../crates/editor-terminal/src/input.rs) | 增量覆盖明确编码；现有 Ctrl+C/V 优先本地复制粘贴，不照搬 Zed 的 Ctrl+Shift+C/V。NumPad、key-up、repeat 等受原生事件可见信息约束。 |
| Kitty 与其他高级键盘协议 | 核心包含 Kitty mode 位；固定 Zed 的 to_esc_str 和 Modes 映射没有完整 Kitty event-type/alternate-key/associated-text 上报。[核心模式][AZ-Term]、[宿主映射][Z-Keys]、[适配模式][Z-Alacritty] | **核心有、宿主未完整接入**，当前 Canvas Key 仅 key/ctrl/alt/shift，没有 key-up、repeat、物理键码。[Canvas](../../../crates/editor-app/src/ui/plugin/canvas.rs) | 不能把核心 mode 位当作完整协议支持；需要完整实现时应先扩展可复用的原生输入契约和测试，避免向程序承诺实际无法上报的事件。它不是固定 Zed 的确定功能差距。 |
| 鼠标报告与选择 | SGR/UTF8/legacy、按钮/修饰符、drag 与 motion 区分、重复单元格抑制；Shift 本地选择；单击抖动不立即产生选择。[鼠标映射][Z-Mouse]、[模型测试][Z-Model] | **部分已有**：SGR、legacy 223 限制、click/drag、Shift 绕过、双击语义/三击整行。缺 UTF8、ctrl/alt 编码；无按钮 motion 仍按按钮值处理。Pointer 当前只携 shift。[交互](../../../crates/editor-app/src/terminal/interaction.rs)、[Canvas](../../../crates/editor-app/src/ui/plugin/canvas.rs) | 单元格、历史/live screen、释放、无按钮移动、选择拖动与超界自动滚动均需行为回归。鼠标编号是左0/中1/右2，协议 legacy release=3；不得用 widget button 值直接冒充所有 motion 状态。 |
| 滚轮、历史与 alternate screen | 保留触控/像素滚轮累计量，按倍率换算；报告模式发鼠标；ALT_SCREEN+ALTERNATE_SCROLL 时发上下应用光标序列；Shift 覆盖；支持行/页/首尾导航。[模型滚动][Z-Model]、[鼠标映射][Z-Mouse]、[视图][Z-View] | **部分已有**：滚动历史、Shift+PgUp/PgDn、滚动条、输入回到底部；wheel 每事件固定 ±3、鼠标报告只发一份，未用 ALTERNATE_SCROLL。[交互](../../../crates/editor-app/src/terminal/interaction.rs) | 使用实际 delta 和累计余量，零 delta 不产生报告；TUI 与普通历史分开；保留恢复的 ConPTY 屏幕/历史坐标隔离和统一滚动条规则。 |
| 文本/路径/图像粘贴 | bracketed paste 净化 ESC；非 bracketed 换行转 CR；路径剪贴板/拖入路径引用后写入；图像剪贴板可转交 Ctrl+V 给原生 TUI。[模型粘贴][Z-Model]、[视图粘贴][Z-View] | **已有文本粘贴**，还会统一 CRLF/CR；未处理路径剪贴板、拖入文件或图像。[input](../../../crates/editor-terminal/src/input.rs)、[交互](../../../crates/editor-app/src/terminal/interaction.rs) | 保持当前文本换行和 Ctrl+V 语义；路径引用按所选 Shell 处理，不能把 Unix shlex 直接套 PowerShell/cmd。图像透传需独立显式行为，避免改变用户既有粘贴。 |
| 复制与选择设置 | copy_on_select、keep_selection_on_copy、平台 primary selection；Vi yank；复制可由系统 Edit 菜单触发。[设置][Z-Settings]、[模型][Z-Model]、[视图][Z-View] | **已有选择/复制/右键可用性**；无上述可选设置、Vi yank。Ctrl+C 固定复制，任务停止走独立入口。[交互](../../../crates/editor-app/src/terminal/interaction.rs) | 默认关闭自动复制并保留选择，以兼容；空白无效选择不覆盖剪贴板，任务中断仍使用现行明确动作。 |
| 焦点、IME 与 Unicode | 视图平台输入处理、预编辑、caret、FOCUS_IN_OUT；零宽字符并入文本 run；在 read-only 下允许 Vi 而禁止输入。[视图][Z-View]、[渲染][Z-Element] | **已有**：Canvas 的 UTF-16 组合范围、预编辑、取消/提交与光标；core wide/spacer/zerowidth；FOCUS_IN_OUT 已发送。[Canvas](../../../crates/editor-app/src/ui/plugin/canvas.rs)、[历史验收](../verification/builtin-terminal-04-2026-10-09.md) | 本轮改变绘图/焦点后重做中文候选/提交/取消、emoji/组合符、窗口缩放与 TUI 输入；历史通过不能代替新产物验收。 |
| Tab、多个会话与分屏 | PaneGroup 内多 Pane，各 Pane 的终端 Item 可 clone_on_split；中心编辑区也可放终端；持久化 Pane 结构。[面板][Z-Panel]、[分屏][Z-View]、[持久化][Z-Persistence] | **已有多 Tab**、命名、重排、左右侧栏及可调宽；只有一个 active 和一个 canvas，缺面板内同时显示多个终端。[模块](../../../crates/editor-app/src/terminal/mod.rs)、[视图](../../../crates/editor-app/src/terminal/view.rs) | 本轮分屏限定终端内部，保留文件 Tab 只放文档与既有单面板政策；每个 Pane 必须独立测量、焦点、PTY resize、搜索状态、菜单归属及关闭，不可多个 canvas 共用全局 width/height。 |
| Shell、环境与新会话目录 | shell/argv、cwd 策略、env、工具环境；Python venv/Conda 等由 Zed project/toolchain 子系统发现并激活。[设置][Z-Settings]、[目录选择][Z-View] | **已有**自定义 shell profiles/argv、PowerShell/cmd/pwsh/WSL 默认；新 Shell 默认工作区，NativeLaunch env 为空；run 配置另有受控环境与工具准备。[模块](../../../crates/editor-app/src/terminal/mod.rs)、[运行](../../../crates/editor-app/src/run.rs) | 通用用户 env、cwd 策略可复用结构化 NativeLaunch；Python/Conda 检测策略继续留插件，接入公开语言上下文/执行准备机制，不能增加宿主语言白名单或扫描后偷偷激活。 |
| 任务、构建、调试与编辑器 | Zed task 负责 spawn/reuse/reveal，终端实现运行与重跑；路径打开依赖 project/workspace/editor；AI 与远程终端另依赖其他子系统。[面板][Z-Panel]、[视图目标][Z-Paths] | **已有且不同**：普通 Shell 独立；同轮准备和程序同 Tab、重新运行复用、活动任务关闭确认、debug 目标 PTY 与检查区、公开执行/PTY 资源边界。[tasks](../../../crates/editor-app/src/terminal/tasks.rs)、[task view](../../../crates/editor-app/src/terminal/task_view.rs)、[运行规格](builtin-terminal-run-debug.md) | 增强视图不得改变配置/轮次/Tab/暂停代次身份，不复制 Zed 的任务业务模型；搜索/分屏/切换仍隔离输出输入。语言发现依赖继续由插件提供。 |
| Title、Bell 与应用事件 | 处理终端 Title/ResetTitle，用户重命名优先；Bell 可提示 Tab；焦点后清除；Zed 的 display-only 输出禁止 OSC52。[模型][Z-Model]、[设置][Z-Settings]、[视图][Z-View] | **缺失部分事件**：IO 处理 PTY reply、颜色/尺寸查询、Shell OSC52 ClipboardStore，Task 禁止；Title、Bell 等被通配分支忽略。[IO](../../../crates/editor-app/src/terminal/io.rs) | 可增加兼容的自动标题/铃声状态设置，不覆盖用户明确名称；遵守任务无剪贴板权限、程序输出不自动加载剪贴板等既有边界。 |
| 绘制性能与字符几何 | 文本 run 按相同样式聚合、相邻背景区域合并；部分 block/box 图形直接绘制；避免字形间缝隙；本轮检索未见其宿主直接调用 damage/reset_damage。[渲染及测试][Z-Element] | **逐 cell 绘图**：逐次构造 Paint::Text/Fill，重建后做整个 Canvas 比较；未接核心 damage。[Engine](../../../crates/editor-terminal/src/engine.rs)、[视图](../../../crates/editor-app/src/terminal/view.rs) | 可按行或样式批量缓存，但正确处理主题/字体/selection/scroll/resize 等非输出失效；damage 不是 Zed 已验证方案。测量繁忙输出、长行滚动与分屏 resize 后再报告改善。 |
| 历史恢复与安全 | Zed 普通终端持久化 cwd/custom title/Pane 结构，Task 的 serialize 返回 None；不等于恢复进程内存或完整输出。[视图持久化][Z-View]、[面板持久化][Z-Persistence] | **已有且需保留**：有界逻辑网格快照、8MiB、迁移备份/重复安全、保留任务输出但不重跑、受限工作区不启动 Shell。[持久化](../../../crates/editor-app/src/terminal/persistence.rs)、[升级](../../../crates/editor-app/src/terminal/upgrade.rs) | 不能用 Zed 的 cwd-only 恢复替换 Nanobug 网格历史；新配置/分屏缺省值须读回旧版本快照，损坏数据不覆盖用户文件，任务关闭不改变确认规则。 |

## 4. 可直接复用的 Alacritty 0.26.0 API

下列接口已存在于 Nanobug 的发布基线，不需要为了接入它们另改核心算法：

| 用途 | 现有公开入口 | 集成注意事项 |
| --- | --- | --- |
| 全历史匹配 | `RegexSearch::new`、`RegexIter::new`、`Match = RangeInclusive<Point>` | 从 grid topmost_line/Column(0) 到 bottommost_line/last_column；核心迭代保留软换行和宽字符语义。默认 smart-case；明确大小写可用 regex inline flags。空 regex、零宽匹配和无效 regex 需处理。 |
| 前后定位 | `Term::search_next(regex, origin, direction, side, max_lines)`、`Term::scroll_to_point` | origin 与结果均是有历史负行号的终端坐标，和 viewport row 不同；控制搜索预算，不在 UI 线程无界扫描每次完整历史。 |
| Vi | `toggle_vi_mode`、`vi_motion`、`vi_goto_point`、`vi_mode_cursor`、`ViModeCursor::scroll`、`Selection` | 原生文本事件和 KeyDown 都需遵守 Vi 只导航；滚动时同步 Vi cursor 与选择，退出不要把导航字符泄漏至 PTY。 |
| 光标 | `cursor_style`、`renderable_content().cursor`、`point_to_viewport` | renderable cursor 会反映 Vi 和有效形状；默认 Beam 可继续保留。 |
| 重绘 | `Term::damage`、`TermDamage::Full/Partial`、`reset_damage` | 修改 grid_mut、恢复/reflow、选择、主题和视图布局的失效仍由宿主负责；在正确完成绘制消费之后才重置。 |

来源：[Term][A-Term]、[search.rs][A-Search]、[vi_mode.rs][A-Vi]；Zed 对应适配见 [alacritty.rs][Z-Alacritty]。这张表是接口可用性核对，不是性能测试结果。

## 5. 自行维护核心的来源与维护要求

以既有 `0.26.0` 行为为兼容基线，将坐标、样式单元格、网格与重排、屏幕转义序列、选择、搜索和 Vi 算法融入 `editor-terminal` 的私有模块；宿主通过 `Engine`、快照和副作用值使用终端。字节解码保留精简的 `vte` 依赖。上游 PTY、平台进程、事件循环、线程与同步实现，以及独立包入口、Cargo 清单、特性矩阵和发布资料均不引入。[来源与许可](../../../crates/editor-terminal/NOTICE)记录固定版本/commit、原始包摘要和融入范围；算法回归与录制回放直接由 `cargo test -p editor-terminal` 执行。

Zed 的两个宿主 crate（`terminal`、`terminal_view`）均声明 `GPL-3.0-or-later`；固定 Alacritty fork 的核心包声明 `Apache-2.0`，并在仓库保留 Apache 许可证。Nanobug 的终端增强以行为对照和自主实现为方式，核心源码保留其原始许可与来源；不把宿主 crate 的许可误标成核心许可。[Zed terminal Cargo][Z-TermCargo]、[Zed terminal_view Cargo][Z-ViewCargo]、[Alacritty 核心 Cargo][AZ-Cargo]、[Apache 许可证][AZ-License]

后续同步以来源记录中的固定 commit/发布包为比较基点，读取所需差异并选择性融入本地模块，不拉取完整上游仓库、分支或另存整包副本。候选改动使用独立输出目录验证 parser/grid/search/selection 及 Windows PTY 交互，不只检查 Cargo 编译。Zed 的 signal-mask 补丁只在有对应 Unix 启动需求与测试时选择性引入，不能为“和 Zed 一样”盲目叠加无调用方的接口。参考更新不授权升级 GPUI 或其他无关依赖。

## 6. 建议验证场景

以下是本轮应补的证据，未运行项必须保持“未验证”：

1. 从普通终端按钮打开真实 Shell，输入含中文、组合符、emoji 和多行输出；搜索跨软换行/历史的结果，前后循环定位，不产生 Shell 输入，输出继续到达时匹配不串位。
2. 切换 Vi，移动、字符/行/块选择、复制、历史滚动、退出再输入；多 Tab/分屏间光标和选择不共享，结束任务输出仍可导航。
3. 实际 Ctrl+Click 打开 URL、OSC8 和存在的 Windows 文件/行列；鼠标报告模式下按明确手势打开，普通点击仍交 TUI；错误路径、不允许 scheme、拖动取消不执行。
4. 实际运行 TUI，检查 APP_CURSOR、SGR/UTF8/legacy、点击/释放/drag/无按钮 motion、修饰符、Shift 本地选择、滚轮累计与 alternate scroll；与裸字节回归结果分别记录。
5. 保持已有 Ctrl+C/V 与右键菜单、文本换行和 bracketed paste；新复制设置以兼容默认读回旧 settings；程序输出不能借新 UI 获得额外剪贴板/进程权限。
6. 深浅主题、用户 palette、DECSCUSR、光标隐藏/失焦、中文 IME caret、字号和 Windows DPI 缩放；核对斜体/装饰与列宽，不把核心 flag 存在当视觉已支持。
7. 分屏内每个真实 PTY 独立 resize、焦点、输入/输出和菜单；改面板总尺寸、拖 divider、关闭/取消活动任务、隐藏后继续输出，重启读回布局和历史，不重跑旧任务。
8. 重跑现有终端 restore、任务生命周期、旧数据升级和真实 Neovim Unicode/resize 测试；完整 workspace 门禁和本轮原生验收分别记录。已有 ignored 测试只在准备当前工具/夹具后显式执行，不把跳过计为通过。

[历史终验](../verification/builtin-terminal-04-2026-10-09.md)提供已有测试命令、环境和限制；本轮结果必须另写验证记录。本文件只完成来源固定、能力归属和差异识别，未执行 Rust 构建，也不承诺固定 Zed 文档之外的全部终端图形/键盘协议或其他平台行为。

[Z-Commit]: https://github.com/zed-industries/zed/commit/d93bf740852672fb299fcbdba3af1d5fd0253e92
[Z-Cargo]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/Cargo.toml#L535
[Z-Lock]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/Cargo.lock#L611-L633
[Z-Docs]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/docs/src/terminal.md
[Z-TermCargo]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal/Cargo.toml#L1-L6
[Z-ViewCargo]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal_view/Cargo.toml#L1-L6
[Z-Model]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal/src/terminal.rs
[Z-Alacritty]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal/src/alacritty.rs
[Z-Vi]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal/src/terminal.rs#L2356-L2475
[Z-Search]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal_view/src/terminal_view.rs#L2103-L2258
[Z-View]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal_view/src/terminal_view.rs
[Z-Element]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal_view/src/terminal_element.rs
[Z-Panel]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal_view/src/terminal_panel.rs
[Z-Persistence]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal_view/src/persistence.rs
[Z-Paths]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal_view/src/terminal_path_like_target.rs
[Z-Links]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal/src/alacritty/hyperlinks.rs
[Z-Keys]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal/src/mappings/keys.rs
[Z-Mouse]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/terminal/src/mappings/mouse.rs
[Z-Settings]: https://github.com/zed-industries/zed/blob/d93bf740852672fb299fcbdba3af1d5fd0253e92/crates/settings_content/src/terminal.rs
[A-Release]: https://github.com/alacritty/alacritty/commit/94e7c8874e526b1e67b349d9ba30ddf81669119e
[A-Term]: https://github.com/alacritty/alacritty/blob/94e7c8874e526b1e67b349d9ba30ddf81669119e/alacritty_terminal/src/term/mod.rs
[A-Search]: https://github.com/alacritty/alacritty/blob/94e7c8874e526b1e67b349d9ba30ddf81669119e/alacritty_terminal/src/term/search.rs
[A-Vi]: https://github.com/alacritty/alacritty/blob/94e7c8874e526b1e67b349d9ba30ddf81669119e/alacritty_terminal/src/vi_mode.rs
[AZ-Cargo]: https://github.com/zed-industries/alacritty/blob/4c129667ce56611becdc82de6e28218c80e2e88f/alacritty_terminal/Cargo.toml
[AZ-License]: https://github.com/zed-industries/alacritty/blob/4c129667ce56611becdc82de6e28218c80e2e88f/LICENSE-APACHE
[AZ-Term]: https://github.com/zed-industries/alacritty/blob/4c129667ce56611becdc82de6e28218c80e2e88f/alacritty_terminal/src/term/mod.rs
[AZ-Search]: https://github.com/zed-industries/alacritty/blob/4c129667ce56611becdc82de6e28218c80e2e88f/alacritty_terminal/src/term/search.rs
[AZ-Vi]: https://github.com/zed-industries/alacritty/blob/4c129667ce56611becdc82de6e28218c80e2e88f/alacritty_terminal/src/vi_mode.rs
[AZ-Unix]: https://github.com/zed-industries/alacritty/blob/4c129667ce56611becdc82de6e28218c80e2e88f/alacritty_terminal/src/tty/unix.rs
[A-Compare]: https://github.com/zed-industries/alacritty/compare/94e7c8874e526b1e67b349d9ba30ddf81669119e...4c129667ce56611becdc82de6e28218c80e2e88f


实施期间用户追加确认：终端移除对 `CanvasView` 的依赖，独立库采用 `editor-terminal`，通用原生文字网格进入 `editor-ui`。第 3 节差异表仍保留实施前状态；移动后的源码链接已更新，实际交付状态以[验证记录](../verification/terminal-enhancements-2026-10-24.md)为准。
