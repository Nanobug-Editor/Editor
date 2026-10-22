# Git 面板隔离与原生帧滚动

日期：2026-10-22。状态：已实现并通过下列回归与 Windows 原生验收；未提交、推送、安装到用户配置或发布。

## 问题与改动

原实现存在六处相互影响：

1. 大多数点击重发提交、历史、冲突和提交详情四个面板，撤销原来的输入 revision。
2. 共用忙碌状态，操作一个面板会使另一个面板的浏览控件失效。
3. 普通点击及提交区选文件取消历史查询。
4. 暂存／取消暂存与提交共用全量刷新，重新查询历史。
5. 焦点、显示与保存事件进入主动刷新。
6. 共用比较任务和反馈，另一面板的选择、错误或弹窗操作影响当前任务。

滚动的独立根因是：原生组件已经推进偏移，图形仍等 WASM 返回绘图后才移动。先前局部发布和有界绘图没有解除这一等待。

- 已发布文档按面板比较语义内容，相同时保留原 revision 和输入权限。
- 普通提交区控件不取消历史查询。跨面板文件点击保留各自最新选择，当前比较完成后再执行；错误取消和详情弹窗按所属面板处理。
- 暂存/取消暂存只刷新状态与操作标记，焦点和可见性使用静默观察；真实提交、检出等仓库变化仍同步受影响数据。
- 新增公开 `ui.canvas 1.3` 可选绘图缓冲。原生帧直接平移保留的图形；迟到回执不回退位置，指针映射回相应绘图。Git 预绘上下各一视口，绘图量仍受视口与协议限额约束。详见[规格](../specs/canvas-retained-scroll.md)。

## 复现及证据

证据目录：`C:/Projects/RustProjects/Editor/target/git-panel-isolation-2026-10-22/`。

- `panel-red.log`：编辑提交信息返回四个面板而非一个；打开提交详情使历史查询队列由 1 变为 0。
- `scroll-red.log`：真实 GPUI 帧中滚动 17 像素后，内容仍在 y=135，预期 y=118。测试刻意延迟插件回复，无计时随机阈值。
- `scroll-tests.log`：3 项原生画布测试通过，包含连续 7ms 帧移动、迟到回包及点击位置、滚动条范围和惯性退役。7ms 为测试时钟推进，不代表实机屏幕 FPS 测量。
- `guest-tests.log`：独立插件 155 项测试通过，包含六项面板隔离回归。
- `integration-isolation-final.log`：最终包通过真实 Git 索引验证；草稿、历史选择、暂存和取消暂存均保留另一面板的 revision，草稿不丢失。
- `integration-history.log`：真实 8193 条历史经 65 页加载；128 次滚动累计 896px，视口 280px 时绘图少于 160 项、面板序列化小于 64 KiB；尾部提交可正确选中。该次通过后仅补上弹窗关闭归属保护，最终插件单元测试及隔离验收覆盖该保护。
- `check-results.json`：`cargo fmt --check`、`cargo test --workspace --exclude editor-app --jobs 2`、`cargo check --workspace --jobs 2` 均退出 0。workspace 测试合计 514 通过、378 ignored；ignored 不计作通过，相关实际包和原生用例另行显式执行。
- `guest-fmt-cargo.log`：独立插件 `cargo fmt --manifest-path ... --check` 通过。宿主 `--plugin-cargo` 仅支持 build/check/test，曾尝试的 fmt 被明确拒绝，随后改用直接 Cargo 格式检查；未绕过 SDK 构建入口。
- `diff-check.log`：`git -c core.safecrlf=false diff --check` 通过。编译存在既有 warning，未为本次修复扩大清理范围。

### Windows 原生行为

使用最终包，两个工作流各运行深色中文 100% 与浅色英文 150% 两个隔离进程；操作真实 HWND、原生输入和 Git 索引，并从 D3D11 渲染目标读取截图。

| 工作流 | 结果 | 证据 |
| --- | --- | --- |
| 提交窗口与历史隔离 | 两组合通过；编辑草稿不改历史 revision，点击历史提交不改提交面板 revision；目录索引锁恢复、部分暂存、整文件暂存／取消暂存及中文多行草稿保留通过 | `native-commit-final.log`、`native-commit-final/<case>/evidence.json` |
| 历史滚动及滚动条 | 两组合通过；32 次 5px 输入得到 160px，反向边界得到 25px，选择保持；滚动条拖动及宽度收缩中间帧、隐藏像素验收通过 | `native-scroll.log`、`native-scroll/<case>/evidence.json` |

四份最终原生证据均记录候选包 SHA-256 `be6fdc4bcd84a63872c6f05867464d05ab85e30a2f5ec4eaa15d2b5c6f9fe404`，窗口／资源退役成功，没有非预期错误日志。首次提交窗口验收的旧候选记录保留在 `native-commit/`，交付依据为最终复跑 `native-commit-final/`。

自动化帧测试验证绘制不等待插件回包，原生验收验证输入、布局和实际可见像素；未测量用户高刷新率显示器的稳定 FPS，也未把 `publication_ms` 当作帧率。中文输入验收不包含操作系统 IME 候选窗。

## 构建与验证命令

```powershell
cargo build -p editor-app --release --jobs 2
& target/git-panel-isolation-2026-10-22/Nanobug.exe --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib
& target/git-panel-isolation-2026-10-22/Nanobug.exe --plugin-package C:/Projects/RustProjects/Git-for-Nanobug --output target/git-panel-isolation-2026-10-22/packages
cargo fmt --manifest-path C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml --check
cargo test -p editor-app scrollbar_canvas -- --nocapture
cargo fmt --check
cargo test --workspace --exclude editor-app --jobs 2
cargo check --workspace --jobs 2
```

实际包测试设置 `NANOBUG_GIT_CANDIDATE` 后，在 `git_plugin` 测试二进制显式执行 `git_package_panels_keep_sibling_scene_during_draft_history_and_index_operations` 与 `git_package_large_history_keeps_scroll_scenes_bounded` 的 `--ignored --nocapture --test-threads=1` 用例。

原生验收先编译 `editor-app` 测试二进制并复制为证据目录中的 `native.exe`；设置 `NANOBUG_GIT_CANDIDATE`、`NANOBUG_GIT_NATIVE_OUTPUT` 与以下互斥工作流变量，然后执行对应测试的 `--exact --ignored --nocapture --test-threads=1`：

- `NANOBUG_GIT_NATIVE_COMMIT_PANEL=1`：`extensions::worker::native_acceptance::commit_panel::git_native_commit_panel`。
- 移除上述变量，设置 `NANOBUG_GIT_NATIVE_HISTORY_ONLY=1`、`NANOBUG_GIT_NATIVE_SCROLL_ONLY=1`：`extensions::worker::native_acceptance::git_native_history_scroll_probe`。

原生子进程使用临时配置和仓库，不操作用户仓库、窗口或系统 DPI。

## 交付文件

| 文件 | SHA-256 |
| --- | --- |
| `target/git-panel-isolation-2026-10-22/Nanobug.exe` | `35a71074592cd10933cd5adb9509f261a05b66c5612fcf6ea5f823ad12375b59` |
| `target/git-panel-isolation-2026-10-22/packages/git-0.1.29.zip` | `be6fdc4bcd84a63872c6f05867464d05ab85e30a2f5ec4eaa15d2b5c6f9fe404` |

相同插件包已复制到 `C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.29.zip`。

候选为 Git `0.1.29` 与配套宿主 `Nanobug.exe`，需一起使用。权限不变，旧宿主因缺少 `ui.canvas 1.3` 明确拒绝新包。共享工作区原有约 471 个文件改动不归入本次成果。
