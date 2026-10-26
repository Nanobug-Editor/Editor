# 警告提示点与消息标题分隔线修正

日期：2026-10-26。依据：用户提供“本地历史未能更新”的警告截图，并指出消息圆点看起来是红色、消息窗口顶部边框偏粗。

## 诊断依据

- 截图圆点的高饱和像素约为 `#ea580c`，采样色相约 20.1°，与原 Warning 配色一致；不是 Error 使用的 `#dc2626`。原先的警告橙偏红，小圆点与危险色难以辨认。此前 Release 验收只证明了等级映射，并未充分检查这种视觉差异。
- 停靠标题使用 `.border_b_1()`。实际 GPUI 绘制记录包含标题顶端的边框抗锯齿片段，即使 top border 为零，仍加重了其与窗口标题分隔线相邻的边缘。
- 通过现有宿主发布入口与主窗口绘制接缝检查：警告色断言失败；标题顶部边缘回归在修复前稳定失败，单次约 0.15 秒。没有以隐藏错误、旧 EXE 或缓存作为未经证实的原因。

## 修改

- Warning 提醒色改为浅色 `#f59e0b`、深色 `#fbbf24`；Info 蓝色、Error 红色与最高未确认等级规则保持原语义。底栏与溢出入口使用同一颜色解析函数。
- 所有本地停靠标题通过共用 `LocalTabGroupRenderer` 绘制单条 1 逻辑像素实心底线；保留标题高度、底部占位、标题内容与原有拖动接缝，避免整框边缘抗锯齿造成额外顶线。
- 更新现有颜色回归、Windows 原生颜色验收预期，追加主窗口标题绘制回归。覆盖深浅主题与缩放。

## 验证状态

已完成以下检查：

| 命令／入口 | 结果 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo test -p editor-app host_messages_ --jobs 2 --no-fail-fast -- --nocapture --test-threads=1` | 13 通过 |
| `cargo test --workspace --exclude editor-app --jobs 2` | 782 通过，409 按既有标记忽略，0 失败 |
| `cargo check --workspace --jobs 2` | 通过 |
| `tests::window_toolbar::native::window_toolbar_native_windows_acceptance --exact --ignored --nocapture --test-threads=1` | Windows/D3D11 原生验收通过，浅色 100%／深色 150% 的三色提醒、点击与长按拖动均通过 |
| `cargo build -p editor-app --release --jobs 2` | 用户关闭占用窗口后重跑通过，3 分 10 秒 |

标题绘制回归覆盖浅色 100%、深色 150%、浅色 200%。150% 下分别取整分隔线两端，允许 1 逻辑像素自然覆盖 1 或 2 个物理像素，没有把取整误判为加粗。额外顶部边框片段必须不存在。

原生测试是测试 EXE 内的 `Window::dispatch_event` 输入；下述正式 Release 实测另用 Computer Use 的真实桌面鼠标／键盘完成。构建保留既有 unused 与链接器警告。

## 正式 Release 桌面验证

首次构建完成链接后，Cargo 因正在运行的原程序而无法替换 `target/release/editor-app.exe`（拒绝访问，os error 5）。先用新链接产物的隔离副本验证；用户明确回复“已保存并关闭”后，重复同一个 Cargo 构建入口成功。随后直接运行正式路径的 EXE，再完成相同交互验证，没有把前一个副本的验证代替正式文件实测。

正式 EXE：`C:/Projects/RustProjects/Editor/target/release/editor-app.exe`。

- 时间：2026-10-26 11:43:15（Asia/Shanghai）；大小：89,112,576 字节。
- SHA-256：`CE24203D691250F266D75E4AE74838C4A3DB16AE23EFB24397105B1FF7DBCBAC`。
- 使用前轮已由用户操作过的隔离 profile 和 `severity-probe.txt`，没有改动信任设置或用户源文件。
- 启动历史只有一条 Info；收起再打开消息窗口确认该提醒，核对圆点消失。
- 保持消息面板打开，在临时文本键入 `x` 并点击文件标签关闭按钮。关闭保护发布“关闭此标签页前请先保存更改”Warning；唯一新增未读等级是 Warning，圆点清晰显示橙色，右侧列表同时可见 Warning 与已确认的旧 Info，没有 Error。
- 同一画面中，消息与资源管理器的标题顶端保持一致，没有原先多出的深色边缘；标题底部分隔线保留。
- 撤销临时字符并保存；文本前后 SHA-256 均为 `BCDDF1286F5F95F3C2B208173CE365EB23D4A6169441DE087197C71E532FCCF0`。仅停止本轮已核对路径、profile 和 PID 的诊断进程。

正式文件的实测截图为本机证据目录中的 `canonical-warning-and-header.png`；`canonical-editor-app.exe` 保存同哈希产物。`release-warning-and-header.png` 属于此前的临时 Release 副本，原生测试截图保存在 `native/`。桌面颜色结论来自实际画面观察，精确颜色值由绘制回归与原生测试的真实 quad 断言覆盖。

工作区为用户要求继续使用的 `C:/Projects/RustProjects/Editor`、`main` 分支（起点 `ffd9efbe5176a23d6caf4424c83903f6d409f64e`），保留原有其他任务的未提交改动。未自行提交或推送。

本机证据目录：`target/message-severity-visual-2026-10-26/`；首次失败及后续构建日志保留在 `target/message-severity-visual-*.log`。

关联：[消息窗口规格](../specs/host-messages.md)、[前轮 Release 实测](message-severity-release-2026-10-25.md)、[原生 UI 目录](../README.md)。
