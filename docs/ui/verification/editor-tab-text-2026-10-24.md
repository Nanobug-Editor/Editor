# 文件标签文字下缘裁切修复

日期：2026-10-24。用户截图中 `plugin_development.rs`、`sdk_export.rs` 的下划线和字母下缘显示不全。

## 原因与修复

标签有足够的横向空间时仍可在独立原生窗口复现。Base 的中性 `Tab` 使用 `1.0` 倍行高，宿主标题使用 `truncate()`，因此超出标题行框的字形会被竖向裁切。Windows DirectWrite 的真实排版检查测得：默认文件标题框仅高 `12px`，同一文本的排版高度为 `16.293457px`。

本地共用 `file_tab` 改为 `1.5` 倍行高，为原生字体的上下缘留出空间。普通文件和比较标签共用此入口，编辑区四边与“已打开文件”窗口也沿用该修复；不修改上游依赖。既有最大宽度和省略、关闭按钮、选中边框及浮起留位拖动继续使用原接口。

## 回归检查

- `editor-ui` 的 `file_tab_title_clip_keeps_font_descenders` 从 Windows 原生平台取得 DirectWrite 字体引擎，在 GPUI 确定性测试窗口里比较实际标题框与排版行高度；覆盖普通文件、比较标签、深浅主题和 100%／125%／150% 缩放。
- 修复前运行 `cargo test -p editor-ui file_tab_title_clip_keeps_font_descenders --jobs 2 -- --test-threads=1`：按预期失败，提示 `title clip height 12px cuts a font line of 16.293457px`。普通无头测试字体不能复现该差异，因此该检查只在 Windows 编译，并使用实际原生排版结果。
- 宿主 `preview_tabs_titles_keep_descenders_inside_clip_bounds` 覆盖五种显示方式，核对标题处于所属标签内；已有长标题、省略和拖动测试继续复查。

## 检查结果

- `cargo test -p editor-ui tabs:: --jobs 2 -- --test-threads=1`：14 项通过，包括原生字体回归、两轴拖动与边缘滚动。
- `cargo test -p editor-app --bin editor-app --jobs 2 -- preview_tabs tab_drag_order --test-threads=1`：23 项通过，包含新增五种布局的标题几何，以及既有长名称、省略、预览、关闭和拖动检查。
- `cargo test --workspace --exclude editor-app --jobs 2`：通过，累计 551 项通过、401 项忽略；忽略项未作为通过项统计。
- `cargo fmt --check`、本次文件的定向格式检查、`cargo check --workspace --jobs 2` 和 `cargo build -p editor-app --jobs 2`：通过，构建保留既有编译及链接警告。
- `git diff --check`：通过。

## 原生复查

使用本轮 Debug 构建、独立项目和 profile；用户原有 Release 窗口未参与测试。修复前已在原生窗口看到文件名中间的下划线消失；修复后重新打开同一夹具，`plugin_development.rs` 与 `sdk_export.rs` 的下划线和字母下缘完整，活动及非活动标签均正常。测试文件通过资源管理器真实双击打开，验收后正常关闭独立窗口。

原生观察为本机浅色横向标签栏；深浅主题、五种显示方式和分数缩放由上述自动化检查覆盖，未改动 Windows 显示缩放设置。

返回[原生 UI 目录](../README.md)；相关布局说明见[编辑器标签显示方式](editor-tab-placement-2026-10-23.md)。
