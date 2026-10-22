# Git 首次选择、提交预览与文件状态颜色

日期：2026-10-19。状态：实现、最终独立包原生回归、规定检查与候选交付已完成。

## 本轮行为与边界

- 独立 Git 0.1.17 在首次引用读取后发布一次当前本地分支的根至目标 ID 链。宿主按正常懒页、重试与来源权限加载祖先和必要的后续页，原生选中并滚动到目标；初始全部引用历史不变，不伪造游离／未产生引用的分支。
- 首次完整提交页默认选中首条并读入文件及完整提交消息。明确切换分支筛选时同样选择该列表首条；后台刷新、翻页及显隐保留用户后续选择。默认动作不打开比较标签或独立窗口，不抢焦点。
- 历史文件查询保留 NUL 分隔的类型与原始路径：新增绿色、删除红色、修改／类型变化蓝色；重复父提交路径合并，混合状态标为修改。此视图关闭重命名检测，两侧路径分别保留为删除／新增，不漏掉删除侧。
- `ui.tree 1.3` 向全部插件公开 `tree::initialize(tree, path)` 与 `Node.tone`，新增字段／操作需要显式 1.3 required 下限；原有 1.0～1.2 范围不因宿主库存升级获得新语义。归属、权限、深度／分页／元数据预算及旧原生动作撤销仍适用。已有分支缓存更新机制保留。
- 初始化至多一次；用户原生鼠标、键盘或滚动输入优先，输入甚至可先于初始化到达。完成／取消后刷新和显隐不复位。来源切换拒绝旧默认请求，当前场景可重试未发生资源变动的页意图。

## 分页修正与红灯证据

新增回归实际测得完整右键菜单的 128 条分支页为 **726168 bytes**，超过公开的 512 KiB 单页预算；真实 140 分支原生场景也因此不能完成初始路径。访客改为同时按节点数和序列化字节划页，预留 JSON 页封装／游标空间，游标只推进已发布数量；200 个分支分页不重不漏，单节点超限明确报错。不是截断菜单、跳过分支或放宽宿主预算。

旧候选 SHA-256 为 `3aa2c2b7737ff56d04ff3b015aa6bae038edf822fec6bb3b1aaae4a831cdef72`，失败日志分别为独立项目 `target/default-selection-page-budget.log` 和宿主 `target/git-default-selection-native-initial-final.log`。最终包为不同摘要，旧包验证不计入最终包通过记录。

## 最终验证

| 范围 | 命令／入口 | 结果 |
| --- | --- | --- |
| 独立访客 | 最终宿主 `--plugin-cargo ./Cargo.toml test --lib` | 114 通过，5.79s；包括默认首行、刷新保留、路径状态与字节分页回归 |
| 通用插件 UI | 当前源码测试程序 `ui::plugin:: --test-threads=1` | 68 通过，17 ignored，3.81s；含深浅主题文字颜色、原有焦点／输入／显隐回归 |
| 实际 ZIP 默认选择 | `windows_tests::branch_retention::git_initial_selection_reveals_paged_current_branch_and_colored_first_commit --ignored` | 1 通过，88.66s；当前分支位于多级目录、140 个同级分支的后续页；初始选中、不抢焦点，默认消息与增删改三色，改选／显隐后保留原生状态 |
| 实际 ZIP 原有保留 | `windows_tests::branch_retention::git_branch_filter_and_dock_toggles_retain_loaded_native_tree_state --ignored` | 1 通过，87.84s；选择、展开、原生滚动、显隐、后续分支筛选保持缓存及焦点身份 |
| 实际 ZIP 历史标签 | `windows_tests::history_preview::git_inline_history_preview_opens_readonly_two_and_three_pane_tabs --ignored` | 1 通过，79.01s；右侧文件点击、双／三栏只读标签、固定与清理仍通过 |
| 实际 ZIP 历史数据 | `git_plugin` 的 `workbench::history:: --ignored` | 2 通过，153.10s；根／普通／多父提交、完整文件与消息、分页、焦点／保存／新提交更新 |
| 实际 ZIP 能力下限 | `git_package_extended_branch_menu_requires_an_explicit_tree_minor_floor --ignored` | 1 通过，69.86s；旧 required 范围无权因新库存使用新增树语义 |
| 原生来源切换 | `ui::plugin::tree::refresh_tests:: --ignored` | 2 通过，114.34s；旧来源不能发页请求，当前来源可恢复被撤回的页意图 |
| 运行时初始化与下限 | 非 UI workspace 中 `trees::initial_tests::` 及 `initial_selection_requires_explicit_presentation_floor` | 3 通过；包括提前输入、一次性、刷新／隐藏、路径预算、撤销与低 minor 尾部补丁拒绝 |
| 非 UI workspace | `cargo test --workspace --exclude editor-app --jobs 2` | 82 组，321 通过，322 ignored，0 失败；忽略项不计通过 |
| 最终门禁 | `cargo check --workspace --jobs 1`、宿主和独立项目 `cargo fmt --check`、`git diff --check` | 均通过；既有行尾空白已由共享工作区其他修改消除，不计为本任务修复 |

首次颜色单测缺少测试环境 Typography 初始化，补齐夹具后完整 UI 68 项通过；首次懒页原生测试也补齐了对原生资源脉冲的等待，不能只用 Git 进程结束判断树页已完成。分页超限的实际缺陷则由上一节独立红灯及更新后的真实包重跑确认修复。真实包通过项全部使用下表最终 ZIP 摘要，早期旧包通过记录未混入。

为避免 Windows 并行编译重链接正在运行的测试程序，将已编译的当前源码测试 EXE 复制到 `target/git-default-selection-test-snapshot/` 后执行。测试仍通过真实 Package／Manager、现有 worker 发布及 GPUI 控件接收物理输入；没有新建测试专用宿主能力。以上为 Windows GPUI 测试窗口集成，不含桌面 D3D 截图、系统 IME 候选窗或全部历史八组合原生矩阵。

## 候选身份

目录：`C:/Projects/RustProjects/Editor/target/git-default-selection-2026-10-19/`。宿主最终通过 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o <候选目录>/Nanobug.exe` 构建，7m23s；没有替换运行中的用户程序。

| 产物 | 大小 | SHA-256 |
| --- | --- | --- |
| `Nanobug.exe` | 77881856 bytes | `f70d4f9c5ba3d59c413f75b77f3d31e0c8d38eb326da2990245b2579b5a98b03` |
| `git-0.1.17.zip` | 830048 bytes，7 项 | `5256c918fdbb918f87e9f71048d8b51bf16675b37b115f36ca21f76741746552` |
| 包内 `git.wasm` | 2579378 bytes | `ce4ac27171e5ee26a31224f9bf98a424a68c10cf7768d18694137dedb6883321` |
| `git-for-Nanobug-0.1.17-source.zip` | 183176 bytes，62 项 | `eef54895ea763026faf8dc86bf58959fd7f9e7040a717328f09370b3f6626000` |

独立源码快照、包副本及输入清单另存 `C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.17/`，62 项逐一读回与当前源码完全相同，无 SDK／上游源码和构建缓存副本。实际 SDK 缓存为 `29a021484c5e4ad103eb00dc983f7478eb26b7dc489eac0517645d9a91e1ddc1`，独立依赖记录核对相同缓存。安装权限与 0.1.16 相同，能力下限明确提升至 `ui.tree >=1.3,<2`。原生 helper 摘要仍为 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。

候选目录保存 `identity.json`、`SHA256SUMS` 和使用说明。原有和并发工作区修改保留，不计为本次成果；未自动安装、提交、推送或发布。

日志：宿主 `target/git-default-selection-{ui-final,native-green,native-retention,native-preview,history-green,capability-green,tree-handover,nonui,workspace-check-final,format-delivery,diffcheck-delivery,host-build-final}.log`；独立项目 `target/default-selection-{unit-green,package-green,format-final}.log`。
