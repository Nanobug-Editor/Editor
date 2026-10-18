# 底部问题面板验收

日期：2026-10-14。依据用户截图中的调整要求：取消“问题”弹窗，默认在底部以可拖动面板显示，诊断按文件归类，状态栏使用图标与数字。后续截图补充文件组折叠、文件类型图标、移除定位按钮和隐藏零计数。

状态：实现、原生交互验收和仓库要求的检查均已完成。本次没有提交、推送或发布。

## 当前行为

- “问题”是现有 Base DockArea 中的独立叶节点，默认位于底部。复用项目本地停靠外观及上游拖动、调整大小和布局序列化行为；关闭面板不会关闭相邻面板或它们的资源。
- 点击状态栏或按 Ctrl+Shift+M 打开列表。面板位置、大小和显示状态随工作区会话保存；旧会话缺少该节点时补到默认底部，不覆盖其他布局。
- 诊断汇总所有已打开文本文件的现有模型，按工作区相对路径分组；每条显示严重级别图标、消息、来源和行列。文件标题复用文件树/标签页的插件及主题图标解析，并带可点击的展开箭头。范围是已打开文件中已有的诊断，不额外启动全工作区扫描。
- 文件组默认展开，支持鼠标和 Base 按钮的 Enter/Space 激活；标题获得焦点时左/右键折叠/展开。折叠选择随诊断更新保留，在面板刷新时清理已关闭文件的状态；诊断清除后重新发布不会重置该选择，不额外保存诊断内容。
- 状态栏按非零级别显示图标及数量；错误、警告、信息/提示分别隐藏零计数，全部已打开文件没有诊断时移除计数入口。Ctrl+Shift+M 仍可打开空列表。折叠不减少总计数，当前文件没有问题时其他已打开文件的计数仍然保留。
- 点击条目或按 Enter 定位到对应文件，列表保持打开；移除重复的“定位问题”按钮。没有快速修复或相关位置时整个操作区不占高度。方向键、Home、End 跨展开文件组选择和滚动，全部折叠时方向键仍可选择文件标题；面板获得焦点时 Escape 隐藏并归还原焦点，编辑器仍可独立接收中文输入。
- 快速修复、相关位置和预览操作继续校验原始文档版本、集合归属及实例存活。面板不维护第二份文本或可变诊断缓存。

## 后续截图调整验收

文件标题改用本地 Base 按钮，提供展开箭头、原生 expanded 语义及共享文件类型图标；列表的可见索引计入折叠文件标题，避免方向键和滚动命中隐藏诊断。移除“定位问题”按钮及未使用翻译，快速修复/相关位置操作区仅在有动作时存在。

折叠标题获得焦点后，诊断清空可能直接移除该标题，而可见诊断数仍然为零。此时显式恢复面板焦点，保持 Home/方向键和展开操作可用。标题使用自动高度和最小高度，适应共享图标随基础字体缩放。

末轮采用与初版相同的 debug=0 隔离配置，以 `cargo test -p editor-app --no-run … --message-format=json` 获取实际测试产物；确认包含六项问题面板测试后，在同一 `target/debug/deps/` 目录保留 `problems-disclosure-native-validation.exe`。末轮 SHA-256：`424d6d25da3851993007c3b4e1638654a8575cfa6c8e795dc10dd4449881fb8a`。

| 检查 | 实际结果 |
| --- | --- |
| 保留产物运行 `native_problems --test-threads=1` | 6 passed：新增鼠标/Enter/Space/左/右键折叠、隐藏条目导航、清除再发布后的折叠状态、折叠标题消失后的焦点恢复、深浅主题及 10/24/14px 基础字体布局、各级别零计数隐藏；保留原定位、滚动、拖动、布局恢复和中文输入覆盖 |
| 保留产物运行 `editor::diagnostics:: --test-threads=1` | 9 passed，包含上述四项诊断列表测试及原诊断发布/定位/悬浮回归 |
| 保留产物运行 `ui::icons::tests:: --test-threads=1` | 3 passed，验证共享图标规则和多色 SVG 呈现 |
| 保留产物显式运行 `native_community_language_diagnostics_fix_preview_source_revision_and_undo --ignored --test-threads=1` | 1 passed、104.49 秒；真实固定 Package 验证快速修复、相关位置、预览取消、过期拒绝和 Undo |
| `cargo fmt --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked -j 1 --config profile.test.package.plugin-protocol.debug=0 -- --test-threads=1` | 274 passed、0 failed、256 ignored；忽略项不计通过 |
| `cargo check --workspace --locked -j 1` | 通过，保留已有警告 |
| `git diff --check` 及受影响文档链接 | 通过 |

日志为根 `target/problems-disclosure-native-build.log`、`problems-disclosure-native-tests.log`、`problems-disclosure-diagnostics-tests.log`、`problems-disclosure-icons-tests.log`、`problems-disclosure-fixes-tests.log`、`problems-disclosure-workspace-tests.log`、`problems-disclosure-workspace-check.log`、`problems-disclosure-format.log` 和 `problems-disclosure-diff-check.log`。没有更新插件包、全局工具链或安装器产物。

## 初版实际验证

Windows x64，现有 Visual Studio 18 x64 与仓库锁定依赖，没有安装或升级工具。首轮 UI 检查复用 debug=1 构建；发现共享目标目录的同名产物被其他源码覆盖后，末轮使用独立配置：

```powershell
--locked -j 1 --config profile.test.package.editor-app.debug=0 --config profile.test.package.plugin-protocol.debug=0
```

| 检查 | 实际结果 |
| --- | --- |
| `cargo test -p editor-app native_problems … -- --test-threads=1` | 4 passed：文件分组、非活动文件更新、定位、80 条列表的键盘滚动、真实鼠标拖动与缩放、第二窗口布局恢复、深浅主题及中文输入 |
| 保留的 `problems-panel-native-validation.exe editor::diagnostics:: --test-threads=1` | 7 passed，含两项上述列表测试，以及诊断过期拒绝、语义合并/清除、滚动与 Unicode 悬浮、F8 定位 |
| 保留产物显式运行 `native_community_language_diagnostics_fix_preview_source_revision_and_undo --ignored --test-threads=1` | 1 passed，126.11 秒：真实固定 spell-check/config-check Package，经公开管理器进入原生列表，验证预览、取消、相关位置、过期拒绝和 Undo |
| `cargo fmt --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked -j 1 --config profile.test.package.plugin-protocol.debug=1 -- --test-threads=1` | 268 passed、0 failed、256 ignored；忽略项不计通过 |
| `cargo check --workspace --locked -j 1` | 通过；保留工作区已有警告 |
| `git diff --check` | 通过；保留工作区已有修改 |

真实 Package 输入按固定摘要账本由测试加载，未改包版本、字节或测试专用宿主 API。准备规则见[固定夹具说明](../../../crates/plugin-runtime/tests/fixtures/packages/README.md)。测试二进制位于 `target/debug/deps/`；上述文件名仅记录本次实际产物，后续修改应通过 Cargo 重新编译后选择对应产物。

末轮原生测试产物是 `editor_app-4520fe06751e4204.exe`，检查包含全部四项当前面板测试后复制到同一 `target/debug/deps/` 目录的 `problems-panel-native-validation.exe`，保留相邻 examples 查找行为。保留产物 SHA-256：`4d6918f0a22e7a05ce237260e7023a7e4429b233669585439d050ccb356b933b`。

日志位于根 `target/problems-native-isolated-tests.log`、`problems-diagnostics-final-tests.log`、`problems-fixes-final-tests.log`、`problems-workspace-tests.log`、`problems-workspace-final-check.log` 和 `problems-format-check.log`。

## 修正与限制

- 首次测试编译复用了缺少既有 ContentStyle 等接口的旧依赖产物。确认当前源码中接口完整后，只刷新 plugin-protocol/plugin-runtime 根源码的时间戳，SHA-256 内容哈希前后相同；重新编译后接口错误消失。没有为此修改接口或清空共享 target。
- 原生验收发现空状态缺少无障碍名称，已补齐本地控件语义。拖动测试使用 Base 的内部 leading 边界命中区域；两种主题复用同一个测试快捷键 profile。
- 跨文件定位会替换正在绘制的编辑区；关闭面板时归还当前文件的焦点，避免返回原先已经切换走的编辑区。原生分组/定位测试覆盖此边界。
- 原生证据来自 GPUI 实际布局、鼠标事件、完整键盘按下/抬起及文本输入；未另做操作系统中文 IME 候选窗口、人工可见窗口或 macOS/Linux 验收。未重新打包安装器。

返回[UI 文档目录](../README.md)。
