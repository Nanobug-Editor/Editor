# 差异文件切换计时与会话保存去重

日期：2026-10-29。状态：本阶段实现、回归、workspace 检查及两轮原生对比测量通过；审阅后用户追加授权合并至本地 main，实际提交与合并结果记录在 Git 和本机任务元数据中。

## 范围与验收依据

用户确认先完成计时和减少重复保存，并用同样文件比较修改前后的切换速度。本轮只处理会话保存及对应观测；后续的首次布局、高亮准备、比较结果缓存和相邻文件预取分别验收。

- 基线：`42a5e76a1d329bdfc59e828a6d9c1a36d45405c1`，开始时主工作区没有未提交修改。
- 工作区：`C:/Users/Tmiracle/.codex/worktrees/diff-switch-timing-save/Editor`，分支 `codex/diff-switch-timing-save-20261029`。
- 用户先选择独立 worktree、保留结果审阅，随后明确要求合并至主分支并更新最新插件；授权本次普通提交与本地 main 合并，不包含推送。
- 会话验收：持久化内容不变时不重复替换文件；真正变化仍可恢复，保存失败保留错误和重试能力，损坏数据保护及退出保存继续有效。
- 导航验收：真实 Git 包经普通文件树及前后文件按钮打开精确来源，计时到两侧正确文本已绘制且原生几何可用；同一套计时程序比较保存去重前后。

## 设计与测量

会话的统一 `try_save` 入口记录请求、真实写入、跳过、失败及总耗时。正常诊断使用 `nanobug::performance` tracing target，不记录工作区内容；测试线程的计数仅供本次原生观测，不成为插件接口。

保存去重比较当前序列化快照与已保存文件。相同快照直接完成；变化继续使用临时文件、磁盘同步与原子替换。比较读取须有大小限制，不引入可能覆盖新状态的后台写入队列。

原生计时入口 `extensions::worker::native_acceptance::git_native_diff_switch_timing_probe` 使用真实 Windows/D3D、生产 Worker、已安装独立 Git ZIP，以及独立的 TOML grammar 包识别 `.novel` 文件。每组先打开 `a.novel`，然后计量两次内容缓存未命中及 32 次缓存命中导航，覆盖前一文件、下一文件和手动树选择。

主题/缩放为深色中文 100% 与浅色英文 150%。首次启动、WASM 准备、截图与证据写入均在计时区间之外。每次记录点击到正确来源绘制的墙钟时间，以及该区间的保存次数和耗时。使用两行文本隔离本阶段目标，结果不代表大文件首次排版速度。

每次正确显示后保留 200ms 阅读间隔，间隔不计入点击延迟。这是独立浏览操作的测量，不是连续点击吞吐或取消正确性验收。就绪条件仍检查两侧精确文本与实际原生几何，不能以这段间隔代替就绪检查。

证据目录：`C:/Users/Tmiracle/.codex/visualizations/2026/10/29/01a18258-60da-76c0-8e33-72186a603e4d/diff-switch-save/`。所有构建与测试使用本 worktree 的 `target/`。

## 结果

有效运行顺序为 `baseline-4 → optimized-1 → baseline-5 → optimized-2`，采样期间没有并行编译。每个运行包含两组原生窗口、68 次切换；两版各有 128 个已读内容缓存命中样本及 8 个首次访问样本。全部对应 `evidence.json` 的 `shutdown_complete=true`、工作流范围和 Git 包摘要已核对。

| 已读内容缓存命中，128 次/版 | 去重前 | 去重后 |
| --- | ---: | ---: |
| 点击到正确来源绘制，中位数 | 308.631 ms | 94.0209 ms |
| 点击延迟 p95（最近秩） | 678.2302 ms | 249.4245 ms |
| 每次切换保存阶段，中位数 | 212.0935 ms | 0.2935 ms |
| 保存请求合计 | 384 | 384 |
| 实际原子替换合计 | 384 | 0 |
| 保存失败 | 0 | 0 |

该组中位切换耗时降低 69.54%，每次重复写盘由 3 次降为 0 次。两轮分组值如下，保留磁盘等待的实际波动，不用单次最好值作结论。

| 轮次及场景 | 去重前切换中位数 | 去重后切换中位数 |
| --- | ---: | ---: |
| 第一轮，深色中文 100% | 285.3053 ms | 91.39935 ms |
| 第一轮，浅色英文 150% | 360.3838 ms | 95.147 ms |
| 第二轮，深色中文 100% | 329.5324 ms | 91.95715 ms |
| 第二轮，浅色英文 150% | 274.15005 ms | 91.24585 ms |

首次访问仅 8 个样本/版：中位数为 202.4856 → 159.0976 ms；单组样本少、Git 进程及磁盘等待波动大，不据此给任意文件的首次打开保证。完整保存计数包括首次访问时为 408 → 0 次实际写盘。

本机小文件样本说明此阶段有效，但未达到此前候选的缓存命中 p95 ≤50ms 目标；剩余初始化、绘制和调度仍需另行分阶段测量。

原始样本及汇总：证据目录的 `baseline-{4,5}/<case>/diff-switch-timing.json`、`optimized-{1,2}/<case>/diff-switch-timing.json` 和 `comparison-results.json`。已查看最终深浅主题截图，确认两侧文字、着色、差异底色及当前文件选择准确。

初期测量入口的失败保留：`baseline-native-1.log` 未成功进入下一文件；`baseline-native-2.log` 的子场景完成但父级缺少独立截图契约；`baseline-native-3.log` 在较密集连续输入中超时。这些运行均不计作通过，也不进入汇总。更正原生指针移动、独立证据契约和明确的浏览间隔后，最终四次完整运行通过；本轮未宣称解决连点问题。

## 回归与门禁

- Red：`session-red.log` 显式执行一项测试，旧实现重写相同内容，固定文件时间断言失败。
- Green：`session-green-build.log` 中同一回归一项通过；原子同步写入与损坏数据保护保留。
- 最终原生对比：`baseline-native-4.log`、`optimized-native-1.log`、`baseline-5-run.log`、`optimized-2-run.log` 均为 1 passed / 0 failed / 0 ignored，每个父用例检查两组独立 Windows 场景及退出收据。
- `cargo test -p editor-app --bin editor-app --locked --offline -j8 app::session::tests:: -- --nocapture --test-threads=1`：12 passed / 0 failed / 0 ignored；含相同内容不重写、同长度内容变化、删除后重建、失败重试及既有重启恢复。
- `cargo test -p editor-app --bin editor-app --locked --offline -j8 window_geometry -- --nocapture --test-threads=1`：3 passed / 0 failed / 0 ignored；含连续缩放推迟保存、不变尺寸不写盘、关闭时保存最新尺寸。
- `cargo fmt --check`、`git diff --check`：通过。
- `cargo test --workspace --exclude editor-app --locked --offline -j8`：退出 0，127 组汇总为 859 passed / 0 failed / 446 ignored。原有 opt-in WASM、原生窗口、父用例启动的子入口均按各自标记跳过，未报告为通过；本次 Git 原生计时另有上述四次显式执行。
- `cargo check --workspace --locked --offline -j8`：退出 0。构建日志有既有 unused/dead_code 等警告；未扩大范围修复无关警告。

审查覆盖相对起点的完整未提交差异及新增文件。规范、规格两条审查线均未发现可证实缺陷；因用户选择保留结果，没有为审查创建提交，也不将空的 `base...HEAD` 当作已覆盖工作区。

## 复现与产物身份

测量使用 Release 测试程序；Git ZIP 固定为 `C:/Projects/RustProjects/Editor/dist/git/git-0.1.48.zip`，SHA-256 为 `b51bc8a7397717a60a272b0a14d38ce646edf81e5e3a363f0dc5cf488a082e38`。证据目录保留相同观测代码的 `baseline.exe`、`optimized.exe`，前者仅未启用保存去重；基线源码和摘要在 `baseline-source/`、`baseline-identity.json`，最终汇总及程序摘要在 `comparison-results.json`。

在本 worktree 中重新测量当前实现，使用新的项目外输出目录，避免覆盖已验收证据：

```powershell
$env:NANOBUG_GIT_CANDIDATE = 'C:/Projects/RustProjects/Editor/dist/git/git-0.1.48.zip'
$env:NANOBUG_GIT_NATIVE_HISTORY_ONLY = '1'
$env:NANOBUG_GIT_NATIVE_DIFF_TIMING = '1'
$env:NANOBUG_GIT_NATIVE_OUTPUT = '<新的项目外证据目录>'
cargo test --release -p editor-app --bin editor-app --locked --offline -j8 extensions::worker::native_acceptance::git_native_diff_switch_timing_probe -- --exact --ignored --nocapture --test-threads=1
```

该入口构建后依次打开两组真实测试窗口，实际计时不包含编译。已有冻结程序也可用相同环境和 `--exact extensions::worker::native_acceptance::git_native_diff_switch_timing_probe --ignored --nocapture --test-threads=1` 直接运行，避免采样中启动编译。不要把不同 Git 包、Debug profile 或阅读间隔的结果混入本表。
