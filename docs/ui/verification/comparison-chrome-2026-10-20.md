# 比较区单滚动条、等宽换行与顶部背景

日期：2026-10-20。状态：实现、门禁、实际包验收与配套候选交付完成。本文仅记录本轮增量，不把既有折叠、箭头导航、整行底色及内侧行号槽作为新增成果。

## 用户确认与实现边界

- 比较区仅在最右侧提供一个垂直滚动条，正常比较保持同步。两／三栏使用相同的实际文本换行宽度，扣除阅读边距、原生行号槽、滚动条及分栏像素取整的影响。
- 箭头／折叠工具栏与两侧或三侧的来源标题统一使用主题的 `muted` 表面背景，标题底部使用主题边框，与滚动内容区分。来源标题保持单行，完整来源信息仍属于原比较资源。
- 移除“关闭比较”工具栏按钮；比较标签原有关闭入口与资源清理继续保留。
- 修改位于宿主通用 `ui/controls` 和比较呈现层。独立 Git 插件仍为 0.1.19，公开 `editor.diff 1.4` 不变，没有 Git 专属 API、访客源码变化或上游副本。

## 诊断与回归

先建立原生失败用例，再验证三个优先假设：重复预留阅读／滚动条宽度、原生行号槽占宽、分栏像素取整。

1. 修复前测试 EXE 的两项回归均失败：中间仍有垂直滚动条；相同内容的原生输入区分别为 476px 和 488px。日志 `target/comparison-single-scrollbar-red-2026-10-20.log`。
2. 按比较策略保留最右侧轨道，并平衡既有阅读边距与轨道预留后，同两项回归 2/2 通过。日志 `comparison-single-scrollbar-green-2026-10-20.log`。
3. 扩展边界测试后 1 通过／3 失败：原生行号槽跨三位／四位时文本宽度为 438.8px 和 430.40002px；奇数宽度与三栏出现 1px 的列分配偏差。日志 `comparison-single-scrollbar-edges-2026-10-20.log`。
4. 改为读取原生字形原点、输入边界和独立测量的实际外列宽度，为所有来源选择共同文本宽度；测量不包含自身追加的 inset，避免循环缩窄。两主题、100%／150% 缩放、650／1000／1311 宽度、行号位数变化及两／三栏折叠回归 4/4 通过，11.85 秒。日志 `comparison-single-scrollbar-normalized-2026-10-20.log`。
5. 首次加入滑块拖动验收后 3 通过／1 失败，日志 `comparison-single-scrollbar-gestures-2026-10-20.log`。原因是测试直接按下尚未显示的滑块；Base 的 `Scrolling` 模式在滚动前不注册隐藏滑块的按下行为。改为真实滚轮显示滑块、读取已绘制滑块像素后拖动，最终回归通过，未因此修改生产滚动行为。

每个字节对应字符的实际原生 Y 坐标用于验证换行位置；仅比较外框宽度不算通过。新增真实右侧滚动条拖动和静止后不重复调度帧的验收。实际 0.1.19 包测试检查两／三栏顶部背景已绘制、来源标题位置、轨道数量及工具栏按钮移除。

## 最终验证与候选

日志位于 `C:/Projects/RustProjects/Editor/target/`。最终测试 EXE 为 `comparison-single-scrollbar-tests-final-2026-10-20.exe`；实际包与既有原生输入使用 `comparison-single-scrollbar-tests-delivery-2026-10-20.exe`，二者生产源码相同，后续仅修正滑块测试的可见性前置条件。

| 实际入口 | 结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过；后者保留既有行尾转换提示 | `comparison-single-scrollbar-fmt-2026-10-20.log`、`comparison-single-scrollbar-diffcheck-2026-10-20.log` |
| `cargo test --workspace --exclude editor-app --jobs 2` | 82 组，325 通过、322 ignored、0 失败 | `comparison-single-scrollbar-nonui-2026-10-20.log` |
| `cargo check --workspace --jobs 2` | 通过，保留既有 warning | `comparison-single-scrollbar-check-2026-10-20.log` |
| 最终测试 EXE：`editor::comparison::render_tests:: sdk_export::tests:: ui::controls::aligned_ ui::plugin:: tests::preview_tabs:: --test-threads=1` | 126 通过、23 ignored、0 失败，29.12 秒；含最终实际可见滑块拖动、逐字符换行、静止不持续调度、内侧行号槽与折叠回归 | `comparison-single-scrollbar-ui-final-2026-10-20.log` |
| `ui::controls::aligned_ code_sync::alignment_tests --ignored --test-threads=1` | 显式执行 20/20，81.43 秒；输入、中文 IME、Undo、指针路由、三栏与独立窗口接缝 | `comparison-single-scrollbar-native-input-2026-10-20.log` |
| `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs git_inline_history_preview_opens_readonly_two_and_three_pane_tabs --ignored --test-threads=1` | 显式执行 2/2，193.36 秒；同一真实 0.1.19 ZIP 经公开 Manager，双／三栏两主题背景绘制、轨道数量、工具栏关闭按钮移除、文件导航、草稿／Undo 与清理 | `comparison-single-scrollbar-real-package-2026-10-20.log` |

实际包使用 `NANOBUG_GIT_CANDIDATE=C:/Projects/RustProjects/Editor/target/unchanged-folds-2026-10-20/git-0.1.19.zip`。本轮无需修改或重新构建访客。ignored 未执行项不计通过；前轮记录的旧社区比较辅助功能命名失败未改写为本轮通过。用户已有窗口布局图摘要仍为 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6`。

配套候选位于 [comparison-single-scrollbar-2026-10-20](../../../target/comparison-single-scrollbar-2026-10-20/)，包含 `Nanobug.exe`、既有独立插件 ZIP 的原样副本、`identity.json`、`SHA256SUMS` 与使用说明。

| 产物 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `Nanobug.exe` | 77,766,656 | `99f84ced55f913121996a1798726d82ebead1cf57157605aad2c7f78a36f4496` |
| `git-0.1.19.zip` | 836,113 | `c43afe7a04d2ac69ca47c7a5e288cb6dae11ecb1c34cf87a1b99d1b90cb845de` |

构建入口为 `cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/comparison-single-scrollbar-2026-10-20/Nanobug.exe`，2 分 22 秒完成；日志 `comparison-single-scrollbar-release-2026-10-20.log`。已核对 `cargo metadata --no-deps --format-version 1` 的 workspace／target，39 个选定宿主输入在构建前后摘要一致；这是 `main` 的 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 之上的既有脏工作区候选，不是完整源码快照。依赖树仅有 Kit／Base 0.7.0，无 gpui-component。

复用包及其中 WASM 摘要与前阶段完全一致；SDK 摘要仍为 `0739a6f43e2faeef07ad4ee48a45fbc8c532191352cf8da04169a56d439e18da`。未覆盖用户正在运行的编辑器，未自动安装插件、重启、提交或推送。

返回 [UI 目录](../README.md)；相关 [Git 规格](../../plugins/specs/git-plugin.md) 与 [上一阶段折叠验收](../../plugins/verification/git-unchanged-folding-2026-10-20.md)。
