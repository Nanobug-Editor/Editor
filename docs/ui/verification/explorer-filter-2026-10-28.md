# 资源管理器名称筛选

日期：2026-10-28。状态：已实现；原工作区的应用回归、原生验收及 workspace 检查结果见下文。

2026-10-29 集成：实现保存为提交 `30ee38cb`，保留统一标题按钮的焦点／悬停行为并汇入本地主分支。本次按用户要求只做 Git 差异、冲突处理和提交归属检查，不重跑构建、测试或原生验收；下文的通过结果来自原工作区。

## 行为与边界

- 目录树上方使用本地 `Input::small()`，框高 24 逻辑像素；小尺寸行高限定为 22 逻辑像素，避免继承大字号的行高后使 Base 原生输入区超出边框。复用 Base 的单行文本、焦点、选择和 IME 状态，保留中英文提示及深浅主题。
- 默认隐藏。标题按钮组增加搜索图标；Ctrl+F 在资源管理器面板获得焦点或鼠标悬停在其标题／内容区域时切换显示。在编辑器仍有焦点但鼠标位于资源管理器时，优先处理这一次 Ctrl+F，避免同时打开文档查找。
- 按目录和文件名执行大小写不敏感的子串筛选。匹配项的祖先目录临时展开；匹配目录时保留其子项；空目录可匹配。项目根节点始终保留为标题，其名称不使所有子项匹配。
- 数据来自现有、已应用忽略／排除规则的工作区树。输入不触发文件扫描；后台文件列表变化继续应用当前筛选。
- 关闭后清空输入、恢复原来的展开状态；临时展开／折叠不写入会话。单击或双击文件后自动隐藏，再按既有预览／固定标签规则打开文件；单击目录保留筛选。
- Escape 关闭并返回树焦点；输入框内按向下进入结果树，选中文件后 Enter 打开并隐藏筛选。
- 标题保留五个紧凑按钮的 120 × 24 逻辑像素槽位，沿用统一的焦点或悬停淡入淡出。

## 实现与验证入口

实现位于 `crates/editor-app/src/explorer/filter.rs`；本地输入尺寸接口位于 `crates/editor-ui/src/input/mod.rs`。名称匹配和实际树交互回归分别位于 `explorer/filter/tests.rs`、`explorer/interaction/tests/filter.rs`。

独立 worktree：`C:/Users/Tmiracle/.codex/worktrees/explorer-filter/Editor`；分支 `codex/explorer-filter`。已用 `cargo metadata --no-deps --format-version 1 --locked --offline` 核对该工作区自身的 `target/`，无目标目录覆盖。

```powershell
cargo test -p editor-app explorer --locked --offline -j 8 -- --test-threads=1
cargo test -p editor-app shortcuts --locked --offline -j 8 -- --test-threads=1
cargo fmt --check
cargo test --workspace --exclude editor-app --locked --offline -j 1
cargo check --workspace --locked --offline -j 1
git diff --check
```

原生 Windows 验收显式执行忽略用例：

```powershell
$env:NANOBUG_EXPLORER_FILTER_OUTPUT = '<项目外的本机验收目录>'
cargo test -p editor-app explorer_filter_native_windows_acceptance --locked --offline -j 1 -- --ignored --nocapture --test-threads=1
```

该用例打开隔离会话的真实 Windows/D3D 窗口，检查深浅主题、14／20 像素字号、原生键盘与指针事件、输入框布局、中文 IME handler 的组合／提交及实际文件打开，并输出截图。组合输入验证直接使用原生编辑端点的公开 handler，不代替真实安装的中文输入法候选窗人工验收。父用例同时检查子进程自然退出码；其他忽略用例不计作通过。

## 实际结果与证据

| 检查 | 结果 |
| --- | --- |
| `cargo test -p editor-app explorer --locked --offline -j 8 -- --test-threads=1` | 74 passed，0 failed，3 ignored |
| 同上过滤 `shortcuts`，`-j 1` | 24 passed，0 failed，5 ignored |
| 最终源码过滤 `explorer_filter`，`-j 1` | 5 passed，0 failed，2 ignored；原生父用例另外显式执行 |
| 显式原生 Windows 验收，`-j 1 -- --ignored --nocapture --test-threads=1` | 父用例 1 passed；两种主题 × 两档字号均通过，子进程正常退出 |
| `cargo fmt --check`、`git diff --check`、修改文档的相对链接 | 通过 |
| `cargo test --workspace --exclude editor-app --locked --offline -j 1` | 127 组，848 passed，0 failed，445 ignored；忽略项不计为通过 |
| `cargo check --workspace --locked --offline -j 1` | 通过，保留已有警告 |

首轮 workspace 测试以 `-j 8` 构建时，编译器在未修改的 `editor-terminal` 报告 `memory allocation of 4718608 bytes failed` 并退出。保留失败日志，使用同一工作区、同一 `target/` 降为 `-j 1` 重试通过；没有为规避资源失败修改终端源码、依赖、profile 或测试断言。

项目外证据目录：`C:/Users/Tmiracle/.codex/visualizations/2026/10/28/01a182e2-6386-7ab2-bd5d-8b17110573ee/`。应用回归日志为 `explorer-tests.log`、`shortcuts-tests.log`，最终源码的针对性复验为 `explorer-filter-tests-final.log`；原生父用例日志为 `explorer-filter-native-run.log`，子进程日志、通过记录及四张主题／字号截图位于 `explorer-filter-native/`。workspace 首轮与重试分别保存为 `workspace-tests.log`、`workspace-tests-retry.log`，编译检查为 `workspace-check.log`。

返回 [UI 文档目录](../README.md)。
