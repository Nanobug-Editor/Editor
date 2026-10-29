# 资源管理器与插件标题按钮显隐

日期：2026-10-24。用户从 HTML 示意中选择方案 1：资源管理器面板获得焦点时显示标题栏四个操作按钮，焦点离开时以约 140 ms 的淡出隐藏；重新获得焦点时反向淡入。

2026-10-28 增加目录／文件筛选按钮后，当前按钮组为五个紧凑按钮，预留槽位扩展为 120 × 24 逻辑像素；沿用面板焦点或鼠标移入时显示的统一淡入淡出规则。筛选交互和本次验证见[资源管理器名称筛选](explorer-filter-2026-10-28.md)。以下四按钮尺寸和验收数量保留为当时记录。

## 2026-10-28：鼠标移入与插件面板统一规则

本次用户明确扩展显示条件：面板拥有焦点，或鼠标位于其标题／内容区域时显示按钮；两者均离开后淡出。资源管理器四个操作按钮和全部插件面板的宿主标题按钮组采用同一规则。

- 本地 Dock 使用 Base 原有面板焦点和 GPUI 鼠标进入／离开事件，鼠标移入不抢夺编辑器焦点。
- `editor-ui::dock::title_actions` 统一保留 140 ms 可反向过渡、减少动画支持、固定槽位和完全隐藏后的输入／无障碍目标移除。
- 插件有菜单命令时保留 48 × 24 逻辑像素槽位；没有菜单命令时仅保留 24 × 24 的最小化按钮槽位。已打开的插件菜单继续保留其标题锚点。
- 没有扩展插件协议、能力或权限，也没有按具体插件 ID 分支；其他宿主标题控件仅适配本地标题接口，保持各自现有显示策略。

本轮检查在独立工作区 `C:/Users/Tmiracle/.codex/worktrees/panel-actions-hover/Editor` 执行，使用其自身的 `target/`；已通过 `cargo metadata --no-deps --format-version 1 --locked --offline` 核对根目录及目标目录，未覆盖 `CARGO_TARGET_DIR`。

| 检查 | 结果 |
| --- | --- |
| `cargo test -p editor-app title_actions --locked --offline -j8 -- --test-threads=1` | 5 项通过，覆盖标题／内容区域移入、焦点保留、固定槽位、无菜单布局及打开菜单锚点；另 1 项原生测试默认忽略，按下行显式执行 |
| 同上过滤 `explorer::interaction::tests` | 42 项通过 |
| 同上过滤 `extensions::dock_tests` | 56 项通过，包含上述插件显隐测试，不能将过滤组重复相加 |
| `cargo test --workspace --exclude editor-app --locked --offline -j8 --no-fail-fast` | 848 项通过、0 失败、445 项按现有标记忽略；忽略项不计为通过 |
| `cargo check --workspace --locked --offline -j8` | 通过，保留已有警告 |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test -p editor-app native_panel_title_actions_follow_focus_and_hover --locked --offline -j8 -- --ignored --nocapture --test-threads=1` | 1 项通过，内部两个独立 Windows/D3D11 场景均通过：深色中文 100%、浅色英文 150%；原始窗口全部关闭、子进程退出码 0 |

日志与原生截图集中保存到项目外 `C:/Users/Tmiracle/AppData/Local/Temp/nanobug-panel-actions-hover-20261028/`。原有共享侧栏全流程首次因未注册原生标题观察点停止；为标题补齐 Base `test_support` 后，第二次在 Git `stage` 按钮的可用／当前状态断言处停止，尚未触达标题显隐。该断言和 `native-sidebar/` 下的失败日志保留，不将原侧栏全流程计为通过。

独立标题场景使用 `NANOBUG_GIT_NATIVE_TITLE_ACTIONS=1`，`NANOBUG_GIT_CANDIDATE=C:/Projects/RustProjects/Editor/dist/git/git-0.1.48.zip`，`NANOBUG_GIT_NATIVE_OUTPUT` 指向上述证据目录内的 `native-title-actions/`。两个场景均确认资源管理器和实际 `git/staging` 面板：鼠标移入显示且编辑器焦点不变、鼠标移出但仍聚焦时保持显示、鼠标及焦点均离开后隐藏。已目视检查四张最终截图，按钮位置及显隐符合预期；两个 `evidence.json` 均为 `workflow=title-actions-only`、`shutdown_complete=true`，不能替代完整侧栏验收。

实际 Git 0.1.48 包的 SHA256 为 `b51bc8a7397717a60a272b0a14d38ce646edf81e5e3a363f0dc5cf488a082e38`；本次没有修改或重新打包插件。原生测试使用独立临时项目与 profile，仅对自有 HWND 请求 DPI；未修改系统缩放、用户配置或验证系统 IME 候选窗口。

以下为 2026-10-24 的历史结果。

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
