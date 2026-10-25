# 编辑器文件标签浮起留位拖动

日期：2026-10-23。用户从交互原型中选择“浮起留位”，本次范围为编辑区域上方的文件标签排序。

同日后续扩展为编辑区四边和通用停靠窗口，参见[标签显示方式](editor-tab-placement-2026-10-23.md)。本文保留首阶段的顶部标签验收记录。

## 行为

- 越过原生拖动阈值后，标签按原抓取位置跟随指针，向上浮起 8 个逻辑像素，保留原宽度、文件图标、状态文字和预览标记。
- 原标签隐藏，目标位置显示等宽浅色空槽，跨过目标的邻项通过上游弹簧动画让位。拖动中不改变文档的真实排列；松手后才提交一次有效移动。
- 支持向前、向后和末尾落点；指针接近标签栏左右边缘时按显示刷新间隔自动滚动，最大速度为每秒 600 个逻辑像素。
- 在栏外松手、按 Escape、窗口失活或可见标签成员／顺序变化时取消。取消后的松手不会意外激活、关闭标签。
- 保留单击激活、双击固定预览、关闭、中键关闭、右键菜单和原有滚动定位。移动现有文档所有者，保持活动文档、编辑内容及 Undo 状态。
- 拖动中源标题或修改标记变化时冻结其实际布局宽度，结束后恢复自适应，避免空槽与邻项错位。

## 代码边界

本地通用控件位于 `crates/editor-ui/src/tabs/dragging/`，仅使用 Kit 原语和 Base 标签行为。控件持有派生几何和手势，不持有文档排序；宿主接收稳定文件身份及目标后继身份，复核可见性后提交。

`crates/editor-app/src/editor/tabs/view.rs` 从原 `editor/view.rs` 提取标签呈现，连接 `FileTabStrip` 与 `reorder_file_tab`。原滚动句柄及直接子项索引保持一致，比较标签保留为不可排序的尾随内容。

浮层图标与源标签共享不可变 `ImageSource`，保留原有全彩图标。Escape 从拥有焦点路径的宿主祖先捕获，确保焦点在编辑器正文时也可取消。浮层释放通过代次标识退休手势，避免旧浮层误清理后续拖动。

## 验证记录

相关回归与原生交互已完成。workspace 编译通过；完整测试与格式检查存在下述范围外失败，未将其标为全通过。

- `cargo test -p editor-ui tabs::dragging -- --test-threads=1`：6 项通过。覆盖浮层／空槽原宽、邻项实际动画位移、释放提交、Escape／栏外／失活／成员变化取消、标题变宽以及滚动索引和边缘滚动。
- `cargo test -p editor-app tab_drag_order -- --test-threads=1`：6 项通过，包含实际鼠标事件以及文档、Undo、预览和持久化不变量。
- `cargo test -p editor-app preview_tabs -- --test-threads=1`：10 项通过，包含中文 IME、焦点、标题宽度、滚动定位与重启恢复。
- `cargo build -p editor-app`：通过，存在其他模块和 Wasmtime／Tree-sitter 链接警告。
- `cargo check --workspace --jobs 2`：通过。
- `cargo test --workspace --exclude editor-app --jobs 2`：退出码 101。停止前 487 项通过、1 项失败、253 项忽略；其中 `editor-ui` 为 128 项通过、12 项忽略。失败为 `plugin-runtime/tests/language_declarative.rs:239` 的 `declarative_language_plan_needs_no_empty_wasm_and_retires_original_authority`：`language.lsp` 实际 `1.8.0`、断言仍预期 `1.7.0`。该 crate 不依赖本次标签 UI，相关语言准备代码已有其他并行任务改动；本次未修改这项契约或断言。失败后的测试程序及被忽略测试未报告为通过。
- 本次文件的定向 `rustfmt --check --edition 2024 --config skip_children=true`、`git diff --check` 通过。完整 `cargo fmt --check` 未通过，最终差异位于其他并行任务的 `plugin-runtime/src/manager/activation/{language_preparation,roles}.rs` 和 `plugin-runtime/tests/maven_package/` 等文件；未代为改动。
- 源码、Cargo.lock 和 `cargo tree --workspace --prefix none` 检索未发现组件运行时依赖；源码中的名称命中仅为既有禁止依赖断言。

上游 spring 使用原生 `Instant`，测试执行器的模拟时钟只推进定时器。动画测试因此先绘制目标，再经过 80ms 真实时间检查邻项已移动，不以“推进模拟时钟”代替动画结果，也不要求固定时间完全收敛。

审查发现并修复了“拖动中源标题变宽导致留位错位”，已添加真实 GPUI 布局回归测试。复核确认源 Tab 与包装布局同步冻结宽度，且原高度及取消后自适应行为保留。

## Windows 原生验收

使用本次 `target/debug/editor-app.exe`、独立测试项目和独立 `--profile`，通过原生鼠标／键盘操作验证。测试窗口完成后正常关闭，用户原有窗口未改动。

| 场景 | 可见结果 |
| --- | --- |
| 浅色主题把 `main.rs` 拖到空白末尾，再拖回首位 | 排列分别变为 `tabs.rs / theme.rs / main.rs`、`main.rs / tabs.rs / theme.rs`；当前正文一直为 `theme.rs` |
| 把文件标签拖到正文区域后松手 | 原排列恢复，正文不切换 |
| 单击中文文件后拖到首位 | 文件名及“预览”标记正常，移动后仍为预览 |
| 移动后双击中文标签 | “预览”标记移除，标签恢复固定文件的自适应宽度 |
| 点击另一个标签的关闭按钮 | 对应标签关闭，当前中文正文保留 |
| 切到深色主题，再将中文标签拖到末尾 | 排列正确，主题、中文标题和选中状态正常 |
| 关闭测试窗口并读回独立会话 | 保存为 `main.rs / theme.rs / 中文预览文件.txt`，活动文件仍为中文文件 |

原生工具观测的是释放后的窗口结果；浮层、空槽及邻项动画的中间几何由 GPUI 测试覆盖。本次未测量实际帧率，也未宣称多显示器／全部 DPI 组合验收。

返回[原生 UI 目录](../README.md)。
