# Git 历史改动文件树

日期：2026-10-24。状态：本轮实现与验证完成，未提交、推送、发布或修改用户安装。

用户确认历史右侧文件区使用总数树根、目录递归计数、单链目录压缩及普通 UI 排版，并保留真实路径差异预览和刷新后的原生状态。

独立插件源：`C:/Projects/RustProjects/Git-for-Nanobug`，固定验收候选版本 0.1.41。共享源码另含其他任务的 0.1.42 列宽改动；本轮使用基于 0.1.40 加本次文件树修改的独立快照，未将列宽改动混入候选包。宿主通过面向所有插件的 `ui.tree 1.8` 提供 `Node.muted` 和不可变 `Options.proportional_labels`；旧清单不能通过新宿主自动使用这些特性，默认外观保持原有契约。

证据目录：`target/git-history-files-2026-10-24/`。基线源及已有宿主 diff 已保存；不把工作区其他改动算入本任务，也未提交、推送或修改用户插件安装。

## 回归与原生验收

- 红测：旧 Git 0.1.40 实际 ZIP 经公开 Manager 运行 `git_package_history_files_has_one_counted_expandable_root`，实际 3 个顶层节点，期望 1 个，按预期失败。
- 初次编译遇到其他进行中画布改动缺少 `ColumnsResized` 分支；该文件后续由原有工作更新，本任务未修补该画布逻辑。
- 最终候选 `target/git-history-files-2026-10-24/packages-final/git-0.1.41.zip`，SHA-256：`BDAB21D1C66282DD63504A8A076B120D4E1276DEFBB7288137BC3903F33A6EEE`。以下最终实际包测试均使用此摘要。
- 独立候选源码 `--plugin-cargo ... test --lib`：186 passed，0 failed，0 ignored。
- 实际包总数／紧凑路径／计数／路径比较／刷新／提交切换／分页回归：1 passed。连续验证 5 → 1 → 0 → 140 个文件，128 项首屏及后续 12 项均保留。
- 实际包删除 `foo` 同时新增 `foo/bar` 的回归：1 passed，两种节点身份及删除文件的真实父版本内容保持正确。相关历史核对为 Hindsight `a1e38656-75e7-4b05-b309-22c0654256af`，文档 `conversation:01a14cc3-6caa-7c92-a938-4260aec28590`；只作为历史证据，不扩展当前授权。
- 宿主已有通用树原生状态回归：1 项主题测试及 3 项显式执行的固定 WASM／GPUI 测试通过，覆盖原生键盘重试及界面撤回、源切换后恢复。
- 主工作区 `cargo fmt --check`、独立候选 `cargo fmt --check`、`git diff --check` 均 exit0。
- 固定宿主源码 `cargo test --workspace --exclude editor-app`：110 组，554 passed，0 failed，402 ignored。被跳过的测试不计为通过；本次相关的实际包、通用树及能力门禁均另行显式执行。
- 固定宿主源码 `cargo check --workspace`：exit0。为保持本次独立输出稳定，完整编译使用 `CARGO_BUILD_JOBS=1`，没有更改源代码或依赖版本来绕过检查。
- `tree_interaction_options_require_explicit_floor_and_preserve_owned_lifecycle --ignored`：1 passed，实际固定 WASM 组件覆盖旧要求拒绝／1.8 要求接受普通字体和次要文字色，以及资源刷新、撤销与权限检查。
- 最终宿主七处功能源码与主工作区对应文件 SHA-256 一致；主工作区及共享独立插件的格式检查、差异空白检查再次 exit0。
- 最终 `git_native_history_files_probe --ignored --nocapture --test-threads=1`：父级汇总 exit0，深色中文 100% 与浅色英文 150% 两个 Windows/D3D 子场景均通过。核对默认展开、目录完整计数、紧凑路径、真实只读源及绘制范围、键盘折叠、手动折叠后刷新、选择与滚动保持；两组 `shutdown_complete=true`、意外错误日志为空、原生窗口均已正常撤销。

## 最终原生证据与使用边界

最终证据目录 `target/git-history-files-2026-10-24/native-accepted/`，两组各有 `evidence.json` 与以下三张截图：

- `history-files-expanded.png`：总数、目录计数、紧凑路径及原生排版。
- `history-files-exact-source.png`：选择真实路径 `crates/editor-app/src/a.rs`，主比较页的只读文档值为 `immutable crates/editor-app/src/a.rs\n`，实际字形范围已绘制。
- `history-files-refreshed.png`：真实滚轮滚动后完成刷新，选择、展开与相对滚动位置保留。

深色目录为 `dark-zh-CN-100`，浅色目录为 `light-en-150`。父级明确核对本场景回执与三张图片，不再沿用旧历史键盘场景的截图列表。最终汇总为 `target/git-history-files-2026-10-24/validation-results.json`；各失败中间包、日志与最终包分目录保留，没有覆盖其摘要。

验收覆盖本轮历史文件树，不宣称整个 Git 工作台、安装器或发布流程的全量原生验收。候选需与本次提供 `ui.tree 1.8` 的配套宿主一起使用；未安装到用户现有配置，也未发布。

## 紧凑目录换层级的增量修复

实际提交切换回归发现：从多个目录变成仅 `crates/editor-app/src/b.rs` 时，末端目录的真实路径和 ID 不变，但显示父级变了。原先扫描所有旧缓存分支会从新旧父级各生成一次该节点，公开契约拒绝 `Duplicate tree patch node`，旧文件集合因此继续可见。快速回归 4.4 秒复现该错误后，将增量生成改为从新根开始遍历仍可到达的已加载分支，保留有用缓存和稳定 ID，跳过旧父级的孤儿页。186 项单元测试及最终实际包提交切换回归均通过。

## 验证环境与早期失败

宿主快照为 `target/git-history-files-2026-10-24/source/Host`，其输出目录为独立的 `host-target`；插件快照及输出分别为 `source/Git-for-Nanobug` 和 `guest-target`。两份源码均先用 `cargo metadata --no-deps --format-version 1` 核对根和输出。为避免共享构建锁，宿主输出预热了第三方依赖缓存；完整 workspace 链接时发现其中 metadata／rlib 不完整，已核对绝对目标并仅在此独立输出目录按锁定包清理、串行重建，没有修改产品接口或用户安装。

初次原生冷启动曾出现暂停／等待失败；后续运行未重复该暂停。验收脚本另有两处已修正的错误假设：共享树折叠会取消后代展开，刷新期间可撤回焦点；历史文件实际打开主编辑区的原生比较预览，不能等待旧独立详情窗口。最终交互回执与截图单独记录，不把这些早期失败算作通过。

回归覆盖目标包括完整后代计数、128 项续页、目录与同名删除文件、空集合、切换提交以及真实提交路径。原生验收使用实际包和 Windows/D3D 窗口，检查默认展开、键盘折叠、手动折叠后刷新、选择与滚动保留及真实不可变源。
