# 工单 04：真实 Windows 桌面验收

本记录使用原生 Nanobug 窗口与真实键鼠输入；GPUI 自动化、运行时测试和 SDK 构建结果见[主验收记录](04-diagnostics-and-language.md)。不将截图中的预览误记为已经应用，不将跳过的测试记为通过。

## 输入与隔离范围

- 第一轮生产源码：`cc17293e2b3611f2e639472447d0c3606f71f592`；文档提交：`d3f04590699138705b0982dbe412fedd67f2f5a2`。
- 第一轮宿主 SHA-256：`51800CA225B3ADF6E9AF7F26A1698ACB48EE5331A839594C6D3A9DBDDCBFAD6E`。
- 最终 SDK 内容摘要：`3f39ebeff8b29cea5577d7c6407c922c0f0edbc664f95686dfc3ebad514331d6`；49 个源码与协议文件、0 个 Markdown，49/49 与导出源码一致。四个正式语言/检查器包与两个事务包的精确版本、包及 WASM 摘要见主记录。
- 隔离目录：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-language/native-qa-04/`。安装帮助程序只调用公开 `Package`、`Manager` 和实际授权接口；启动使用公开插件开发入口。未修改用户原工作区或用户日常配置。
- 使用 `computer-use` 的 Sky API 选择真实返回的应用与窗口，逐次观察后只执行一次输入。第一轮控制器 session `51511`，窗口 `658328`；设置窗口只捕获截图，避免此前已复现的上游 AccessKit 设置 UIA 读取故障。

## 第一轮实际观察

| 入口 | 实际输入与可见结果 |
| --- | --- |
| 中文 IME | 在表达式文档输入真实字母 `n`，出现微软拼音候选“你、呢、那……”及组合文本；Escape 取消后恢复原文。不是粘贴中文模拟 IME。 |
| 两个检查器 | 通过面板公开检查命令运行 spell-check 与 config-check；问题列表同时显示两项，来源、严重程度、`3:9` / `4:1` 坐标及各自相关位置和 Fix 可辨。 |
| 拼写修复 | 预览显示 `teh` → `the`；取消后原文与磁盘不变；再次预览并应用后文档显示 `the` 且未保存；点击该操作的“撤销整组操作”恢复 `teh`。 |
| 创建配置 | config-check 的公开 Fix 进入通用预览，显示创建 `native-generated.ini`，10 字节。取消后目标不存在；再次预览并应用后文件树出现新文件，实际内容 `port=8080\n`，配置诊断归零。 |
| Direct 引用 | 在中文符号上按 Shift+F12，显示表达式来源的三项 `review.nbexpr:1:5`、`:2:1`、`:2:6`；Down 实际切换焦点和蓝色选中项，Return 定位回文档 `2:1`。 |
| Direct 动作 | 原生编辑右键菜单“显示代码操作”返回 `Use sum / 使用 sum`；共用预览显示 `add` → `sum`；应用后真实文本改变，Ctrl+Z 经原生文本撤销栈恢复 `add`。 |
| Direct 参数 | `add(中文, 2)` 第二参数处按 Ctrl+Shift+Space，显示 `sum(left, right)`、当前参数 `right` 及说明；Escape 关闭。 |
| 标准 LSP 引用 | 打开 `.nbkv`，实际受管原生服务返回三项 `2:1`、`3:7`、`4:9`；Down 与 Return 定位回中文引用 `3:7`。 |
| 标准 LSP 动作 | 原生编辑菜单返回 `Use default port / 使用默认端口`；共用预览显示 `oops` → `8080`，取消后 `port=oops` 保持不变。 |
| 标准 LSP 参数 | `setting(中文,` 后按 Ctrl+Shift+Space，显示 `setting(key, value)`、当前参数 `value` 和说明；Escape 关闭。 |
| 外观与缩放 | 浅色/深色主题真实切换，编辑字号从 14px 经 15px 到 16px，文档、菜单、参数提示和原生面板随本地主题与字号渲染。窗口最大化、还原后仍能输入与导航。英文资源与其他比例由主记录中的真实 GPUI 交互用例覆盖。 |

第一轮没有通过 UI 删除新文件；跨文件整组 Undo 由主记录中的实际 GPUI 事务用例验证。第二个长标题事务通知的按钮在底部裁剪，未将该次点击记为 Direct 整组 Undo 通过；第一项拼写修复的原生整组 Undo 和 Direct 原生 Ctrl+Z 是各自独立的实际结果。

## 发现与复验过程

1. 初次打开问题列表时，Down 没有切换选中来源；指针选择可工作。已在正式双检查器 GPUI 用例建立真实 Red：`0 passed / 1 failed`，失败断言为初始焦点下 Down 必须选中另一来源与其 Fix。修复焦点接缝后的同一用例 Green：`1 passed / 0 failed / 0 ignored`，包括 Down、Up、Home、End、完整 Enter 定位以及原有事务断言。第二轮新宿主物理 Down、Enter 与 Escape 复验已通过，具体见下节。
2. Ctrl+. 在编辑区没有显示代码动作，同一提供者的原生菜单正常。已建立原生反馈断言并重复观察失败；快捷键目录能够捕获并显示 Ctrl+.，但该目录不是编辑器文本焦点，不能单凭此排除 IME。随后实际字母输入仍出现微软拼音候选；Escape、单 Shift 切换后，普通 `n` 直接提交、无 IME 候选，并由 Ctrl+Z 恢复原文，编辑区 Ctrl+. 仍未显示动作。第二轮排除了先前弹窗状态；第四轮进一步检查了真实事件入口，见后文。

微软说明 Shift 可切换中英文模式、Ctrl+. 在中文模式切换标点；此处使用真实字母输入验证模式，未修改系统输入法设置。[Microsoft Simplified Chinese IME](https://support.microsoft.com/en-us/windows/hardware/input-devices/microsoft-simplified-chinese-ime)

## 第一轮退出与文件核对

控制器通过公开 `stop` 正常退出，exit 0；Sky 返回该验收应用窗口 0，目录内验收程序及受管服务进程 0。两个原文件的磁盘 SHA-256 均与启动前相同：

- `review.nbexpr`：`57296520794A00FFD4D8F9EBBD616D324C32741B0228C9C9C5A64F2572F1874E`。
- `review.nbkv`：`F36BE9141DA31B040D88D9C30019CBEEA7B980DBCD8A5EDDA694AF0DC498BCD4`。

仅本轮明确应用的隔离文件 `native-generated.ini` 新增；未保存的原文改动未落盘。该阶段尚不能关闭工单，后续复验结果见下文。

## 第二轮：问题列表修复的物理复验

生产候选为 `fde5a430a5a8ff6f850e567d2206816a8c1d15b2`；宿主 SHA-256 为 `D4D899CCFB0CF713CD30E12320880EF39CB56313B54596B39BDFA9C7AEB7E8FC`。SDK 与六个正式包均未变。公开开发控制器 session `58581`，全新窗口 `920792`，无上一轮弹窗或通知状态。

为使配置诊断继续真实出现，新增独立的 `review-next.nbexpr`，其内容包含 `add(中文, 2)`、中文引用、`include=native-next.ini`、`teh` 与夹具用途注释。未覆盖第一轮的两个原文件；未通过 UI 删除已经创建的配置文件。

| 复验项 | 实际输入与结果 |
| --- | --- |
| Direct 整组 Undo | 在新文档经原生菜单进入代码动作，预览 `add` → `sum`，应用后编辑区实际显示 `sum` 与未保存状态；点击唯一首条通知的“撤销整组操作”，实际文本恢复 `add`，通知报告已整组撤销。该次按钮完全可见，未沿用第一轮被裁剪按钮的点击结果。 |
| 初始问题焦点 | 新文档的两个正式检查器实际各发布一项。首次 Ctrl+Shift+M 打开后，待帧稳定，真实焦点为 config-check 行；一次 Down 将焦点和蓝色选中项移到 spell-check，同时相关来源和 Fix 改为拼写项。 |
| Enter 与 Escape | 在拼写行按 Enter，列表实际关闭，编辑器定位到 `4:1`。再次打开列表后按 Escape，列表实际关闭，恢复编辑区。 |
| Ctrl+. 最小复现 | 所有提供者就绪后，启动以来第一次 Ctrl+. 没有显示代码动作。真实 `n` 直接提交为 `port=onops`，无 IME 候选；Ctrl+Z 读回 `port=oops` 后再按 Ctrl+.，独立延迟读取仍无动作窗口。原生断言实际 Red：`Native regression RED: Ctrl+. did not show the available code actions`。菜单动作可用；真实事件分派仍待定位，不能以 GPUI 的合成按键 Green 代替该结果。 |

第二轮公开 `stop` 正常退出，exit 0；Sky 返回验收应用 0，隔离目录内受管进程 0。第一轮两个原文件磁盘摘要仍精确相同；新增 `review-next.nbexpr` 未保存，磁盘摘要保持 `99FBF7861F23F8A8A53D6E664B5581D5EE749E49BDF17FFA0A547D97390FA14B`。问题列表修复已获得物理 Green；此时 Ctrl+. 的真实事件仍未定位。

## 第三、四轮：组合键的事件入口与公开重绑定

第三轮仅为诊断，在编辑区和语言请求路径添加临时日志；Ctrl+. 没有进入这些路径，而同焦点 Ctrl+Shift+Space 完整请求成功。由于正常 GPUI 动作也会先于编辑区原始键监听器消费事件，该轮的零日志不足以判断原因，没有据此修改按键传播或提供者。

第四轮将临时日志前移到现有 `ShortcutRuntime` 的公开 keystroke intercept 与最终 observe，保留原事件传播。诊断宿主 SHA-256 为 `AC1C6710B5D88237C5C1BB3DAE9654EA2298480AF2E1A39CBEA305FD87DA051C`；公开控制器 session `69812`，唯一返回窗口 `12321560`。该诊断程序不是最终交付制品。

| 对照 | 真实输入、事件与可见结果 |
| --- | --- |
| 默认 Ctrl+. | 在全新、已就绪的表达式文本焦点，原生反馈仍为 Red。公共 intercept 只收到单独 `control`，resolver 为 Pass、最终 action 为 None，没有收到句号事件。不是已收到组合键后 resolver 冲突、提供者拒绝或过期结果导致。现有证据定位到 GPUI 通用事件入口之前，未改动上游依赖或系统输入法设置。 |
| 参数提示对照 | 同焦点 Ctrl+Shift+Space 收到 `space` 与 control/shift，resolver Pass，最终动作 `me_editor::ShowSignatureHelp`；原文档 ID、revision 0、提供者 generation 2 完整请求成功。可见 `sum(left, right)` 与当前参数 `right`。 |
| 公开快捷键设置 | 在“全局快捷键”搜索“显示代码操作”，用现有“修改快捷键”录入并保存 Ctrl+Shift+F9。只写隔离 `profile/shortcuts.json` 的 schema 1、`me_editor::ShowCodeActions` 对应 context override；没有改用户日常配置。面板快捷键中同名 Input 动作的一次新增冲突已取消，未保存或强制替换。 |
| Direct 实际键盘闭环 | Ctrl+Shift+F9 收到 `f9` 与 control/shift，最终为 `me_editor::ShowCodeActions`；可见表达式来源 `Use sum / 使用 sum`，选中按钮真实有焦点。Return 打开共享预览 `add` → `sum`；Escape 取消后原文 `add` 保持、无未保存状态。 |
| 标准 LSP 实际键盘闭环 | 切换到 `.nbkv` 原文本焦点，以同一绑定显示键值来源 `Use default port / 使用默认端口`。Return 打开共享预览 `oops` → `8080`，Escape 后原文 `port=oops` 保持、无未保存状态。请求绑定原 EntityId(22v1)、revision 0，而非重新采样当前文档。 |

第四轮公共 `stop` exit 0；Sky 返回该验收应用 0，隔离程序进程 0，三个原文档的磁盘摘要保持不变。临时日志只用于定位；移除后的生产宿主复验见下节。默认 Ctrl+. 在本机当前输入环境的物理结果仍是未送达；不将重绑定通过改写成默认组合键通过。已有公开快捷键设置提供可用的键盘入口，无需修改宿主默认绑定或用户输入法。

## 第五轮：干净生产宿主的最终窄复验

全部临时 trace 通过 `apply_patch` 移除；`git diff --exit-code fde5a430 -- crates plugins Cargo.toml Cargo.lock` 为 0，源码没有诊断改动。生产宿主直接 `cargo build -p editor-app --bin editor-app` exit 0、37.84 秒；最终 SHA-256 为 `13B45326BC9B0283D466B7C2C056680EA3DE7610A2352AB7BEFEEB461BEBDCA6`。SDK 49 个文件及六个正式包重新核对摘要与源文件一致、0 Markdown、0 差异；没有为已移除的诊断重复构建访客包。

公开开发控制器 session `64583`，新返回窗口 `15337192`。隔离配置保留第四轮由公开 UI 保存的 Ctrl+Shift+F9；生产宿主启动后真实加载该绑定。

- 标准 LSP：在 `review.nbkv` 原文 `port=oops` 真实焦点按 Ctrl+Shift+F9，显示键值来源与 `Use default port`；Return 打开 `oops` → `8080` 的普通预览，Escape 关闭后读回原文、没有未保存标记。
- Direct：切换到 `review-next.nbexpr` 原文焦点按同一组合键，显示表达式来源与 `Use sum`；Return 打开 `add` → `sum` 的普通预览，Escape 关闭后读回 `add(中文, 2)`、没有未保存标记。
- 每次输入后重新观察可见窗口与焦点；上述结果来自生产窗口，不沿用诊断日志或合成 GPUI 按键作为物理通过证据。

最后公共 `stop` exit 0；Sky 验收应用为空，隔离宿主及 `profile/` 内受管原生程序进程 0，生产日志中临时 trace 0 行。三个文档的 SHA-256 与前述原值逐一相同。问题列表实际 Down/Enter/Escape、整组 Undo、两种语言结果与键盘预览均已通过；当前输入环境对默认 Ctrl+. 的限制如实保留，未扩大为所有 Windows 输入法已验证。其他平台、本轮未实际运行的完整语言工具及 UI 语言切换不宣称原生通过。
