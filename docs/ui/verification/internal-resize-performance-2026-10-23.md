# 内部布局横向拖动与实时换行缓存

历史状态（2026-10-24）：本地上游补丁曾撤回，`gpui-base` 恢复为锁定的 crates.io 0.8.0，副本移到项目外保留。2026-10-25 用户明确允许维护锁定 Base 0.8.0 的本地补丁并保留实时换行，现已恢复、补充真实字体回归并重新验证；当前结果见[新的修复验收](internal-resize-performance-2026-10-25.md)。下文仅保留当时的测试与产物，不能代替当前源码验收。

开始日期：2026-10-23；复验日期：2026-10-24。针对用户截图中资源管理器与编辑区之间的横向分隔条卡顿；上下分隔条作为对照。当前记录按实际完成的检查更新，不把 CPU 计时换算成屏幕帧率。

## 定位证据

在同一个 `source_editor::Editor` 与 `EditorState` 上连续改变窗口宽度和高度，逐帧执行真实控件布局与绘制。测试使用相同的中英文、emoji 代码文本，剔除开始四帧后统计 32 帧；关闭软换行仅用于单变量定位，不改变产品默认行为。

| 输入与配置 | 横向单帧 CPU p50 | 纵向单帧 CPU p50 | 结果 |
| --- | ---: | ---: | --- |
| 原始 Base 0.8.0，1000 行，实时换行 | 4.9672 ms | 0.2351 ms | 性能断言失败 |
| 原始 Base 0.8.0，1500 行，实时换行 | 7.3829 ms | 0.2364 ms | 性能断言失败 |
| 原始 Base 0.8.0，1000 行，关闭换行的定位对照 | 0.2497 ms | 0.2427 ms | 性能断言通过 |
| 缓存补丁，1500 行，实时换行，首轮 | 0.2405 ms | 0.2329 ms | 性能断言通过 |
| 最终补丁，1500 行，实时换行 | 0.2307 ms | 0.2342 ms | 性能断言通过 |
| 最终补丁，实际 `main.rs`，1253 行 | 0.1973 ms | 0.1943 ms | 性能断言通过 |

首轮修复后横向 p95 为 0.3189 ms。日志分别为 `target/internal-resize-cpu-1000.log`、`target/internal-resize-cpu-1500.log`、`target/internal-resize-cpu-no-wrap.log`、`target/internal-resize-cpu-fixed.log`。测试名为 `source_resize_frame_cost`，显式运行 ignored 测试；默认 1500 行，可用 `NANOBUG_DRAG_SOURCE_ROWS` 或 `NANOBUG_DRAG_SOURCE_PATH` 指定输入。

最终 1500 行横向 p95 为 0.2427 ms、实际 `main.rs` 横向 p95 为 0.3726 ms；日志为 `target/internal-resize-cpu-final.log`、`target/internal-resize-cpu-main.log`。这组 CPU 基准使用 800 逻辑像素起始宽度，窄视口与原生字体的验证另列如下。

原因位于锁定依赖 `gpui-base 0.8.0` 的 `TextWrapper::set_wrap_width`：每次宽度变化都重新测量全文换行。高度变化不进入这条路径，因此横纵向耗时差异随文档行数增长。此前空白分栏夹具的高刷新率结果不能覆盖此问题。

## 修复与边界

此前把“保持拖动时实时换行，允许针对上游依赖做性能修复”扩大解释为允许整包复制上游源码，并以 `vendor/gpui-base` 路径覆盖依赖。此解释不符合既有约束，现已撤回；以下仅记录当时的修复方案与验证证据。

- 记录原换行算法每次字形宽度比较成立的视口宽度区间，复用仍在区间内的逻辑行；不估算中英文字符宽度。
- 行摘要保存所有区间的交集。整篇文档仍适用时直接返回；只有部分行跨过边界时，复用其他行并重算受影响行。
- 优先搜索合法 Unicode 单词断点，超长单词仍按完整字素拆分。原算法先搜索字符再退回单词，导致同一个单词内部的字符临界点也触发无效重算；新算法保留原算法作为测试中的独立差分基准。
- 宽度变化期间复用精确的原生文本片段测量值，上限为 32,768 项与 4 MiB 文本负载，另有受条目数限制的容器开销。其他更新清空缓存；不长期保留原生字形资源，也不使用字符数估算宽度。
- 边界向内收缩一个浮点步长，避免缩进相减的舍入误差；不确定情况回到原算法。
- 文本、字体、缩进和内联元素的既有失效流程保留。内联元素不启用宽度复用，换行开关切换仍重新生成映射。
- `EditorState` 继续独占文本、选择、IME 与撤销状态；拖动中实时换行，无延迟、量化宽度或松手后重排。

首次打开、改变字体以及大量行同时跨越换行边界仍需要测量。本修复减少重复工作，不承诺任意文件、硬件与插件负载下均达到显示器刷新率。

## 验证入口

```powershell
cargo test -p editor-ui source_resize --release --locked --offline -- --nocapture
cargo test -p editor-ui source_resize_frame_cost --release --locked --offline -- --ignored --nocapture

# 上游算法测试独立于产品 workspace；保持独立输出目录。
$env:CARGO_TARGET_DIR = "$PWD/target/gpui-base-resize-tests"
cargo metadata --manifest-path vendor/gpui-base/Cargo.toml --no-deps --format-version 1
cargo test --manifest-path vendor/gpui-base/Cargo.toml --features test-support --lib text_wrapper --locked -- --nocapture
Remove-Item Env:CARGO_TARGET_DIR
```

上游差分回归覆盖宽度收缩、扩大、跳跃、临界点相邻浮点数、中文、组合字符、emoji、阿拉伯文、CRLF 和两种缩进策略；失效回归覆盖编辑、字体、内联元素及换行开关。源编辑器回归检查拖动中即时换行、两种主题下的 IME 标记与撤销。

原生验证使用 `window::tests::native_window_drag_tracks_display_refresh`，以 1500 行真实源编辑器或 `NANOBUG_DRAG_SOURCE_PATH` 指定的实际源码加载 `split`、`split-vertical` 和 `dock-resize` 夹具。只统计前台窗口实际几何变化时的绘制，鼠标产生频率不作为 FPS。

2026-10-24 用真实按键状态配合鼠标位置复测后，空分栏与纵向 1500 行均为约 159.38 次/秒（当前显示器 160 Hz）。首轮只有宽度区间缓存的补丁在窄编辑区横向拖动中仅产生 6 次实际几何变化，证明宽视口 CPU 基准不能代替原生压力验收。加入精确片段缓存后升至 134.41 次/秒，但最长间隔仍达 181.780 ms，仍未通过 90% 显示器刷新率门槛，因此继续改为优先搜索合法断点。

最终原生窗口为 800×500 逻辑像素，左面板初始宽度 230，加载同一原始 `EditorState`。当前显示器报告 160 Hz，窗口截图为 1200×750，覆盖 150% 缩放。两秒实际鼠标拖动剔除首尾各 150 ms，只统计前台窗口中的有效位移绘制；下列成功记录均通过刷新率门槛及普通子进程退出检查。

| 最终场景 | 位移绘制速率 | 间隔 p95 | 最大间隔 | 日志 |
| --- | ---: | ---: | ---: | --- |
| 1500 行，横向分栏，深色 | 154.71/s | 6.720 ms | 58.782 ms | `internal-resize-native-word-cache.log` |
| 1500 行，纵向分栏，深色 | 158.82/s | 6.764 ms | 12.776 ms | `internal-resize-final-split-vertical.log` |
| 1500 行，停靠分隔条，浅色 | 154.11/s | 6.814 ms | 59.419 ms | `internal-resize-dock-repeat.log` |
| 1253 行实际 `main.rs`，横向分栏，深色 | 160.08/s | 7.132 ms | 11.723 ms | `internal-resize-final-main.log` |

上述日志均在 `target/`。已查看实际 `main.rs` 和混排文本截图，换行、行号及字素呈现正常。测试使用真实本地源编辑器与停靠/分栏控件，但未把资源管理器、已安装插件和语言服务器全部装入该夹具，不代表完整宿主所有负载下的帧率。

停靠首轮开启截图导出后，统计到 154.67/s，但子进程在退出阶段报 `0xc0000005`，该轮不计通过。随后同负载不截图为 154.11/s，开启截图复测为 147.65/s，均正常退出；空停靠对照为 159.39/s。未复现的退出异常保留在 `target/internal-resize-final-dock-resize.log`，不声称已修复其原因。1500 行均跨越断点的压力场景仍有约 59 ms 的单次慢帧，因此不承诺完全消除所有卡顿。

上游输入相关测试（含换行、选择、IME、撤销、内联元素）为 **360 passed / 0 failed / 0 ignored**，见 `target/internal-resize-base-input-tests.log`；其中独立原算法差分回归位于 `wrap_width/tests.rs`。本地源编辑器的即时重排与 IME/撤销回归也通过，见 `target/internal-resize-word-cache-build.log`。

## 工作区检查与交付

| 检查 | 结果 | 日志（`target/`） |
| --- | --- | --- |
| `cargo fmt --check` | 通过 | `internal-resize-fmt.log` |
| 补丁源码单独 `rustfmt --edition 2024 --check` | 通过 | `internal-resize-vendor-fmt.log` |
| `cargo check --workspace --locked --offline -j 1` | 通过 | `internal-resize-workspace-check.log` |
| `cargo test --workspace --exclude editor-app --locked --offline -j 1` | 546 passed、0 failed、397 ignored；共 110 组（含空 doctest） | `internal-resize-workspace-tests.log` |
| 上游输入相关测试 | 360 passed、0 failed、0 ignored | `internal-resize-base-input-tests.log` |
| `git diff --check`、本记录链接检查 | 通过 | `internal-resize-diff-check.log` |

跳过的测试未计为通过。原生帧率与 CPU 基准均已另行显式运行；本轮未修改 `editor-app` 产品源码。工作区已有其他任务的大量改动，本记录不把它们归入本次成果。

与原始 `.crate` 逐文件比对，唯一改变的原有上游源码是 `text_wrapper.rs`，新增四个文件（含 `PATCHES.md`）；原清单、锁文件与许可证均保留。审计结果见 `target/internal-resize-vendor-audit.json`。根锁文件仅将 Base 0.8.0 切换为同版本的本地补丁；先前其他工作区改动保留。产品源码和根依赖清单/锁文件未引入 `gpui-component`。

普通运行配置使用 Release，本轮执行
`cargo build -p editor-app --bin editor-app --release --locked --offline -j 1`
成功，耗时 6 分 08 秒。构建保留现有 unused/private-interface 与 Wasmtime/Tree-sitter 链接警告；没有把警告记为编译失败。日志为 `target/internal-resize-release-build.log`。

产物为 `target/release/editor-app.exe`，86,687,232 字节，2026-10-24 11:28:56（Asia/Shanghai）生成；SHA-256 为
`88c49eb86c11fc1c46629fb8c1263f61fde9e1d585cbb822bee1a8683e8b0a70`。
元数据保存在 `target/internal-resize-release-artifact.json`。该历史产物使用现已撤回的本地 Base 0.8.0 补丁，不代表当前源码；不能通过重启该产物验证恢复发布依赖后的行为。当时未提交、推送或替换安装目录中的程序。

返回[原生 UI 文档目录](../README.md)。
