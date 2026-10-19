# 项目选择后的窗口居中

日期：2026-10-19。

项目激活此前仅调用 GPUI 的 `Window::resize`，恢复客户区尺寸时保留原生窗口左上角位置。现在 Windows 在项目成功激活时以一次 `SetWindowPos` 恢复尺寸，并将整个窗口居中到当前显示器的可用区域；计算包含原生边框，逻辑尺寸按当前缩放换算成设备像素。同步定位完成后通过 GPUI 公开的 `Window::bounds_changed` 入口更新视口、布局与命中范围。超过可用区域的尺寸会收缩，已最大化或最小化的窗口保留状态。无原生句柄的测试窗口及其他平台继续使用原有尺寸恢复路径。

实现入口为 `crates/editor-app/src/app/projects.rs`，本地窗口接口在 `crates/editor-app/src/ui/controls/window.rs`，Win32 适配在 `crates/platform-windows/src/window.rs`。不新增依赖，不改变会话格式或项目授权流程。

## 回归与原生验证

`crates/editor-app/src/tests/project_windows/geometry.rs` 使用独立 profile 和项目，在真实 GPUI/Windows 窗口中从偏离屏幕中心的 720×480 窗口打开已记录为受限的项目，恢复 900×600 客户区，再通过 `GetWindowRect`、`GetClientRect` 和 `GetMonitorInfoW` 检查实际尺寸与中心。子进程有 30 秒超时，并通过正常项目关闭入口退出。

```powershell
cargo test -p editor-app project_selection_centers_native_window -- --ignored --nocapture --test-threads=1
cargo test -p editor-app app::projects::tests -- --test-threads=1
```

- 修复前：客户区 900×600 正确，窗口中心偏移约 790×356 设备像素；测试 0.83 秒失败。日志：`target/project-window-center-red.log`。
- 修复后：原生回归 1 passed，0.85 秒；中心误差不超过 1 设备像素。日志：`target/project-window-center-green.log`。
- 项目选择与窗口交互：39 passed、0 failed、4 ignored。被忽略的原生几何测试已按上方命令单独执行；其余三个真实插件包场景未执行，不计为通过。日志：`target/project-window-center-project-tests.log`。

原生验证环境的缩放为 100%；未改变系统缩放，也未进行多显示器硬件验收。主题、IME 和项目选择的既有交互由上述项目测试覆盖；首轮原生几何测试仅验证窗口定位与尺寸，后续补充的视口和绘制检查见下文。

## 初始化后右侧与底部空白回归

首轮仅检查 HWND 尺寸与外框中心，遗漏了 GPUI 内部视口。补充测试发现，同步 `SetWindowPos` 的 `WM_SIZE` 回调尝试再次更新当前已借用的 GPUI 窗口，导致 GPUI 仍按旧尺寸布局；原生窗口已扩大时，右侧和底部留下空白。修复在原生调用结束后显式同步 GPUI 视口，不修改根布局尺寸规则，也不等待用户手动缩放窗口。

同一个原生回归现在同时断言：窗口外框居中、恢复后的客户区尺寸、GPUI 逻辑视口乘以缩放后的设备尺寸，以及实际 D3D11 绘制目标尺寸一致。检查前明确绘制当前帧，并支持通过 `NANOBUG_PROJECT_GEOMETRY_CAPTURE` 指定 PNG 截图路径。

- 修复前：原生客户区 900×600，GPUI 仍为 720×480；回归 0.91 秒失败。日志：`target/project-window-viewport-red.log`。
- 修复后：原生回归 1 passed，0.95 秒；视口和绘制目标均为 900×600，外框继续居中。日志：`target/project-window-viewport-green.log`。
- 实际 D3D11 截图：`target/project-window-viewport-green.png`；已目视检查标题栏延伸到右边缘、状态栏贴合底边，未出现旧尺寸之外的空白带。
- 项目交互复查：39 passed、0 failed、4 ignored，11.31 秒；日志 `target/project-window-viewport-project-tests.log`。原生几何测试已单独执行，其余忽略项仍未执行。

## 仓库检查

| 命令 | 结果 |
| --- | --- |
| `cargo test --workspace --exclude editor-app` | 空白修复后复查 exit0；78 组，304 passed、0 failed、311 ignored；日志 `target/project-window-viewport-workspace-tests.log` |
| `cargo check --workspace` | 空白修复后复查 exit0；日志 `target/project-window-viewport-workspace-check.log` |
| `cargo fmt --check` | exit1；仅 `crates/editor-app/src/ui/controls.rs` 中原有的 `diagnostic_popup` 重导出行有行尾空格；本次文件无格式差异 |
| `git diff --check` | 全局同样报告上述原有行尾空格；限定本次涉及文件的检查通过 |

保留已有未使用代码及 Windows 链接器警告，未顺手修改无关工作区内容。未执行的忽略测试不计入通过数量。

返回 [UI 文档目录](../README.md)。
