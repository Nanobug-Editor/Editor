# 问题面板默认右侧与首次布局

日期：2026-10-24。用户要求问题面板首次停靠右侧，宽度约 360px；未手动调整的初始布局按左侧、底部、右侧的顺序分配空间。

## 当前实现

- 没有保存的 `DockArea` 布局时，左侧沿用独立全高区域；编辑区与右侧窗口列位于同一上方行，底部区域横跨左侧以外的全部宽度。右侧列仅占底部上方。问题和宿主消息在同一右侧列内上下排列，默认隐藏状态分别保留。
- 首次显示问题面板时，右侧列使用 360 个逻辑像素；用户保存过的宽度优先。新出现的右侧插件窗口加入现有右侧列，避免再建立一条全高的外侧列。
- 保存的布局树仍按原位置与尺寸加载。旧会话若缺少问题面板节点，补入右侧区域，不移动已经存在的其他面板。

## 验证

- `cargo test -p editor-app native_problems`：7 通过。新鲜工作区的原生布局几何、约 360px 右列、底部跨度，以及实际指针拖动后的尺寸与位置恢复均通过。
- `cargo test -p editor-ui initial_right_contribution_joins_center_column -j 1`：1 通过；新的通用右侧窗口加入中心右列，底部区域仍保留。
- `cargo test -p editor-app dock`：61 通过、1 失败、3 忽略。失败的 `plugin_dock_layout_survives_delayed_startup` 预期树缺少当前进行中的 OpenFiles 面板；实际恢复树补入该面板，与本次右侧默认布局无关。
- `cargo test --workspace --exclude editor-app -j 1`、`cargo check --workspace -j 1`、`cargo fmt --check` 和本次改动的 `git diff --check` 通过。工作区测试第一次按默认并发执行时遇到 Windows 页文件不足（OS 1455），单线程重试通过。
- 验证使用独立的 `target/problems-right-default` Cargo 输出目录；没有执行完整 `editor-app` 测试或单独的人工窗口验收。

返回 [UI 文档目录](../README.md)。
