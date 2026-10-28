# 编辑器滚动停止时的文字跳动

日期：2026-10-28。初始修复基于 main `7edaa2be`，任务分支为 `codex/editor-scroll-settle`。实现阶段检查结果见下文；后续按用户要求直接本地合并，不重复执行测试或构建。

## 现象与依据

用户动图显示，惯性滚动接近停止时，相邻文字行先后移动最后一个像素，行距短暂变化后恢复。逐帧检查确认这不是文本修改；只检查 `EditorState::scroll_offset()` 和 `text_bounds()` 无法捕获该显示问题，两者始终同步且没有反向移动。

锁定的 GPUI Pre 0.3.10 会分别将字形基线映射到物理像素。150% 缩放下，21 逻辑像素的行高对应 31.5 物理像素；连续的小数滚动位置使相邻行在不同帧跨过取整边界，尾段放大了这种视觉变化。

原生 Windows/D3D 回归通过真实滚轮事件驱动生产 `source_editor::Editor`，读取实际渲染图像。断言允许整段文本整体平移，但不允许不同行分别跳动。首轮相同断言在八个小数起点共检出 15 个异常尾段样本；添加物理像素对齐后，同一组输入为 0 个。日志分别为 `native-settle-row-red.log`、`native-settle-row-green.log`。

## 修复边界

- [平滑滚动呈现](../../../crates/editor-ui/src/scroll/smooth.rs)仅在原生编辑器和多行输入框的普通滚轮动画中，将竖向写入位置对齐到当前缩放的物理像素。两者通过既有 [native 入口](../../../crates/editor-ui/src/scroll/targets.rs)启用。
- 原有解析弹簧继续保存完整精度、速度和终点；`Motion` 记录实际写入位置，避免将像素对齐误判为外部定位。
- 停稳后再次滚动，只有原生位置仍等于本次最后写入值时，才沿用精确终点。回归曾捕获两次独立滚动在 150% 缩放下积累一物理像素偏差，现已消除。
- 触控板精确像素输入、水平滚动、外部定位、取消与原生边界裁剪继续使用既有路径。没有升级依赖、修改依赖缓存或新增文本／Undo 状态。

## 验证

所有命令在 `C:/Users/Tmiracle/.codex/worktrees/editor-scroll-settle/Editor` 执行。`cargo metadata --no-deps --format-version 1 --locked --offline` 已确认根目录与该 worktree 自身的 `target/`，没有 `CARGO_TARGET_DIR` 覆盖。

| 检查 | 实际结果 |
| --- | --- |
| `cargo test -p editor-ui scroll:: --locked --offline -j 6 -- --nocapture --test-threads=1` | 15 passed、0 failed、2 ignored |
| `cargo test -p editor-ui smooth_scroll_native_text_has_no_terminal_pixel_shift --locked --offline -j 6 -- --ignored --nocapture --test-threads=1` | 1 passed；8 个原生场景的异常尾段样本均为 0 |
| `cargo fmt --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked --offline -j 6 --no-fail-fast` | 127 组；850 passed、0 failed、446 ignored |
| `cargo check --workspace --locked --offline -j 6` | 通过；保留既有未使用项等警告 |
| `git diff --check` | 通过；两个文档共 85 个本地链接有效 |

[可控时钟回归](../../../crates/editor-ui/src/scroll/tests/inertia.rs)覆盖 144 Hz 步进、100%／125%／150%／200% 缩放、深浅主题、文本与输入坐标一致、停稳重绘，以及八次独立滚轮操作的总距离。既有测试同时验证反向滚动、精确设备输入、原生定位、边界、中断和有限帧请求。

[原生图像回归](../../../crates/editor-ui/src/scroll/tests/native_settle.rs)覆盖本机真实 150% 缩放、八个起点、深浅主题、上下滚动、颜色文本和中文回退字形，且要求实际捕获仍在移动的尾段。已查看导出的深浅主题停稳画面。此项使用 GPU 图像读回，不作为帧率或性能基准；其他 DPI 的图像绘制以及完整宿主 Release 未在本轮单独验收。未执行的其他 ignored 测试不计为通过。

本机证据集中保存在项目外 `C:/Users/Tmiracle/.codex/visualizations/2026/10/28/01a182e9-504a-7f72-84b6-41093f0087be/`，包含原始动图分析、红绿测试、`native-settle-final.log`、`scroll-regressions.log` 和完整检查日志。设置 `NANOBUG_SETTLE_CAPTURE` 为已有的项目外目录可导出原生场景的选定 PNG。

## 差异比较视图的后续补充

用户在首轮合并后反馈 Git 差异比较仍有相同偏移。代码追踪确认，比较视图沿用原生 `Editor` 负责文本和输入，但 [AlignmentPane](../../../crates/editor-ui/src/aligned_editor/pane.rs) 使用独立 `SmoothScroll::new(...).projected(line)` 驱动左右／三方共享的投影坐标；它没有经过首轮已启用像素对齐的 `targets::native` 入口。因此首轮普通编辑器的修复并未覆盖比较视图。

本次在共享投影视口启用同一个 `snap_vertical_to_pixels()`，所有源切片继续跟随同一偏移，不对左右源分别取整；滚轮距离余量和惯性精度沿用前述实现。对齐能力只扩展为 crate 内可复用，未改变 `external_scroll` 的通用画布行为。

按用户要求，本次补充仅核对调用链和 Git 差异，直接本地合并，**未执行测试、构建或原生交互验收**。上面的通过数字属于首轮代码，不能作为本次比较视图补充的实测结果。

返回 [UI 目录](../README.md)。
