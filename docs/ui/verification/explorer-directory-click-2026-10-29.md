# 资源管理器目录点击方式

日期：2026-10-29。状态：功能与本轮验收通过。

资源管理器目录默认单击展开或折叠；“设置 → 外观与行为 → 布局 → 目录展开/折叠方式”提供“单击”和“双击”两个选项。选择立即作用于当前工作区，并随其本机会话保存；旧会话缺少该字段时同样采用单击。目录箭头始终单击切换，文件保持单击预览、双击固定标签的行为。

实现复用现有 `TreeState`、目录切换及递归折叠逻辑。仅在配置对应的点击次数上切换，单击模式忽略快速双击的第二次点击，避免立即恢复原展开状态；行回调消费点击，防止上游 Tree 的默认切换再次执行。选择仍在按下时发生，展开等待完整点击，因此拖动不误触展开。

本任务工作区：`C:/Users/Tmiracle/.codex/worktrees/explorer-directory-click/Editor`；任务分支：`codex/explorer-directory-click-20261029`。构建使用工作区自身 `target/`，证据位于项目外 `C:/Users/Tmiracle/.codex/task-evidence/explorer-directory-click-20261029/`。

## 验证

| 检查 | 结果 |
| --- | --- |
| `cargo test -p editor-app directory_click_mode --locked --offline -j8 -- --test-threads=1` | 3 passed，覆盖默认/旧会话、保存重载、深浅主题、点击次数、根目录、空目录、箭头与方向键 |
| `cargo test -p editor-app explorer --locked --offline -j8 -- --test-threads=1` | 最终 77 passed、0 failed、3 ignored；包含单击预览/双击固定、拖放和筛选交互 |
| `cargo test -p editor-app settings_dialog_tests --locked --offline -j8 -- --test-threads=1` | 2 passed，使用原生设置对话框中的真实按钮切换并读回保存值 |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked --offline -j8` | 127 组，864 passed、0 failed、446 ignored |
| `cargo check --workspace --locked --offline -j8` | 通过，保留既有警告 |
| `cargo build -p editor-app --locked --offline -j8` | Debug 原生程序构建通过；用于下面的真实 Windows 窗口验收 |

首次测试支架误用了 TreeState 未公开的焦点访问器，编译失败后改为观察编辑器失焦和实际方向键响应。首轮资源管理器为 76 passed、1 failed、3 ignored：筛选测试仍假设点击目录只选中，未重新展开递归折叠后的内层目录；更新此场景后最终全部通过。未放宽生产行为或断言。

## Windows 原生交互

使用独立 profile 和临时受限工作区，实际操作目录标签、箭头、设置按钮与键盘，查看 Windows/D3D 的可见结果：

- 默认单击展开/折叠；快速双击只切换一次，编辑区保持原状。
- 设置选择“双击”后，单击仅选中，双击展开，箭头仍可单击折叠。
- 深浅主题的标签和两项选择均清晰可见，选中态与实际值一致。
- 关闭后重新启动同一工作区，设置仍为“双击”；单击目录仍仅选中。
- 设置改回“单击”后立即可展开；左方向键仍可折叠。读回保存值为 `false`，正常退出码为 0。

项目外证据：`native-settings-dark-double.jpg`、`native-settings-light-single.jpg`、`native-settings-restarted-double.jpg`、`native-acceptance.json` 与 `native-exit.json`。首次隐藏窗口启动没有形成可见交互，已清理其自有进程，不计为通过；有效验收使用后续可见窗口。验收程序 SHA-256：`180b56e4dd29aee2fb8ecfc9476ab2c515db9998443805b363cb9891b28ebe23`。

未执行或忽略的检查不计为通过。按用户选择执行普通任务提交和隔离集成验证后，本地合并结果保存在 Git common directory 的 `task-worktree/tasks/explorer-directory-click-20261029.json` 及项目外集成日志；无推送或发布操作。

返回[原生 UI 目录](../README.md)。
