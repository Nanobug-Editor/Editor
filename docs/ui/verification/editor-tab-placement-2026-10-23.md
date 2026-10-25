# 编辑器标签显示方式

日期：2026-10-23。用户确认五种互斥显示方式；“已打开文件”通用停靠窗口替代编辑区域标签栏，选择保存在用户设置中，对同一 profile 的所有项目生效。

## 行为与边界

- 设置 → 编辑器 → 文件标签显示方式：上方横栏、下方横栏、左侧列表、右侧列表、工具窗口列表；旧配置默认顶部。
- 顶部和底部为横向标签，左侧、右侧及通用窗口为竖向列表，文字保持横排。左右列表宽 200 个逻辑像素，并限制为编辑区域宽度的 40%。
- 所有模式复用打开文档、活动身份、预览状态、关闭、菜单、比较视图和排序回调，不复制文本、文档列表或撤销栈。
- 横向和竖向复用[浮起留位拖动](editor-tab-drag-2026-10-23.md)。切换显示方式取消旧手势，清理旧滚动偏移并定位活动文件。
- 工具窗口复用现有 Dock，标题和底栏按钮采用 Kit 的 `Files` 叠放文件页图标，图标显式打入本地资源。窗口可停靠、隐藏、通过底栏恢复；隐藏不关闭文件，位置和显隐意图仍归项目布局保存。
- 用户选择写入 profile 下 `editor.json`；原子保存成功后才通知同 profile 的项目窗口。不同 profile 隔离，空启动窗口也可修改。损坏或未知模式配置显示错误并保留原文件；其它未知设置字段保留。
- 文本、文件预览、比较视图和历史面板共用编辑区布局组合；选中标记面向正文，切换不重新创建文档。

## 验证

- `cargo test -p editor-ui tabs::dragging --jobs 2 -- --test-threads=1`：9 项通过，覆盖两轴浮层、空槽、弹簧让位、边缘滚动、取消和比较项索引。
- `cargo test -p editor-app tab_ --jobs 2 -- --test-threads=1`：26 项通过，包括四边布局、工具窗口、竖向指针排序、38 文件溢出后的活动文件定位、文件预览恢复按钮尺寸和历史面板焦点。
- `cargo test -p editor-app --bin editor-app --jobs 2 -- editor_preferences open_files preview_tabs window_toolbar --test-threads=1`：25 项通过、3 项忽略。覆盖配置保存及损坏保护、同 profile 多窗口同步、空窗口设置、停靠位置和显隐恢复、预览标签及底栏排序。忽略项需要独立 WASM 夹具或专用原生窗口环境，本次没有报告为通过。
- 最终合并复查：`cargo test -p editor-app --bin editor-app --jobs 2 -- tab_ editor_preferences open_files preview_tabs window_toolbar --test-threads=1`，52 项通过、3 项忽略；包含新增的固定标签顺序、活动身份和已移动停靠窗口重启回归。
- 加强活动文件断言后，`cargo test -p editor-app --bin editor-app open_files_restart_restores_fixed_order_active_file_and_moved_dock --jobs 2 -- --test-threads=1`：1 项通过。

切轴测试曾发现 GPUI 在更新滚动句柄的视口／方向之前消费 `scroll_to_item`。宿主现于 `Window::on_next_frame` 重新解析当前活动身份再定位，避免使用旧几何和过期索引。测试通过实际下一帧回调验证，没有只放宽可见性断言。

原生重启还发现启动期间恢复停靠布局会保存当前会话，在文档尚未恢复时覆盖待恢复标签。现在在布局初始化之前快照固定标签与活动文件，再按该快照恢复；回归测试覆盖真实保存、重新加载和新窗口构造，并确认预览标签不恢复。最终审查将活动文件设为恢复顺序的第一项，避免默认选中最后打开文件也能通过断言。

- `cargo check --workspace --jobs 2`：通过。
- `cargo build -p editor-app --jobs 2`：最终构建通过，使用此产物完成下述重启复查；保留既有编译及链接警告。
- `cargo test --workspace --exclude editor-app --jobs 2`：退出码 101，失败前累计 490 项通过、1 项失败、253 项忽略；其中 `editor-ui` 为 131 项通过、12 项忽略。范围外失败仍为 `plugin-runtime/tests/language_declarative.rs:239` 的 `language.lsp` 版本断言：实际 `1.8.0`、预期 `1.7.0`。该模块已有其它并行任务改动，本次未改其契约或断言。
- 不指定 `--bin editor-app` 的补充测试入口还触发了已有示例 `examples/svg_preview_render.rs:92` 的 `Paint::TextBox` 非穷尽匹配编译错误；指定实际应用测试目标后的上述相关测试通过，未把示例报告为通过。
- `cargo fmt --check`：范围外的 `plugin-runtime/src/manager/activation/{language_preparation,roles}.rs` 与 `tests/maven_package/` 等文件仍有格式差异，本次没有代改。
- 本次相关 Rust 文件的定向格式检查及 `git diff --check`：通过。

## Windows 原生验收

使用本轮构建、独立测试项目与 `--profile`，通过真实鼠标／键盘操作检查。用户原有 Release 窗口不参与测试。

| 场景 | 实际结果 |
| --- | --- |
| 从设置依次选择左、右、底部和工具窗口 | 文件列表移到对应边；正文、顺序和中文预览标记保持，工具窗口模式移除编辑区内标签栏 |
| 左侧竖排将首项拖到末尾，再拖中文预览到正文区域 | 有效拖动提交新顺序；栏外释放取消，活动正文仍为中文文件 |
| 隐藏“已打开文件”，点击底栏叠放文件页按钮恢复 | 文档未关闭，原列表和预览状态恢复；标题与底栏图标均可见 |
| 切为深色主题，工具窗口内重新排序 | 文件图标、窗口图标、中文预览和排序结果正常 |
| 将“已打开文件”标题拖到右侧 | 通过既有 Dock 移到右侧，原文件身份、顺序和正文保留 |
| 关闭最终构建窗口后重新打开相同项目与 profile | 两个固定文件按原顺序恢复，第二个文件继续为活动正文，工具窗口模式与右侧停靠位置恢复 |
| 设置页选中“工具窗口列表”后移开鼠标 | 正常前景色与浅色选中底保持可读 |

原生验收发现设置选中按钮叠加 primary 白字和 selected 浅色底，鼠标移开时对比度偏低；现保留标准 selected 语义与正常前景色，并已在原生窗口复查。验收后正常关闭独立测试窗口。

浮起／空槽／弹簧中间几何由 GPUI 测试覆盖，原生工具观测释放后的可见结果。本轮未测实际帧率或跨显示器全部 DPI 组合。

## 2026-10-24：预览图标

标签上的“预览”文字改为 14px 的淡色 `Eye` 图标。普通文件、比较标签及拖动浮层复用同一标记，悬停说明与无障碍名称继续使用中英文 `editor.preview_label`。图标随固定操作消失，原有单击预览、双击或编辑后固定的状态规则不变。图标通过本地图标资源入口显式嵌入。

后续按用户反馈将图标改为主题淡色前景的 60% 不透明度，预览与关闭控件通过共用尾部行以 2px 间距排列。文件图标容器和关闭控件显式居中，避免 14px 图片在 16px 容器内偏向上缘。当时仅核对了布局框中心，用户后续截图证实文字实际字形仍偏低；实际字形对齐结论由[后续光学修复与验收](editor-tab-optical-alignment-2026-10-24.md)替代。

- 图标替换后的 `cargo test -p editor-ui --jobs 2 -- tabs:: assets::tests::exposes_supplemental_action_icons --test-threads=1`：15 项通过，覆盖资源解析、标签拖动与字体布局。
- 当时样式的 `cargo test -p editor-app --bin editor-app --jobs 2 -- preview_tabs tab_drag_order --test-threads=1`：24 项通过。几何检查覆盖五种显示方式、深浅主题及 100%／125%／150% 缩放，要求各布局框中心偏差不超过 0.75px，预览和关闭控件间距不超过 2.1px。布局框检查不能证明可见文字居中，后续增加了原生字形像素回归。

- 最终 `cargo test --workspace --exclude editor-app --jobs 2`：551 项通过、401 项忽略；忽略项未作为通过项统计。初版检查曾遇到 Windows 对测试程序的文件占用，随后重试通过，最终样式的全仓检查也通过。
- 最终 `cargo check --workspace --jobs 2`、`cargo build -p editor-app --jobs 2` 及本次 Rust 文件的定向格式检查通过。全仓 `cargo fmt --check` 仍报告范围外 `editor-app/src/extensions/dock_tests/drag/live/exclusive.rs` 的格式差异，本次未代改。
- `git diff --check` 通过。

当时原生窗口中，单击资源管理器文件显示淡色眼睛图标，眼睛与关闭按钮紧凑排列。此前仅按布局框判断文字居中，这项结论在用户后续截图后撤回。其余行为观察仍保留：单击预览图标没有误触关闭；悬停显示“预览”，Windows 可访问树包含“图像 预览”；双击标签后图标消失且文件保留。使用独立受限项目与 profile，未操作用户原有 Release 窗口，验收后关闭独立窗口。原生观察为本机浅色横向布局，其余布局、主题和缩放由上述自动化检查覆盖。

返回[原生 UI 目录](../README.md)。
