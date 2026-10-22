# 比较视图原生换行映射与滚动峰值

本记录继续[上一阶段](scroll-validation-folds-2026-10-22.md)的偶发校验慢帧优化。目标是在保留原始 `EditorState` 编辑、中文 IME、选择和定位权威的前提下，减少滚动中的隐藏完整来源布局。

状态：2026-10-22 的两轮六场景硬件矩阵、三个额外压力场景及相关输入回归已通过；本机滚动平均帧率接近 144 Hz。首次原生准备仍有明显开销，见末尾边界。未提交、推送或替换已安装程序。

## 依据与实现

同一台 144 Hz Windows 设备上，旧版本有焦点的两栏长行用例复现 25.228 ms 最大绘制间隔、20.224 ms 最大子树耗时；只移除焦点则为 8.803／2.888 ms。临时分阶段探针确认：未知原生通知后的完整来源布局约 13–21 ms，行范围查询另占约 4–6 ms。调整批次大小不能稳定消除峰值；增大预热批次还会恶化密集文本装饰的耗时，因此没有保留这类调参和临时探针。

正式发布的 [gpui-base 0.8.0](https://crates.io/crates/gpui-base/0.8.0) 提供只读 `display_map()`，已直接核对 crates.io 发布包的 `src/input/base/state.rs`。本轮明确锁定 Kit／Base `=0.8.0` 和配套 GPUI `=0.3.9`，继续关闭 Kit 默认功能，依赖树与锁文件均不引入 `gpui-component`。Kit 0.8 的兼容命名空间指向 Kit 本身，宿主测试入口显式采用相同 crate 别名，消除嵌套测试的宏歧义。

`editor-ui/aligned_editor/geometry_display_map.rs` 在本帧最后一个原生输入层完成绘制后，读取该实体已经准备好的逻辑行／显示行映射。只保存不可变像素行元数据，不维护第二份可变文本、选择、输入状态或撤销历史。未知原生通知仍然撤销缓存；这轮重新校验不再需要完整来源的隐藏字形布局，也不再需要保活完整来源字形。

快速路径的前提是实际存在软换行、没有原生折叠、非遮罩、左对齐，且行数、文本长度、完整连续覆盖和高度上限都通过检查。实际换行证明上游处于受限横向宽度模式，包括 inline token。未换行、右／中对齐、遮罩或原生折叠保留原有测量回退，避免猜测不可公开读取的原生横向范围。窗口宽度或字体改变后必须以本帧原生 Map 重新判断；屏幕外精确定位保留原生完整几何路径。

首次来源先进行一次有界原生布局，再选择 Map 或分批测量，避免尚未知道换行模式就铺开全部来源。回退按“有界探测 → 分批测量 → 完整布局 → 明确失败”有限升级。深处关闭换行导致内容高度缩短时，以 Base 本帧夹紧后的实际纵向偏移抵消坐标，避免把旧请求与实际位置的差额误当作顶部留白；已有字体／DPI／深处换行切换回归先复现了该错误，再验证修复。

## 验证方法

所有原始输出保存在本地 `target/scroll-peaks-2026-10-22/`，该目录不属于交付源码。最终硬件矩阵统一使用 `matrix-native.exe`，由 `cargo test -p editor-ui --release --no-run --jobs 2` 生成的测试程序复制固定，避免共享工作区的后续构建替换被测程序。其 SHA-256 为 `D04AE94EB8679AF5DB17D54500F939109DD0699C350DD7A1A74D866EA2E7B419`。

设备为 Windows、NVIDIA GeForce RTX 5080、3840 × 2160／144 Hz，刷新率由 `Win32_VideoController` 读回。测试创建独立的真实 Windows／D3D 窗口，使用 1800 行原始 `EditorState`，从逻辑行 1200 开始，焦点位于可编辑来源；常规矩阵其余栏为只读快照。所有场景都有中文、真实差异和整行底色，长行另含 emoji；收起场景连续跨越多个未改动区段。每次测量同时断言惯性尾段实际继续移动、最后停止，以及来源文本保持不变。

FPS 和最大间隔来自实际前台移动绘制帧的完成时间，不按 20 ms 观察定时器的触发次数计算。正式门槛为平均至少 140 FPS、最大移动帧间隔不超过 20 ms、至少 100 个前台移动帧，并限制最终原生输入层的行数。最终十五次运行的移动帧全部处于前台。这里测量应用绘制节奏，没有使用外部显示器扫描或 GPU 呈现追踪。

本轮还修正了测试探针的“子树耗时”口径：分别累加 `request_layout`、`prepaint`、`paint` 调用的耗时，排除阶段之间的空闲时间；整窗布局求解、调度及呈现仍由实际帧间隔反映。这些分阶段耗时不能与旧探针的跨阶段总时长直接比较；FPS 与最大绘制间隔的算法未变。修正只影响测试统计。

## 最终硬件矩阵

每个场景连续两轮，日志为 `matrix-r1-<case>.log` 和 `matrix-r2-<case>.log`。FPS 列为两轮范围，其余耗时为两轮中的较大值。十二次运行全部通过上述门槛。

| 场景／日志 case | 平均 FPS | 最大帧间隔 ms | 阶段耗时 p95／最大 ms | 最终原生输入层最多逻辑行 |
| --- | ---: | ---: | ---: | ---: |
| 普通两栏／`plain-two` | 143.99–144.03 | 16.354 | 1.579／2.173 | 15 |
| 普通三栏／`plain-three` | 144.00–144.04 | 15.971 | 2.483／3.240 | 15 |
| 长行两栏／`wrapped-two` | 143.96–143.99 | 8.682 | 1.469／2.125 | 6 |
| 长行三栏／`wrapped-three` | 144.05 | 8.338 | 2.708／3.350 | 6 |
| 跨收起区两栏／`collapsed-two` | 143.98–144.02 | 12.001 | 1.897／5.743 | 39 |
| 跨收起区三栏／`collapsed-three` | 143.99–144.01 | 13.254 | 3.286／7.807 | 39 |

三个额外压力场景各运行一次，日志为 `matrix-stress-<case>.log`，均通过相同门槛。密集文本装饰在原始来源中增加粗体与颜色，检验字形准备成本；最后一项将三个来源全部设为可编辑。

| 场景／日志 case | 平均 FPS | 最大帧间隔 ms | 阶段耗时 p95／最大 ms | 最终原生输入层最多逻辑行 |
| --- | ---: | ---: | ---: | ---: |
| 密集装饰两栏／`dense-two` | 144.06 | 8.293 | 1.505／1.999 | 6 |
| 密集装饰三栏／`dense-three` | 144.08 | 8.970 | 2.946／3.580 | 6 |
| 全部可编辑、密集装饰、跨收起区三栏／`editable-collapsed-three` | 144.02 | 13.200 | 3.128／8.013 | 39 |

所有硬件日志的 `complete_source_frames` 为 0。此计数仅观察最后的原始输入层，不能据此宣称所有隐藏测量均为零。换行 Map 路径另有逻辑计数回归，证明冷准入、未知通知、整窗刷新和深滚动没有隐藏测量批次或逐行字形查询；普通短行没有实际换行，仍使用既有测量回退。跨收起区的 39 行是可见片段之间的完整原生包络。

## 编辑、输入与定位回归

新增回归检查换行来源的深滚动、未知通知与整窗刷新均不计划隐藏批次，也不逐行查询屏幕外字形；原生逻辑行范围、可见字形和 EOF 坐标必须一致。关闭换行后检查真实横向范围与实际横向滚动，再开启换行检查返回零隐藏测量路径；遮罩切换明确拒绝快速路径。

已有深视口与收起视口回归覆盖屏幕外字节定位、光标实际绘制、点击命中、中文及 emoji 预编辑、UTF-16 范围查询、提交与撤销；跨收起区点击和拖选覆盖隐藏来源字节。字体、主题、内边距、DPI、窗口宽度及深处关闭换行后的原生坐标检查也通过。宿主 SourceFrame 使用平台输入接口，验证原有 `DocumentSession`、补全锚点与历史；对齐宿主回归保留原始来源实体并检查三栏动作／滚轮与 IME 期间的动作失效。

输入回归由 GPUI 测试平台驱动真实原生控件和宿主输入代理。它们没有人工切换 Windows 输入法或操作系统候选窗，因此不把这些结果写成系统候选窗的人工端到端验收。

| 检查 | 实际结果及日志 |
| --- | --- |
| `cargo fmt --check` | 通过；`fmt-final-matrix.log` |
| `cargo test --workspace --exclude editor-app --jobs 2` | 通过；其中 `editor-ui` 为 108 通过、11 忽略；`workspace-tests-final-matrix.log` |
| `cargo check --workspace --jobs 2` | 通过；保留宿主现有 79 条警告；`workspace-check-final-matrix.log` |
| `cargo test -p editor-app comparison --jobs 2 -- --test-threads=1` | 13 通过、7 忽略；`app-comparison-final-matrix.log` |
| `cargo test -p editor-app source_frame --jobs 2 -- --ignored --nocapture --test-threads=1` | 显式 2/2 通过；`source-frame-final-matrix.log` |
| 显式对齐输入、IME、三栏滚轮、来源退休与插入行回归 | 5/5 通过；`native-input-final-matrix.log` |
| 最终固定程序的原生指针回归 | 显式 6/6 通过；`pointer-final-matrix.log` |
| 最终固定程序的 1800 行普通／带底色滚动回归 | 显式 2/2 通过，暖帧约 0.97–1.23 ms，逐行查询、对齐重建及底色查询均为 0；`long-final-matrix.log` |
| `git diff --check` | 通过；`diff-check-final-matrix.log`；新增文件另检查空白和相对链接 |

常规命令不会执行 ignored 测试；表中显式运行项单独计数，未运行的 Package／WASM 用例仍为忽略。对齐输入的本次入口为 `cargo test -p editor-app native_code_alignment --jobs 2 -- --ignored --nocapture --test-threads=1 --skip gestures:: --skip workbench::`，未把其他手势与独立工作台用例计入本次结果。固定程序的过滤词分别为 `aligned_pointer::tests` 和 `aligned_editor::tests::performance`，均带 `--ignored --nocapture --test-threads=1`。

## 复现硬件检查

先运行 Release 编译入口并从 Cargo 输出取得实际测试 EXE。下面以三栏收起场景为例，门槛只作用于验收，不改变生产限帧策略。切换到普通场景时需移除 `WRAPPED`、`COLLAPSED`；切换到长行未收起场景时只移除 `COLLAPSED`。增加 `DECORATED` 可复现密集装饰，移除 `READONLY_PEERS` 可检查全部可编辑。

```powershell
$buildRecords = cargo test -p editor-ui --release --no-run --jobs 2 --message-format=json
if ($LASTEXITCODE -ne 0) { throw 'Release test build failed' }
$nativeExe = ($buildRecords | ConvertFrom-Json | Where-Object {
    $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'editor_ui' -and
    $_.profile.test -and $_.executable
} | Select-Object -Last 1).executable
if (-not $nativeExe) { throw 'Missing editor-ui test executable' }
Remove-Item -LiteralPath Env:\NANOBUG_DIFF_NATIVE_DECORATED -ErrorAction SilentlyContinue
$env:NANOBUG_SCROLL_SURFACE = 'comparison'
$env:NANOBUG_DIFF_NATIVE_PANES = '3'
$env:NANOBUG_DIFF_NATIVE_ROW = '1200'
$env:NANOBUG_DIFF_NATIVE_FOCUSED = '1'
$env:NANOBUG_DIFF_NATIVE_DIFFERENT = '1'
$env:NANOBUG_DIFF_NATIVE_MARKED = '1'
$env:NANOBUG_DIFF_NATIVE_READONLY_PEERS = '1'
$env:NANOBUG_DIFF_NATIVE_WRAPPED = '1'
$env:NANOBUG_DIFF_NATIVE_COLLAPSED = '1'
$env:NANOBUG_SCROLL_MIN_FPS = '140'
$env:NANOBUG_SCROLL_MAX_FRAME_MS = '20'
& $nativeExe smooth_scroll_real_windows_paints_intermediate_native_positions --ignored --nocapture --test-threads=1
```

布尔开关按环境变量是否存在判断。关闭开关必须使用 `Remove-Item -LiteralPath Env:\变量名`；将值设为空字符串也会开启该选项。场景实际开关在每个日志开头读回。

## 保留的失败记录与边界

- 早期矩阵清理环境变量时留下空值，导致场景全部开启换行、收起与装饰；记录保存在 `env-probe-*.log`，未计入正式矩阵。其后部分 `final-*.log` 被另一原生验收窗口抢走前台，无法满足至少 100 个前台移动帧，也未计入正式结果。没有降低门槛；最终两轮在同一固定 EXE 上重新完整执行。
- 早期完整 Release 控件套件为 89 通过、19 失败、11 忽略，保存在 `ui-verified.log`。失败用例依赖 `debug_selector`，GPUI 0.3.9 的依赖 Release 构建不保留该测试选择器。正式常规控件验收采用默认测试 profile，108 项全部通过；Release 用于硬件帧率及不依赖选择器的显式回归。不能把早期 Release 全套结果标为通过。
- 首次原生准备仍慢：普通两栏约 295–298 ms、三栏约 449–469 ms；长行／收起两栏约 1.12–1.31 秒、三栏约 1.68–1.72 秒。这是滚动采样前实际原生调用的最大阶段耗时，修正探针后仍能复现；不计入暖滚动成绩，也未宣称已解决。起始行改为 0 时仍可复现约 1.13 秒，因此不能仅归因于深处定位。
- 精确屏幕外定位及不满足 Map 条件的来源仍可走完整几何路径；本轮保留其正确性。上述结果覆盖本机有限测量窗口，不代表所有设备都达到 144 FPS，也不保证每帧都低于 144 Hz 的 6.94 ms 预算。最终矩阵未复现此前的 30 ms 滚动峰值，但普通短行仍有约 16 ms 间隔，收起场景仍有约 13 ms 间隔。

返回[原生 UI 目录](../README.md)。
