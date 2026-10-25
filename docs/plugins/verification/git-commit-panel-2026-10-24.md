# Git 提交窗口选择与表单修复

日期：2026-10-24。状态：六项修复已实施，自动回归与两种主题的原生界面场景通过，候选包已生成。浅色窗口关闭仍触发既有 AccessKit 故障，完整原生退出流程未全通过。

## 已确认范围

- 修复文件／目录勾选卡顿与提交窗口整块刷新；保留树展开、滚动、原生视图身份及草稿。
- 未跟踪文件仅本地勾选，目录覆盖未加载后代；不写索引、不迁入“更改”。
- 删除暂存计数、操作反馈区及其专属状态／弹窗逻辑。
- 分割线左右各留 10px；消息框按实际原生可视行数在 2～4 行之间增长，超过后内部滚动。
- 提交按钮使用插件管理安装按钮的公共 `Button.primary().outline()`，统一默认字号、高度、悬停及禁用效果。

## 实现边界

最终候选为固定源码快照构建的 Git 0.1.38。未跟踪选择与行焦点、实际 Git 状态分离；已跟踪勾选即时发布节点元数据，最新意图及点击时的索引观察按路径保留，同一时刻只有一条经过现有状态／HEAD／脏缓冲区保护的索引写入链。只允许本批实际写入推进后续反向选择的观察；无关外部索引变化使等待中的选择失效。成功观察后撤销临时呈现；失败撤销待处理意图并恢复实际状态。外部明确暂存和实际提交仍使用既有入口。

宿主公开 `ui.native 1.2` 的 `Node::textarea_rows(min, max)`，限制 `1 <= min <= max <= 64`，与固定像素高度互斥。使用原 `TextareaState` 的 auto-grow，无第二份可变文本；默认及主按钮使用公共组件，只有显式主题覆盖才提供自定义 palette，外层不重复背景和禁用透明度。没有按 Git 插件 ID 添加宿主业务分支。

证据目录：`C:/Projects/RustProjects/Editor/target/git-commit-panel-2026-10-24/`。修改前的独立插件保存在 `before/guest/`；现有工作区差异保存在 `before/workspace.patch` 与 `before/staged.patch`，不视为本任务成果。独立插件目录没有 `.git`，没有初始化新仓库、提交或推送。

为了固定并行工作区中的实际输入，最终插件从 `source/Git-for-Nanobug/` 构建，先用 `cargo metadata --no-deps --format-version 1 --offline --config <宿主 SDK 缓存>/host-cargo.toml` 核对独立 `workspace_root` 和 `source-target` 输出目录，结果保存在 `source-metadata.json`。WASM 和原生 helper 仍由宿主 `--plugin-package` 直接调用 Cargo 构建，使用与原 checkout 不同的构建目录。没有使用历史打包脚本或向访客添加仓库路径依赖。

生产入口为独立插件的 `src/commit_panel.rs`、`src/commit_panel/selection.rs`、`src/trees.rs` 及队列／错误清理接缝；宿主改动位于 `crates/plugin-protocol/src/ui.rs`、`crates/plugin-runtime/src/capabilities.rs`、`crates/editor-app/src/ui/plugin/{textareas,theme,leaf,render}.rs`。SDK 独立构建发现共享工作区新增的画布测量子模块未进入内嵌导出，已在 `crates/editor-app/src/sdk_export.rs` 补齐源文件及对应测试输入。

## 初始复现

`untracked-red.log`：`--plugin-cargo ... test --lib commit_untracked_checkbox_only_selects_and_keeps_its_group -- --nocapture` 驱动真实命令构造路径，失败于 `unversioned selection must not construct an index write`。第一次试跑误用 `--exact` 仅筛出 0 项，不计为测试通过；后续修正过滤词获得有效失败。

`native-baseline/`：原 0.1.35 实际包通过原有深色中文 100%／浅色英文 150% 原生流程；记录到深色末次文件勾选首次空闲约 905ms。该基线只检查原有布局稳定性，不能证明本轮需求已满足。

## 最终验证

| 检查 | 实际结果 |
| --- | --- |
| 宿主 `--plugin-cargo ... test --lib` | `guest-tests-final.log`：184 passed，0 failed |
| 实际 ZIP 的 `index_edges::` 回归 | 11 个唯一场景通过；首轮 10 passed、1 项测试范围错误，修正后单独复跑通过，见下文 |
| `cargo test -p editor-app ui::plugin:: --jobs 2 -- --nocapture --test-threads=1` | `ui-tests-final.log`：76 passed、17 ignored，0 failed；含行数策略切换、中文／emoji composition、草稿 revision 回执、原生焦点、Undo/Redo 保留 |
| SDK 导出测试 | `sdk-export-tests.log`：6 passed，独立构建输入完整性与缓存复用通过 |
| `cargo test --workspace --exclude editor-app --jobs 2` | `workspace-tests.log`：551 passed、401 ignored，0 failed；汇总见 `workspace-test-counts.json`，ignored 不计作通过 |
| `cargo check --workspace --jobs 2` | `workspace-check.log`：通过；保留既有 warning |
| 宿主与独立插件 `cargo fmt --check`、`git diff --check` | `style-checks.json`：全部退出码 0 |

实际包回归先执行 `cargo test -p plugin-runtime --test git_plugin index_edges:: --no-run --jobs 2 --message-format json-render-diagnostics`，再运行所返回的原测试程序 `index_edges:: --ignored --nocapture --test-threads=1`。`integration.log` 的首轮 10 项通过，唯一失败是新测试把未打开的比较窗口也要求为零更新：其初始提交 OID 在历史首屏加载后才补齐；提交和历史两个可见面板的文档（包括 revision）实际都保持相同。测试改为对用户实际使用的这两个面板严格断言，未改生产代码或候选包，随后执行：

```powershell
$env:NANOBUG_GIT_CANDIDATE = 'C:/Projects/RustProjects/Editor/target/git-commit-panel-2026-10-24/final-packages/git-0.1.38.zip'
cargo test -p plugin-runtime --test git_plugin git_package_untracked_checks_keep_files_unversioned_without_git_or_editor_requests --jobs 2 -- --ignored --nocapture --test-threads=1
```

`untracked-integration-retry.log`：1 passed。常规仓库与 unborn HEAD 各创建 471 个未跟踪文件，覆盖未加载后代、文件／目录三态、重复取消；原始索引字节保持，文件仍全部未跟踪，ListDocuments 次数为 0，提交／历史文档不变。其他实际包回归覆盖连续勾选与反向选择、等待中无关文件被外部暂存、部分暂存、字面路径、原生脏缓冲区、索引锁失败及恢复、只改索引的取消暂存和差异块、被动刷新不写索引，以及停用后的资源清理。

初次独立 Cargo 检查发现内嵌 SDK 缺失 `src/ui/canvas/text_measure.rs`；补齐导出、重建宿主后重新通过，不使用 SDK 源码副本或直接仓库依赖绕过。首轮控件测试的画布测量断言失败、首次 IME 测试使用了非 Windows 的 Redo 绑定，这些早期日志保留；最终以 `ui-tests-final.log` 的 76 项通过为准，Windows Redo 使用现有 `ctrl-y`。

## 原生界面与退出限制

使用同一个固定候选 ZIP 和 `tools/native-editor-tests.exe`，走真实 Windows 窗口、D3D、公开插件管理器和生产 GPUI 输入路径；没有使用 TestPlatform 替代原生界面。

- `native/dark-zh-CN-100/`：深色中文 100% 的完整窄场景通过，`shutdown_complete=true`，`unexpected_error_logs=[]`。
- `native/light-en-150/` 与 `native-light-retry/`：浅色英文 150% 的交互场景两次通过；两次均在主窗口移除之后遇到下述 AccessKit 故障，`shutdown_complete=false`，不得记为完整退出通过。
- 逐帧检查树／输入区边界、原生视图身份和选中文件不变；未跟踪勾选在持有测试自有 `index.lock` 时仍正常本地选择，并且不启动索引写入、不替换两个可见面板。
- 已跟踪目录与部分暂存文件点击后立即呈现目标勾选态，失败回到实际状态；实际 Git 结果独立核对。深色记录的代表性反馈为未跟踪 106ms、更改目录 72ms、文件 233ms，各次操作均通过 `<350ms` 的反馈门禁；后台 Git 完成时间另行记录，未把提前绘制当作写入成功。
- 两种缩放下原生文本输入区均为 2 行 40px、3 行 60px、4 行 80px（逻辑坐标），边框与正常内边距在行高之外。超过四行保留全部文本并内部滚动；软换行同样受限，清空后回到两行。提交按钮实际高度 28px，使用同一公共 primary outline 组件。
- 已人工查看深色四行上限、浅色两行下限截图，分割线左右留白、已删除区域、按钮外观及主题可读性符合本轮要求。系统 IME 候选窗口未自动化验证；原生输入桥接的 composition 与 Undo/Redo 由通过的 GPUI 回归覆盖。

浅色退出首因为 `accesskit_windows 0.34.0/src/util.rs:380:52` 对 `HRESULT(0x80070578)`（无效窗口句柄）的 unwrap，随后 COM 回调出现 cannot-unwind。日志显示交互场景、项目关闭、输入退役和主窗口移除均已完成，崩溃发生在之后的 UIA 通知。相同故障已见[本地控件迁移记录](../../ui/verification/base-only-components-2026-10-19.md)及[既有基线复现](plugin-api-community-foundation/03-workspace-transactions-native.md)。本轮保留原始失败，没有升级或 fork GPUI/AccessKit，也不把两次浅色退出记为通过。

## 交付

- 插件包：`target/git-commit-panel-2026-10-24/final-packages/git-0.1.38.zip`。
- 配套开发版宿主：`target/git-commit-panel-2026-10-24/tools/Nanobug.exe`，需要宿主提供 `ui.native 1.2`；安装权限与起点相同，协议仍为 7。
- 插件 SHA-256：`35bee98bb9f20a7555c33d0bb26c910ef831e9a2f268839faea2f249f1c1343c`。
- 宿主 SHA-256：`4997e9cecfeb5c9caa14e40078e3b1d6962d7c170929ae83524b7b3f3df2f291`。

两组原生记录及浅色重试记录均使用上述插件摘要。没有覆盖其他并行任务的同名交付包；本轮结果按此固定路径和摘要交付。
