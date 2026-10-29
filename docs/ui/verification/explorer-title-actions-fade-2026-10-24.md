# 资源管理器标题按钮的焦点淡入淡出

日期：2026-10-24。用户从 HTML 示意中选择方案 1：资源管理器面板获得焦点时显示标题栏四个操作按钮，焦点离开时以约 140 ms 的淡出隐藏；重新获得焦点时反向淡入。

2026-10-28 增加目录／文件筛选按钮后，当前按钮组为五个紧凑按钮，预留槽位扩展为 120 × 24 逻辑像素；焦点淡入淡出保持原有行为。筛选交互和本次验证见[资源管理器名称筛选](explorer-filter-2026-10-28.md)。以下四按钮尺寸和验收数量保留为当时记录。

## 行为

- 使用 Base 的目标值过渡和默认 ease-out 曲线，只改变按钮组透明度。连续切换焦点时从当前透明度继续，遵循系统的减少动画设置。
- 标题右侧固定保留四个紧凑按钮的 96 × 24 逻辑像素槽位，标题文字和周边布局不随焦点移动。完全透明时不再生成按钮节点，避免不可见按钮接收输入或进入可访问性树。
- 面板标题保持独立的稳定定位标识；布局原生验收脚本据此寻找资源管理器，避免把失焦的隐藏按钮误判为面板消失。

## 验证

- `cargo test -p editor-app explorer_title_actions_fade_with_panel_focus -j 1`：1 项通过，覆盖初始隐藏、获得／离开焦点后的过渡、固定槽位及减少动画。
- `cargo test -p editor-app explorer::interaction::tests -j 1`：41 项通过，包含定位当前文件和展开／折叠目录的标题按钮点击。
- `cargo test --workspace --exclude editor-app -j 1`：555 项通过、402 项按原有标记忽略，0 失败；未将忽略项计作通过。
- `cargo check --workspace -j 1`、`cargo fmt --check`、`git diff --check`：通过。
- `cargo build -p editor-app -j 1`：通过。在隔离 profile 和临时项目启动实际 Windows 窗口；初始编辑器聚焦截图可见资源管理器标题而四个操作按钮隐藏。随后桌面收到其他输入，自动点击被窗口控制工具中止；没有据此声称完成原生聚焦／失焦过程和深浅主题的目视验收。临时实例已关闭。
- 上述 Cargo 命令使用独立的 `target/explorer-actions-fade` 输出目录，已用 `cargo metadata --no-deps --format-version 1` 核对工作区与目标目录。

返回 [UI 文档目录](../README.md)。
