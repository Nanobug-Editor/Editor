# 长代码差异滚动性能

日期：2026-10-21。状态：本轮性能优化、针对性验证、实际包比较及候选交付完成；额外 Windows 整流程用例保留已确认的既有验收脚本失败，详见下文。

## 范围与复现

用户报告长代码的差异窗口滚动卡顿。本次处理宿主通用的原生对齐编辑控件，适用于主编辑区及插件使用的两／三栏比较，不向独立 Git 插件增加专属接口，不复制或修改上游 Kit／Base。

真实 `ScrollWheelEvent` 驱动已挂载的原生编辑器，逐轮检查共享滚动位置确实改变、仍为 `Ready`，并确认源文本与选择不变。默认夹具为 1800 行包含中文、emoji、软换行的代码，右侧在中段插入一行；每轮包括滚轮分发及两次有限后续绘制。测试计时包含这些绘制，不能解释为单帧时间或 Release 帧率。

日志与测试程序位于 `target/diff-scroll-2026-10-21/`。回归入口为 `cargo test -p editor-app long_diff_scroll -- --ignored --nocapture --test-threads=1`。`NANOBUG_DIFF_BENCH_ROWS=240`、`NANOBUG_DIFF_BENCH_WHEELS=2` 仅用于缩短同一路径的诊断循环；默认仍运行原始规模。断言使用实际逻辑行探测次数，避免将机器负载导致的计时浮动当成产品错误。

## 修复前证据

| 场景 | 实际结果 | 日志 |
| --- | --- | --- |
| 原始 1800 行、8 轮 | 失败；各轮 4.65–4.71 秒，重复探测 86,472 行；测试共 55.50 秒 | `baseline-red.log` |
| 缩小至 240 行、8 轮 | 同一断言失败；各轮约 144–147 毫秒，重复探测 11,592 行 | `minimized-rows-red.log` |
| 240 行、2 轮 | 同一断言失败；测试 0.83 秒，重复探测 2,898 行 | `minimized-wheels-red.log` |
| 定向计时，1800 行、2 轮 | 滚动共约 9.306 秒，其中行几何读取约 8.248 秒（89%）；重复探测 21,618 行 | `timing-large.log` |
| 第一次修复后的彩色段小场景：120 行、2 轮 | 失败；仍额外调用 2,160 次原生底色行坐标；测试 0.58 秒 | `marked-minimized-red.log` |
| 第一次修复后的彩色段原始场景：1800 行、2 轮 | 失败；每轮约 3.75 秒，额外调用 30,960 次底色行坐标 | `marked-large-red.log` |

以上均为本机 `test` 未优化配置。计时之前已捕获当前控件源码到 `baseline/`，其文件摘要在 `baseline/inputs.json`；它包含此前用户已确认的改动，不把这些既有实现计作本次成果。

根因是每轮绘制都遍历整份源文件，并为每个逻辑行调用原生 `range_to_bounds`。当前 Base 0.7.0 的该公开接口内部顺序寻找布局行，逐行调用因此随文件增长呈二次成本。滚动改变绝对坐标还会导致已对齐的元数据被当成新布局重建。初始需要原生测量实际换行；平移已测量的显示区域不应再次探测所有行。

进一步使用每七行一处真实改动、带原生整行底色和比较轨道的场景，发现底色可见性检查仍为离屏标记重复查询原生行坐标。这解释了只缓存对齐测量不足以改善大量改动文件的原因。该路径同样先保留失败记录，再复用已验证的几何。

## 实施与验证

- `aligned_editor/geometry_cache.rs` 保存当前源计划的只读几何。以源／折叠 epoch、真实原生输入尺寸与字形 inset、EOF 布局、字体样式、根字号和 DPI 验证；实际内容变化及未被本次完成绘制覆盖的原生通知撤销缓存。保留原生私有状态通知，不建立永久的通知相等过滤。
- 每帧继续读取真实原生边界、字形原点、光标和 caret；缓存不承担输入状态。`logical_rows`、`visual_rows` 及不可变源行对应表使用共享只读元数据，避免帧间克隆整份数组。
- `GroupState::measured` 区分源相对行几何与屏幕平移。滚动只发布实际输入投影；实际行高、换行或折叠计划改变时才构建对齐布局，不因完成一帧滚动通知所有来源再次绘制。
- `row_backgrounds` 使用按原生实体登记的弱几何租约，覆盖普通组合及源所有者的原生 factory。读取者只在当前布局匹配、组有效时提供行元数据；过期、独立模式或租约撤销立即回退原生查询。背景、选中、行号槽仍使用原生绘制，没有自行实现换行或文字编辑器。
- 比较的同步、唯一右侧轨道、两侧等宽、四个箭头、折叠按钮、整行／行号槽底色和独立插件边界均保留。本次没有公开协议或 SDK 接口变更；实际包仍使用既有 Git 0.1.21。

调试配置下的原始规模复验（几何缓存与底色复用已落地）：

| 场景 | 修复后实际结果 | 日志 |
| --- | --- | --- |
| 1800 行、8 轮 | 每轮 539–556 毫秒；逐行重新测量 0、对齐重建 0，较原始约 4.67 秒减少约 88% | `performance-debug-final.log` |
| 1800 行、2 轮、258 处彩色改动行 | 每轮 742–747 毫秒；逐行重新测量 0、对齐重建 0、底色原生行查询 0，较第一次修复后约 3.75 秒再减少约 80% | 同上 |
| 120 行、2 轮、彩色段缩小回归 | 通过；底色原生行查询从 2,160 降为 0 | `marked-minimized-green.log` |

这些是相同 `test` 未优化配置下整个滚轮分发与后续绘制的对照，不将其冒充 Release 单帧耗时。缓存命中仍使用少量原生起点／EOF 校验；计数 0 指不再为每个逻辑行和每个底色标记逐一查询，不代表不再调用原生布局接口。

之后补充了几何租约的立即撤销：旧元素即使保留回调，所有者撤销后也不能再获得行元数据。最终源使用优化测试程序重新执行全部相关行为及压力场景。

## 最终验证

优化测试程序为 `optimized-tests.exe`，实际命令为：

```powershell
cargo test -p editor-app --release long_diff_scroll --no-run --jobs 2 --config 'profile.release.package.editor-app.opt-level=1'
```

该配置仅将含千余测试的应用包设为优化级别 1，其余依赖使用 Release 默认设置；候选主程序使用普通 Release 默认设置，不沿用此覆盖。首次默认 O3 测试构建也已成功，但最终行为、优化计时均基于包含立即撤销保护的上述程序，不借用旧二进制冒充最终验证。扩展压力夹具首轮出现过测试读借用跨可变调用的编译错误，修正句柄克隆后已重新构建并通过。

| 最终入口 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --check` | 退出 0 | `fmt-last.log` |
| `git diff --check` | 退出 0；保留既有 CRLF 转换提示 | `diffcheck-last.log` |
| `cargo test --workspace --exclude editor-app --jobs 2` | 82 组，332 通过、324 ignored、0 失败 | `nonui-final.log` |
| `cargo check --workspace --jobs 2` | 退出 0，保留既有 warning | `check-final.log` |
| `ui::controls::aligned_ editor::comparison::render_tests:: sdk_export::tests:: ui::plugin:: tests::preview_tabs:: --test-threads=1` | 130 通过、25 ignored、0 失败；2.90 秒 | `ui-optimized-final.log` |
| `ui::controls::aligned_ code_sync::alignment_tests --ignored --skip long_diff_scroll --skip long_marked_diff --test-threads=1` | 显式 20/20；4.15 秒；包含原生滚动／选择／指针、中文 IME、Undo、三栏及独立窗口接缝 | `native-input-optimized-final.log` |
| `long_diff_scroll long_marked_diff --ignored --nocapture --test-threads=1` | 显式 2/2；1800 行普通场景每轮 89.9–92.3 毫秒，258 处改动行场景 117.4–119.7 毫秒；逐行测量／重建／底色重复查询均为 0 | `performance-optimized-final.log` |
| `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs git_inline_history_preview_opens_readonly_two_and_three_pane_tabs --ignored --test-threads=1` | 同一既有独立 0.1.21 ZIP 经公开 Manager，显式 2/2；23.06 秒；双／三栏、两主题、折叠、唯一右侧轨道、文件导航、源草稿与撤销、资源清理 | `real-package-optimized-final.log` |

上述选定普通测试里的 ignored 项没有计为通过；后三行是显式执行记录。原生几何失效回归直接检查当前原生行坐标，覆盖换行开关、字号／主题、无源通知的内边距调整及 150% DPI；原有不同列宽／行号位数、缩放、折叠、窗口原点和静止不持续绘制的行为也在选定测试中通过。

计时用于 CPU 侧组件对照，不代表用户显卡的实际帧率，也没有把优化配置数字与未优化基线计算百分比。没有追加系统 IME 候选窗的人工验收。源字节／行数、比较工作量及布局高度的既有预算没有扩大；超限仍由原来的原生独立呈现接管。首次打开和真正布局／内容变化仍需冷测量，不使用旧坐标覆盖新文档。

## 额外 Windows 整流程检查的既有失败

显式执行 `extensions::worker::native_acceptance::git_native_workbench_smoke --exact --ignored --nocapture --test-threads=1`，使用同一个独立 0.1.21 ZIP、隔离项目／用户目录及实际 Windows D3D11 窗口。当前测试程序在本地／远程树及真实分支过滤之后，等待 `all` 按钮超时，54.32 秒；对应截图显示正常文档及 incoming 分支，尚未打开差异比较。日志为 `native-smoke.log` 及 `native-smoke/dark-zh-CN-100/native.log`。

没有将这个失败算作通过，也没有因此修改本任务以外的工具栏、分支业务或原生验收流程。现行 Git 界面已经按用户要求移除工具栏，“全部分支”进入分支区菜单；但 [原生分支验收](../../../crates/editor-app/src/extensions/worker/native_acceptance/scenario/branches.rs)仍执行 `h.click_button(h.main, "all", ...)`。

严格对照使用本轮生产修改前、只含行测量插桩的 `timing-tests.exe`，同一包、同一 case 同样停在 `all timed out`，场景修订号同为 69，107.22 秒；日志 `native-smoke-perf-baseline.log` 及 `native-smoke-perf-baseline/dark-zh-CN-100/native.log`。更早的冻结测试程序也在初始原生 ready 条件失败，见 `native-smoke-baseline.log`；它不是上述同版对照，未用来代替严格基线。失败后的自有窗口、插件进程均走原有回收路径。本次不宣称完整原生工作流或八组合矩阵通过。

## 候选与身份

候选位于 [diff-scroll-2026-10-21](../../../target/diff-scroll-2026-10-21/)，主程序为 `Nanobug.exe`。构建入口：

```powershell
cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/diff-scroll-2026-10-21/Nanobug.exe
```

普通 Release 构建退出 0，用时 2 分 15 秒，日志 `release-build.log`。程序大小 78,048,256 字节，SHA-256 为 `13f78ea96e5f74ebee28026060f69aa6fb5c57129a0f169c7fcdcb30b18cfddc`。

已将 `cargo metadata --no-deps --format-version 1` 结果保存为 `cargo-metadata.json`，核对 workspace 与 target 分别是当前 `Editor` 与其 `target`。构建前后 1,354 项选定输入的路径及摘要完全一致，账本为 `host-build-inputs-before.json`、`host-build-inputs-after.json`。程序来自 `main` 的 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 上的既有脏工作区，不声明只包含本次性能修改；本轮控件增量相对于开工快照另存 `task-changes.patch`，没有把此前改动当成本轮成果。

`identity.json`、`SHA256SUMS` 与目录说明记录程序、验证配置及既有例外。依赖树确认 Kit／Base 0.7.0、无 gpui-component；没有上游副本或 fork。独立 Git 包没有修改或重新发布；验证使用的 0.1.21 ZIP 摘要仍为 `dce2c4b27edd8f83ea251790801e988274f486ae3b2a4b7e751f7c996e7482f9`。用户既有布局图摘要仍为 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6`。

未替换用户正在运行的程序、自动安装插件、重启、提交或推送。

返回 [UI 目录](../README.md)。
