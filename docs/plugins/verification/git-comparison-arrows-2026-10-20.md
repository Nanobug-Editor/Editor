# 比较工具栏箭头与原提交文件导航

日期：2026-10-20。状态：四箭头指定行为、独立包及规定检查已验收；附加旧比较视图无障碍命名检查在新旧构建均失败，见下文，不计为通过。

## 行为与边界

- 工具栏按左、右、上、下排列四个原生图标按钮：原提交的上一个／下一个改动文件、当前文件的上一处／下一处差异。中英文悬停说明同时提供可访问名称。
- 移除截图所示的统一视图和同步滚动按钮，默认分栏同步；保留上一轮阅读边距、完整行与行号槽底色、补白及原生编辑会话。
- 文件顺序固定于打开比较时的完整改动文件列表，提交 OID 和所选父提交组合固定于比较来源。选择别的提交、保留固定标签不能替换该导航上下文。首尾方向禁用、不循环；工作区／暂存区比较没有提交文件序列，左右箭头禁用。
- 独立 Git 0.1.18 拥有文件序列和 Git 查询。宿主只提供公开通用 `editor.diff 1.3` 的可选 `OpenDocumentComparison.navigation`。动作必须属于当前实例声明的类型化命令，每方向参数至多 64 KiB；发布时校验能力、声明和参数，执行前重新核对原始文档及实例生命周期。关闭来源、禁用或替换实例后不能把旧动作交给新实例。
- 箭头不自动打开插件面板，也不切换 Git 日志当前预览。没有安装新权限，没有将 Git 业务内置到宿主。

## 验证记录

最终 `cargo test --workspace --exclude editor-app --jobs 2` 为 82 组、324 通过、322 ignored、0 失败；忽略项不计通过。接口准入过滤测试 5 通过，覆盖 1.1／1.2／1.3、来源去重、类型化命令、参数、大小限制及原始实例所有权。

提取类型化命令校验后的首次 Release 构建发现缺少 `BTreeMap` 导入，已补齐。首次独立访客构建随后发现 `sdk_export::SDK_FILES` 漏了 `src/api/comparisons.rs`，产生 E0583；已修正正式内嵌 SDK 清单及 required-resource 断言，重新构建宿主，不手工修改 SDK 缓存。上述失败日志保留，均不计通过；后续使用最终宿主及完整 SDK 重新构建访客和包。

实际包首次组合为 1 通过／1 失败（115.69s），历史导航未切换文件；单项诊断重现 55.05s，回执明确为 `PermissionDenied: Virtual providers require direct ownership`。旧准入把任何调用上下文都当成委托，因此误拒绝本实例直接响应类型化命令时创建、刷新和读取私有来源。复用现有 `Context::direct_host_command` 的运行时签发来源、单跳、实例及生命周期校验后，仅允许真实本实例原生命令；跨插件、嵌套及已撤销调用继续拒绝，原资源归属、版本及权限检查保留。快速回归先 0 通过／1 失败，再 5 通过（断言阶段 0.00s）；日志为 `ownership-red`／`ownership-green`。

修正后的组合中，工作区差异导航已通过，历史导航也实际切换到相邻文件，但测试把上游按钮没有暴露的 `disabled` 标志误当成 false。按公开 `ElementSnapshot::disabled` 契约修正该观察：存在标志时核对值，首尾则通过真实点击及标签身份不变验证不循环。最终历史单项通过，不添加测试专用宿主接口，也未修改上游源码。

日志保存于宿主 `target/comparison-arrows-*-2026-10-20.log`，独立项目日志保存于其 `target/arrows-*-2026-10-20.log`。

| 范围 | 实际入口 | 当前结果 |
| --- | --- | --- |
| 独立访客 | 最终宿主 `--plugin-cargo ./Cargo.toml test --lib` | 115 通过，6.06s；包括冻结提交／父顺序／首尾／失效来源回归 |
| SDK、完整行底色、原生控件 | owned 测试副本 `editor::comparison::render_tests:: sdk_export::tests:: ui::controls::aligned_ ui::plugin:: --test-threads=1` | 110 通过、23 ignored，9.66s；实际 SDK required-resource 检查通过 |
| 对齐／输入／同步 | owned 测试副本 `ui::controls::aligned_ code_sync::alignment_tests --ignored --test-threads=1` | 20 通过，60.67s；非 UI 默认忽略项不借此计通过 |
| 真实 Git 包：上下箭头与原文档 | owned 测试副本 `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs`，显式 `--ignored` | 1 通过：真实上下点击揭示两个差异，保留文本、选择及未保存内容；同次历史测试的观察失败不计通过 |
| 真实 Git 包：左右箭头 | boundary 测试副本 `git_inline_history_preview_opens_readonly_two_and_three_pane_tabs --ignored --test-threads=1` | 1 通过，63.99s；两主题的箭头顺序／尺寸，选其他提交后仍浏览固定合并提交，首尾真实点击不循环，根／普通／合并来源只读及退役清理 |
| 工作区编译 | `cargo check --workspace --jobs 2` | 所有权修正后最终复查通过 |
| 格式／空白 | `cargo fmt --check`、独立访客 `cargo fmt --check`、`git diff --check` | 通过 |

所有权修正后的测试副本为 `target/comparison-arrows-owned-tests-2026-10-20.exe`，修正按钮观察后的历史副本为 `target/comparison-arrows-boundary-tests-2026-10-20.exe`；均通过 `cargo test -p editor-app --bin editor-app --no-run --jobs 2` 构建，后者编译 1m24s。两者生产代码和包摘要相同，差异仅为历史测试的公开快照观察及边界断言。Windows 执行测试副本，避免占用 Cargo 输出中的可执行文件。

额外运行 `community_consumers_compare_visible_panes_and_retire_resources --ignored --test-threads=1` 在“左侧必须暴露带名称的只读原生文档”断言失败（83.40s）。使用修改前保存的 `target/diff-row-backgrounds-tests-delivery-2026-10-20.exe` 原命令重跑，同一文件 149 行同样失败（75.55s）；是既有比较视图无障碍命名检查问题，本轮未修改或弱化该测试，未将该项及其后续未达到的断言计通过。日志为 `source-ownership` 及 `source-ownership-baseline`。

最终 SDK 摘要为 `593964905ee57992abb9fa5148009bc6a94dc6354e60e3c9100d7e706b91fbd2`，由配套宿主公开导出；独立源码不携带 SDK 或上游项目副本。

## 交付范围

配套目录为 `target/comparison-arrows-2026-10-20/`；最终宿主由 `cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/comparison-arrows-2026-10-20/Nanobug.exe` 构建，所有权修正后 4m02s，未覆盖运行中的默认 Release EXE。独立包经公开 `--plugin-package . --output <配套目录>` 构建，7 个 ZIP 项；包内清单为 0.1.18、`editor.diff >=1.3,<2`，包含声明的类型化邻项命令。最终宿主与构建包时的 SDK 摘要相同；包及 WASM 未变，两项 Git 原生场景均使用下表同一 ZIP 摘要。

| 产物 | bytes | SHA-256 |
| --- | --- | --- |
| `Nanobug.exe` | 78036992 | `847662c4dea3817a67d10d235b02bcc7248cc57d6029274db0cfa88773dfa069` |
| `git-0.1.18.zip` | 834259 | `35acc491a6c808a9a63214c7b3286f5a0aa6a17fe0d714a651cf6424fcbe618b` |
| 包内 `git.wasm` | 2591626 | `35009b05fb09bd272183ab8f571792ee9a15c29ff214e132db371b1d9e2b9b91` |

同目录 `identity.json`、`SHA256SUMS`、`使用说明.txt` 保存配套身份及启动／本地更新入口。15 个选定宿主输入、56 个插件输入及两项产物摘要已逐一读回相同；SDK 两个入口源码与仓库字节相同，不含 Markdown，原生 helper 的包内摘要与声明一致。该清单不是完整干净源码快照；共享改动的归属不因此改变。

现有共享工作区修改全部保留；本任务不提交、推送、关闭议题、安装插件或重启用户运行的程序。历史 #119／#120 状态不变。当前原生验证使用 Windows GPUI 测试窗口及实际输入、布局／可访问快照，桌面 D3D 逐像素截图、系统 IME 候选窗口及完整历史工单矩阵未执行，不计为本轮通过。
