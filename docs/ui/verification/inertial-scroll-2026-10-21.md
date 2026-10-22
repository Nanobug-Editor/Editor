# 惯性滚动与原生高刷

日期：2026-10-21。承接[第一阶段平滑滚动](smooth-scroll-2026-10-21.md)。本轮在同一 `editor-ui::scroll` 入口加入速度连续的惯性，按用户后续要求跟随 GPUI 的实际刷新能力，不在产品中固定 120FPS。

本文保留该阶段的帧率和完整源布局限制；后续优化见[比较视图的有界原生布局](comparison-viewport-scroll-2026-10-21.md)。

## 上游能力与目标

仓库继续使用 Kit／Base 0.7.0 与 `gpui-pre` 0.3.7；后者的 Cargo 元数据对应 Zed `1a28cff4b409169bac058bca40dfbfeb7621d19b`，没有升级依赖或修改上游源码。

- [锁定版本的 Windows VSyncProvider](https://github.com/zed-industries/zed/blob/1a28cff4b409169bac058bca40dfbfeb7621d19b/crates/gpui_windows/src/vsync.rs)使用 `DwmFlush` 同步桌面合成刷新，并通过 `DwmGetCompositionTimingInfo` 获取周期。`16_666µs` 是读取失败时的回退周期，不是正常运行的 60FPS 上限。
- [窗口帧调度](https://github.com/zed-industries/zed/blob/1a28cff4b409169bac058bca40dfbfeb7621d19b/crates/gpui/src/window.rs)没有统一的 120FPS 硬上限；后台窗口默认约 30FPS，严重热压力下约 60FPS。前台正常运行时受显示器、系统合成与实际帧耗时约束。
- 本机显示模式约 144Hz，目标是接近这一本机可观察刷新上限。其他刷新率继续由 GPUI 原生回调驱动；不靠增加固定频率定时器或持续空转制造 FPS 数字。

## 实现

`scroll/inertia.rs` 直接复用上游 `SpringConfig::step` 和 `SpringState`，保存两个方向的位置、速度、目标和采样时刻。临界阻尼参数为 `k=324, c=36, m=1`，停止阈值为 `0.25` 逻辑像素；停止同时检查位移和速度。同向滚轮保留速度，松手后继续衰减，反向输入制动该方向，另一方向保留动量。

`scroll/smooth.rs` 使用 `Window::on_next_frame` 跟随原生 vsync。每个活动视口最多保留一个待执行回调，两轴共用；额外通知和重绘不重复排队。停止、关闭、原生定位、拖动、键盘及减少动态效果设置都能撤销惯性。精确像素输入交还原设备及上游处理，避免重复叠加触控板惯性。

比较视图的原生诊断确认，几何缓存只在初始化时建立，慢帧主要来自多个完整源绘制副本。`aligned_editor/pane.rs` 将可见副本的分配高度限制到可见源区间下沿，加两行余量；完整原生权威挂载继续保留，以提供输入、IME 和任意位置定位所需的全源几何。源文档深处和密集改动仍存在完整布局成本，因此不把这一优化描述为比较视图已满刷。

每次输入及采样都是常数级计算，沿用原始 `ScrollHandle`、虚拟列表和原生 `EditorState`，不遍历文件或列表、不复制文本和撤销栈。已接入的编辑器、文件树、列表、标签和共享比较视口统一使用这一实现。应用调用点与第一阶段保持相同接口。

## 反馈循环与测试条件

诊断产物位于 `target/scroll-120hz-2026-10-21/`。开工时保存已有滚动源码到 `baseline/`；它包含之前已完成的工作，不把整个脏工作区计作本轮修改。

可重复的惯性回归命令：

```powershell
cargo test -p editor-ui --offline smooth_scroll_inertia_moves_on_each_120hz_frame_after_release -- --nocapture --test-threads=1
```

修复前 `inertia-red.log` 中该测试失败：以 120Hz 推进，位置在第 18 帧约 150ms 后停在 `138`，200–250ms 没有剩余惯性。修复后的同一真实滚轮／原生偏移路径通过。另覆盖 60／120／240Hz 及停顿后的解析结果一致、速度保留、独立方向制动、边界收缩、120 次突发输入、虚拟化、单一待执行回调、定位与减少动态效果设置。

原生验收使用真实 Windows／D3D 窗口、生产控件和停靠式缓存。记录布局开始到子树绘制完成的 CPU 时间，并在每次真实绘制完成时记录时间戳与原生偏移。20ms 定时器只派发／观察测试输入，不用来计算绘制 FPS。验收为前台窗口；硬件门槛通过测试环境变量显式设置。

首轮 `native-baseline-release.log` 的窗口失焦，平均约 81.82FPS；这不是有效的前台基线，不能与后续数字相减来声称性能提升。测试改为在输入前激活自有窗口，前台及后台样本分开。比较视图的初次测试还暴露了支架用类型化 `WindowHandle<Root>::update` 派发输入、重复借用 Root 的问题；改用 `AnyWindowHandle` 后沿用正常原生输入的借用边界，没有因此修改比较视图的生产行为。

## 验证结果

以下为最终源码的 Release 原生绘制观测。前四项设置 `NANOBUG_SCROLL_MIN_FPS=140`，在本机约 144Hz 模式下通过门槛；比较视图通过滚动、惯性尾段、停稳和文本不变断言，未达到同样的高刷门槛。

| 场景 | 实际绘制 FPS | 帧间隔 P95 | 子树 CPU P95 | 日志 |
| --- | ---: | ---: | ---: | --- |
| 1800 行中文编辑器 | 143.90 | 7.484ms | 0.415ms | `native-verified-editor.log` |
| 10,000 项原生虚拟树 | 143.91 | 7.360ms | 0.263ms | `native-verified-tree.log` |
| 横向标签滚动视口 | 143.90 | 7.406ms | 0.082ms | `native-verified-tabs.log` |
| 普通嵌套滚动容器 | 143.94 | 7.380ms | 0.100ms | `native-verified-nested.log` |
| 1800 行双栏比较 | 71.22 | 14.247ms | 14.056ms | `native-verified-comparison.log` |

比较视图的同机连续复测使用修改前冻结程序和最终程序：32.09 → 65.83FPS，子树 CPU P95 为 33.168 → 15.773ms，日志 `comparison-idle-{baseline,final}.log`。更早的运行分别为 25.48 与 48.69FPS，见 `native-final-release-comparison.log`、`native-prefix-release-comparison.log`；机器负载会明显影响这类长文件，不能把某次峰值当成恒定帧率。高刷门槛失败的结果保留，没有计作通过。

复查命令（`NANOBUG_SCROLL_SURFACE` 可为 `editor`、`tree`、`tabs`、`nested`、`comparison`）：

```powershell
$env:NANOBUG_SCROLL_SURFACE = 'editor'
$env:NANOBUG_SCROLL_MIN_FPS = '140'
cargo test -p editor-ui --release --offline --jobs 2 smooth_scroll_real_windows_paints_intermediate_native_positions -- --ignored --nocapture --test-threads=1
Remove-Item Env:/NANOBUG_SCROLL_MIN_FPS
Remove-Item Env:/NANOBUG_SCROLL_SURFACE
```

其他检查：

| 命令／检查 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo test -p editor-ui --offline --jobs 2` | 93 通过，11 ignored；没有将 ignored 计为通过 | `ui-tests.log` |
| `cargo test -p editor-ui --offline --jobs 2 -- long_diff_scroll long_marked_diff --ignored --nocapture --test-threads=1` | 2/2 通过；普通长比较重探测与对齐重建均为 0，密集底色原生行读取为 63，仍在视口预算内 | `long-comparison-tests.log` |
| `cargo test -p editor-ui --offline --jobs 2 aligned_pointer::tests:: -- --ignored --test-threads=1` | 6/6 通过；原生拖动、取消、关闭重挂载、Unicode Undo 和 Root 插件边界回归 | `native-pointer-tests.log` |
| `cargo test -p editor-app --offline --jobs 2 -- editor::viewport::tests:: editor::comparison::render_tests:: tests::preview_tabs:: --test-threads=1` | 22/22 通过，覆盖原生定位、包装宽度、比较底色、预览标签、中文 preedit 与 Undo | `app-regressions.log` |
| `cargo check --workspace --offline --jobs 2` | 通过；保留既有警告 | `workspace-check.log` |
| `cargo fmt -p editor-ui --check`、`git diff --check` | 通过 | `ui-fmt-final.log`、`diff-final.log` |
| `cargo fmt --check` | 未通过；差异在其他工作区改动，例如 `editor-app/src/extensions/management/diagnostics.rs`、`extensions/worker/queued.rs` 等，本轮文件格式通过 | `fmt-final.log` |
| `cargo test --workspace --exclude editor-app --offline --jobs 2` | 未通过；停在 `plugin-runtime/tests/capability_packages.rs` 的两个英文错误文本断言，实际外层错误为中文 API 不兼容提示；本轮未修改该路径 | `workspace-tests.log` |

依赖树确认仍为 Kit／Base 0.7.0、GPUI 0.3.7，无 `gpui-component`。`verification.json` 汇总原生结果，`inputs-before-final-release.json` 与 `inputs-after-final-release.json` 的 11 项选定源码摘要完全一致；工作区和输出目录分别是当前 `Editor` 与其 `target`。源码相对本轮开工快照的增量保存在 `task-source.diff`，没有把此前已有迁移和其他任务的改动计为本轮成果。

原生验证程序 `native-validation.exe` 的 SHA-256 为 `9558f928401e5034019ae541b8c9cf877be3e6800ad92b79edd7427d882af96d`；这是测试程序，不是宿主发行程序。

## 范围

原生帧测量覆盖独立窗口中的代表性滚动子树，FPS 是实际原生绘制节奏，不是 `1000 / CPU耗时` 的估算，也不等于直接测量显示器扫描输出。240Hz 运动一致性由可控时钟验证；本机没有对 240Hz 显示设备进行原生验收。完整宿主的插件负载、密集改动底色、不同设备和系统负载会影响最终帧率；本轮不宣称任意文档及负载都能恒定满刷。

返回 [UI 目录](../README.md)。
