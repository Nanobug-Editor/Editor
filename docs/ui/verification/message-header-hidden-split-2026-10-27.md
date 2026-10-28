# 消息顶部隐藏停靠项分隔线修复

日期：2026-10-27。用户截图指出“消息”标题上方边框偏粗。

## 原因与范围

当前项目保存的右侧布局为纵向 `[Problems, HostMessages]`，其中 Problems 隐藏。Base 按保留的存储索引给 HostMessages 添加前置 resize handle，因此在窗口标题栏底线下面多画了一行实心线。

这与 [2026-10-26 的标题条抗锯齿修复](message-severity-appearance-2026-10-26.md) 不同；旧修复仍存在，原有回归也通过。旧测试只排除了标题的边框 quad，未覆盖隐藏前驱产生的实心分隔线。

本次在本地 `LocalDockRenderer` 的公开绘制接缝过滤内部 split 分隔线：paint 阶段核对当前可见兄弟的完整共享边界，只有实际相邻的可见面板之间才画线。Base 的持久树、隐藏项、尺寸、命中区、拖动状态及活动配色均保留。外围 dock 边线和标题分隔线未改动。

## 回归与验证

新增 `host_messages_hidden_predecessor_has_no_top_split_separator`，通过真实主窗口构造 Editor 与右侧 `[Problems(hidden), HostMessages]` 嵌套布局。原实现稳定失败于额外实心顶线断言（0.17 秒）；修复后通过普通横向与纵向分隔线、Problems 显隐切换、浅色 100%、深色 150%、浅色 200% 检查。

显式 `1px` 分隔线在 GPUI 0.3.10 的 150% 布局中先按半值向零取整为 1 个设备像素。正断言采用 `window.pixel_snap(px(1.)) * scale`，没有放宽生产边界匹配，也没有将测试中的错误取整预期当成产品缺陷。定向诊断已移除。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test -p editor-app host_messages_ --locked --offline -j 4 -- --nocapture --test-threads=1` | 14 通过，0 失败 |
| `cargo test --workspace --exclude editor-app --locked --offline -j 4` | 831 通过，0 失败，441 按既有标记忽略 |
| `cargo check --workspace --locked --offline -j 4` | 通过 |
| `cargo rustc -p editor-app --release --bin editor-app --locked --offline -j 4 -- -o C:/Projects/RustProjects/Editor/target/message-border-followup-20261027/Nanobug.exe` | 通过，2 分 20 秒 |

验证日志保存在本机 `target/message-border-*.log`。检查中的既有编译及链接警告不作为新增失败；忽略的测试不计为通过。Release 候选与正常退出后更新的 `target/release/editor-app.exe` SHA-256 均为 `15CD4E4420DFC9A563BBCD6CFD848B5A2DBB6D141FD9EC26BF795E5614D9B2D6`。

## 正式运行文件实机复查

用 Computer Use 核对原窗口没有未保存标记后正常退出，再更新正式路径的 EXE。首次通过后台 Hidden 启动的进程没有显示窗口；仅停止本次启动的该进程，改由原生应用启动入口恢复可见窗口。当前正常运行的正式进程为 `57820`。

在用户原有中文浅色 100% 配置与保存布局上，消息列表仍显示相同的“本地历史未能更新”警告，消息标题上方已与其他列保持单条细线。真实鼠标点击收起消息，再由底部消息按钮重开，顶边仍无额外分隔条；右侧竖线、标题底线、其他区域分隔线保留。深色与额外缩放由上述绘制回归覆盖，未额外声称全部主题均完成桌面交互。

工作区：`C:/Projects/RustProjects/Editor`，`main`。沿用当前修复任务的保留改动、不自动提交选择，并保留已有插件与语言服务修复。

关联：[宿主消息规格](../specs/host-messages.md)、[原生 UI 目录](../README.md)。
