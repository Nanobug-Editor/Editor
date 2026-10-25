# 独立原生终端与核心代码融入验证

日期：2026-10-24 至 25 日（Asia/Shanghai）。状态：本轮代码融入、构建门禁及下述 Windows 交互验收已完成；关闭时抓取无障碍树的 AccessKit 错误单独保留为限制。未提交、推送或发布。

依据：[固定 Zed 参考与实施前差异](../specs/terminal-zed-reference-2026-10-24.md)、[原内置终端规格](../specs/builtin-terminal-run-debug.md)。本轮用户进一步明确：终端不再依赖 `CanvasView`；通用 UI 放入 `editor-ui`，终端实现独立成库；Alacritty 的实际实现代码融入该库，不保留整包 `vendor` 副本或独立上游终端依赖。

## 实现与边界

| 位置 | 本轮职责 |
| --- | --- |
| [editor-terminal](../../../crates/editor-terminal/src/lib.rs) | 唯一屏幕与历史状态、恢复、原生绘图、搜索与 Vi、输入编码、Shell 元数据和参数引用、设置、会话模型及终端专用搜索栏 |
| [editor-ui/text_grid](../../../crates/editor-ui/src/text_grid/mod.rs) | 通用原生文字网格、样式文字与矩形、焦点、UTF-16 IME、指针捕获和统一滚动条；不解释终端协议或管理进程 |
| [editor-app/terminal](../../../crates/editor-app/src/terminal/mod.rs) | 工作区/信任、受控进程、Run/Debug、停靠及侧栏组合、文件导航、旧数据迁移和持久化适配 |
| [核心算法与来源](../../../crates/editor-terminal/NOTICE) | 选用的 Apache-2.0 屏幕算法融入库内私有 screen/grid/coordinates/selection/vi 模块，保留来源与许可；不引入上游 PTY、线程、事件循环及包清单 |

终端实现没有 `CanvasView`、插件画布事件、画布版本协商或序列化 Paint 依赖。`TextGrid` 直接使用 GPUI 原语及 Base 的公开滚动条行为；终端专用搜索 UI 留在 `editor-terminal`。宿主仍通过原有公开执行/PTY 能力与任务协作，没有增加终端专属插件协议。

`alacritty_terminal` 已从产品依赖和锁文件移除，原 `vendor` 目录已删除。宿主只使用 `editor-terminal::Engine` 和该库公开的快照、模式与副作用数据，不能直接操作其私有屏幕存储。GPUI Base 同时恢复锁定的发布依赖，没有本地路径覆盖。

共享工作区本轮开始前已有大量改动，且另一个会话继续修改插件 Canvas。那些改动不计入本轮成果；本轮曾临时添加的 Canvas 字体/原生输入适配已撤回，只保留原生文字网格接缝。

## 功能与兼容情况

| 项目 | 本轮结果 |
| --- | --- |
| 搜索 | 终端专用搜索栏；字面量/regex、显式大小写、前后及环绕定位；核心处理中文宽字符和软换行；非法表达式可见；不创建第二份文本缓冲区 |
| Vi | 显式按钮或 Ctrl+Shift+Space；h/j/k/l、词/段/括号、首尾和屏幕导航、页/半页、v/V、Alt+V 矩形选择、y、Esc、i、/、n/N；导航字符不发给子进程 |
| 链接与源码 | OSC8 优先，打印 URL/路径及 `:line[:column]`、`(line,column)`；Ctrl-click、右键打开及 Vi Enter；只允许安全 HTTP(S) 或真实本地文件，按会话 cwd/工作区解析 |
| 样式与光标 | 原生斜体/粗体、独立装饰颜色、双线/波浪/点线/虚线、隐藏和划线、DECSCUSR 与 Vi 光标；块/竖线/下划线/失焦空心、闪烁与 IME anchor |
| 键鼠 | F13–F20 和 Ctrl 数字别名；SGR/UTF8/legacy、完整 Ctrl/Alt 修饰符、无按钮 motion、单元格去重、拖动选择与边界滚动；Shift 保留本地选择覆盖 |
| 滚动与 TUI | 按实际 delta/每会话余量累计；零 delta 不产生输入；ALT_SCREEN+ALTERNATE_SCROLL 发应用光标；历史坐标不会报告到 ConPTY 实时屏幕 |
| 粘贴 | 保留文本换行及 bracketed paste/ESC 净化；文件剪贴板/系统文件拖放按 Shell 引用，不追加 Enter；cmd 展开不安全的路径明确拒绝；图像 Ctrl+V 转交为显式选项 |
| 多会话与分屏 | 终端面板内同轴平铺，可左右/上下扩展并使用现有 Base 调整分隔；各叶独立测量/IME/焦点/PTY resize；折叠只改变展示，不停止其他会话；保存身份与方向 |
| Shell 设置 | 新增结构化 env、可选继承当前 cwd、滚动倍率、光标/闪烁、行高、可选标题与视觉 Bell、自动复制/复制后保留选择等；语言环境发现仍由插件负责 |
| 同步输出 | VTE synchronized update 遗漏结束序列时按核心超时释放；EOF 也发布最后一帧，避免 TUI 内容永久留在缓冲中 |
| 原有生命周期 | 保留 Tab 名称/顺序/侧栏、Ctrl+C/V、受限工作区、任务轮次、调试目标及输入、关闭确认、隐藏继续运行、快照限额和旧数据迁移 |

旧配置使用 serde 默认值继续读取。默认保留稳态竖线、1.45 行高、工作区 cwd、文本 Ctrl+V、复制后保留选择；新增 env 为空，不自动激活语言环境。`shift_enter_newline=false` 保留原 CR，显式开启后才采用 LF；`forward_image_paste=false` 保留原图像粘贴行为。历史快照仍为 version 1，只增加可选 `panes`；旧文件默认单视图，损坏叶身份会拒绝恢复，不覆盖原文件。

## 来源、许可与构建

- Zed 固定 `d93bf740852672fb299fcbdba3af1d5fd0253e92`，其 Alacritty 固定 `4c129667ce56611becdc82de6e28218c80e2e88f`。Zed 的 GPL-3.0-or-later 宿主终端源码只作行为参考，没有复制入本项目。
- 融入算法的行为基线为 0.26.0，对应上游 `94e7c8874e526b1e67b349d9ba30ddf81669119e`；原始归档 SHA-256 为 `bda177466b9524d59f1b12f0dd30b68696788e9992a7e959021c4a0ed96fcf59`。许可证及来源保留在库内，原始包清单与发布资料不引入。快照字段、匿名超链接标识和 DA2 响应保持兼容。
- 算法单测及 45 组录制字节流回放已融入本库测试，当前入口为 `cargo test -p editor-terminal`，使用主工程 `target`；不存在独立上游包测试入口。依赖保持原锁定版本，移除不再需要的上游 PTY 依赖。
- Windows 已安装 Rust/MSVC/SDK；测试只在子进程提供 `RUST_MIN_STACK=16777216`、现有 MSVC/SDK `LIB` 和 `/STACK:16777216`，没有安装或修改全局工具链。

## 实际检查

以下记录核心融入与原生焦点修复后的实际结果。默认忽略项不计为通过；显式原生测试单独列出。命令统一在本 checkout 的主 `target` 执行。

| 入口 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo test --workspace --exclude editor-app --locked --offline --jobs 2 --no-fail-fast` | 113 组，766 passed、0 failed、409 ignored；包含新终端库及通用网格 | `target/terminal-integrated-workspace-tests-retry.log`、`target/terminal-integrated-workspace-totals.json` |
| 其中 `editor-terminal` | 199 passed、0 failed、1 ignored；45 组录制回放、同进程匿名 OSC8、DA2、Engine/输入/配置回归；忽略的是下节原生焦点测试 | 同上；加入原生测试前的独立结果另见 `target/terminal-integrated-core-tests-final.log` |
| 其中 `editor-ui text_grid` | 2 passed、0 failed；中文/emoji preedit 提交/取消、UTF-16、指针修饰符及 AltGr 文本路径 | 同上；独立结果 `target/terminal-text-grid-tests-final.log` |
| `cargo test -p editor-app terminal:: --locked --offline --jobs 2 -- --test-threads=1` | 37 passed、0 failed、4 ignored，40.89 秒；含最终文件名行列解析修复、真实 Shell 搜索/Vi/双视图隔离、resize、恢复及原有任务/迁移 | `target/terminal-navigation-final-app-tests.log` |
| `cargo check --workspace --locked --offline --jobs 2` | 文件导航修复后再次通过，3.87 秒 | `target/terminal-navigation-final-workspace-check.log` |
| `cargo fmt --check` | 最终通过；中途一次仅发现并行 Canvas `scene/svg.rs` 格式差异，没有替其他会话格式化 | `target/terminal-integrated-delivery-format.log`；中途结果 `target/terminal-navigation-final-format.log` |
| `cargo build -p editor-app --locked --offline --jobs 2` | 通过，35.18 秒；生成包含文件导航修复的原生复验程序 | `target/terminal-navigation-final-build.log` |
| 显式 Windows cached 搜索栏焦点回归 | 1 passed、0 failed，1.04 秒；真实 AccessKit 焦点名称和值 | `target/terminal-cached-focus-green.log`、`target/terminal-cached-focus-green.json` |
| 显式 Windows 宿主搜索焦点（浅色、深色各一次） | 各 1 passed、0 failed；两张读回 PNG 均已查看 | `target/terminal-native-light-final.log`、`target/terminal-native-dark-final.log` |
| 显式真实 Neovim Unicode/resize | 1 passed、0 failed，2.05 秒；使用已安装 `nvim.exe` | `target/terminal-neovim-final.log` |
| 本轮范围 `git diff --check`、文档相对链接 | 通过；326 个链接均存在 | `target/terminal-final-doc-links.json` |

录制回放仍比较原始预期网格。其匿名 OSC8 标识采用保持双射的规范化，允许同进程其他终端先消耗全局编号，同时检查链接分组、URI、样式和单元格内容；不修改生产编号。历史回放的旧环形索引元数据只用于原有网格等价比较，不作为新建屏幕索引。

早期全 workspace 尝试遇到 Windows 文件占用（OS error 32），该次 `context_selection` 没有执行，不计为通过；完整重跑结果见表中 retry 日志。宿主初次定向测试曾有一次 Shell 超时；该项单独重跑及完整终端测试重跑均通过（`target/terminal-shell-focused-rerun.log`、`target/terminal-integrated-app-tests-rerun.log`）。并行 Canvas 改动还曾在写入中导致字段不匹配的编译失败，重试同一入口后通过，没有为其修改协议。没有格式化其他会话仍在修改的 Canvas 文件。未为无关 ignored WASM 测试准备夹具，也未将其计入本轮通过数。

依赖审计使用 `cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc --locked --offline`，核对 `workspace_root`、`target_directory`、实际依赖来源及终端清单。`target/terminal-integrated-dependency-audit.json` 记录：无 `alacritty_terminal`、`gpui-component` 或其宏库；GPUI Base 来自 crates.io，`vendor` 不存在。

## Windows 原生焦点回归

实际点击搜索按钮触发过 `set_focus called more than once in a single frame`。单独输入框，以及从正文直接打开搜索框的原生测试均通过，不能覆盖标题按钮已持有焦点的路径。最小复现保留一个已聚焦标题按钮、一个与停靠布局相同的 cached 正文及实际 `SearchBar`；正文延后绘制时立即修改焦点，会让同帧标题与输入框都登记为焦点。

正式回归位于 [search_view/tests.rs](../../../crates/editor-terminal/src/search_view/tests.rs)。同一命令连续两次复现原始焦点错误（`target/terminal-cached-focus-red-3.log`、`target/terminal-cached-focus-red-repeat.log`），将焦点请求延到当前绘制周期结束后转为通过：**1 passed、0 failed**，实际运行 1.04 秒；`target/terminal-cached-focus-green.json` 同时验证了真实 AccessKit 焦点节点的输入名称与值。

```powershell
$env:NANOBUG_TERMINAL_FIND_FOCUS_OUTPUT = Join-Path $PWD 'target/terminal-cached-focus.json'
cargo test -p editor-terminal search_view::tests::native_cached_find_preserves_one_accessible_focus_owner -- --ignored --exact --test-threads=1 --nocapture
```

[搜索栏](../../../crates/editor-terminal/src/search_view.rs)和[宿主终端焦点调度](../../../crates/editor-app/src/terminal/view.rs)使用 GPUI 的公开 `defer_in`，并校验会话身份；宿主还检查面板可见性。没有修改 GPUI/Kit 或通用输入框来绕过无障碍约束。该原生测试默认 ignored，只有上述显式运行才计入结果。

实际点击历史中的 `native-terminal.txt:2:1` 时还发现 URL 解析器把文件名误识别为 scheme，导致菜单打开没有导航。先在 [navigation.rs](../../../crates/editor-app/src/terminal/navigation.rs) 增加可观察目标回归，得到 **0 passed、1 failed**（`target/terminal-navigation-filename-red.log`）；修复后带数字行列后缀的候选继续接受原有本地文件存在性校验，明确的 HTTP(S)/file URL 仍先按 URL 规则校验。完整宿主终端测试已重新通过，覆盖含/不含列号两种裸文件名。

## Windows 实际交互验收

使用独立临时工作区与 `ME_EDITOR_PROFILE_HOME`，通过普通 version-1 状态恢复 80 行含中文、emoji、斜体及本地文件位置的历史；不修改用户资料。实际程序从当前 `target/debug/editor-app.exe` 复制到测试目录运行，避免占用其他会话的构建输出。焦点修复产物 SHA-256 为 `57F58DEB4854E9365306F1B6B5DFC381B2893221D8ACEED42D0045D6F6B2BD93`；身份和来源记录于 `target/terminal-final-native-artifact.json`。

- 实际点击工具栏搜索按钮，输入中文、前后定位并观察匹配高亮；Esc 关闭后回到网格。连续切换 Vi、打开终端菜单及创建左右分屏均未复现焦点崩溃。
- 搜索框使用系统拼音键入 `n`、`i`，候选窗口在输入框附近，空格提交为“你”；再次预编辑后 Esc 取消，保留已提交内容。网格也实际显示行内预编辑及系统候选，Esc 取消；再次提交“你”后普通 PowerShell 回显。没有通过 UI 执行 Shell 命令。
- 左右两个会话实际显示各自的 PowerShell；滚动左侧历史只改变左侧，右侧提示符和活动边框保持独立。测试后按确切可执行路径核对身份，结束本轮拥有的程序及其子进程。
- 最终文件导航产物 SHA-256 为 `30E6C010CA0A7BEA072C023F812A63C9DDD77D8FB169EEE062855B94C5E46265`（`target/terminal-navigation-native-artifact.json`）。实际右键打开 `native-terminal.txt:2:1` 后出现文件 Tab，光标及状态栏均位于第 2 行、第 1 列；未启动浏览器或通过 Shell 打开文件。
- 浅色、深色原生截图分别为 `target/terminal-native-light-final.png`、`target/terminal-native-dark-final.png`，均已读回检查搜索框、中文/emoji、斜体、光标和滚动条；当前 Windows 比例为 150%，逻辑 1200×800 对应 1800×1200 PNG。
- 当前代码的真实 Neovim 测试验证 Unicode 与窗口尺寸变化，不把它称为字号缩放或其他 DPI 验收。

## 保留限制

- 当前分屏为终端内部同轴平铺，未引入 Zed 的任意嵌套 PaneGroup、中心文件 Tab 终端或通用自由停靠；重启保留叶和方向，分隔比例仍由当前 Base 控件状态处理。
- 相对文件按当前会话 cwd 和工作区解析；没有给全部历史行承诺精确 cwd 追踪。当前自动目录元数据主要来自已有 PowerShell 集成。
- 语言虚拟环境发现、远程会话、AI/REPL 等依赖其他子系统的策略没有放进终端或宿主语言分支。
- 不声称完整 Kitty 键盘、所有高级图形协议或所有字体 feature 配置；固定 Zed 宿主本身也没有完整 Kitty 事件上报。保留当前能力协商，不向子进程虚报不支持的协议。
- 没有宣称绘图性能提升或接入 damage。当前仍按可见网格生成原生绘图；超长历史的 regex 搜索仍可能在 UI 线程花费较长时间。
- 原生验证仅覆盖当前 Windows、150% 比例及已安装字体/输入法；未穷举其他 DPI、字体、Shell 或平台。原生界面验收不等于所有鼠标协议组合或其他平台均已实测。
- 文件导航复验结束时发送 Alt+F4，并立即抓取同一窗口的无障碍树，`accesskit_windows 0.34.0` 的 `ClientToScreen(...).unwrap()` 报无效窗口句柄 `0x80070578`，测试进程退出。记录在隔离目录的 `navigation-visible-stderr.log`；未确认是否仅由关闭与抓取的时序触发，不将这次关闭报告为正常通过，也未为此复制或修改上游依赖。
