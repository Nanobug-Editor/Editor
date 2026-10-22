# Git 分支查询白屏与增量刷新

日期：2026-10-20 开始，2026-10-21 完成本轮两项功能及冻结候选验收。状态：分支增量刷新与比较单按钮切换已完成；全量格式检查有并发滚动任务的例外，见结案段。之前的工单、比较界面验收和用户已有改动保留。

## 范围与责任

选择分支不能在新查询返回前清空提交列表；读取下一提交时保留最后完整文件树与说明，两个查询均成功后再更新。真实空仓库／无变更提交仍可呈现合法空态。提交记录按相同前后区间保留，只有变化段被替换。

独立 Git 0.1.20 缓存实际成功发布的懒页元数据，按目录前缀和稳定节点 ID 计算变化。未变化目录、状态字节与页不发送结构补丁；新增、删除、类型色彩、当前分支标记及游标变化经公开 `ui.tree 1.4` 更新。未请求目录不提前加载，部分加载页保留数量及正确后续游标。版本、清单与权限分别处理，不增加安装权限。

宿主新增面向所有插件的 `tree::apply_patch` 通用能力：先校验完整候选森林、归属、能力下限、命令身份、图标和预算，再原子发布；非法补丁不影响原树。变化补丁撤销旧请求／动作，无变化不旋转 epoch。原有 `refresh` 全量失效语义保留。没有 Git ID 特判、上游副本、第二份文本或 Undo 状态。

本轮追加比较工具栏修订：四个方向箭头后仅一个折叠／展开切换按钮，继续使用独立包已准入的深浅主题 SVG。存在隐藏区段时展开全部，否则折叠全部；单段展开、导航揭示及标签恢复都按实时区段状态决定动作，不另存布尔开关。

## 诊断记录

- 旧独立源码的 `--plugin-cargo ./Cargo.toml test --lib branch_filter_ -- --test-threads=1` 实际 1 通过／1 失败：新分支返回前原 128 条提交变成空数组；测试本身 0.00 秒。日志为独立项目 `target/branch-incremental-red-2026-10-20.log`。
- 原 `tree::Operation` 不识别结构补丁，公开 wire 回归实际失败；宿主 `target/branch-incremental-wire-red-2026-10-20.log`。
- 定位到 `queue_log` 提前清空、历史预览以“加载中”替换树，以及状态／引用／文件变化时的整树 `refresh`。后者会清除缓存页并产生异步重新加载。
- `--plugin-cargo metadata` 不受入口支持；未扩展无关命令，改用直接 `cargo metadata --no-deps --format-version 1 --offline` 核对独立 workspace／target。新构建使用独立项目 `target/branch-incremental-2026-10-20`。
- 访客首轮完整回归 117 通过／1 失败，旧断言要求文件在消息前发布；按本次原子发布规则修正断言。随后 121/121 通过，最新 5.53 秒；最终包构建后另行复验。日志 `branch-incremental-unit-first-2026-10-20.log`、`branch-incremental-unit-delivery-2026-10-20.log`。
- 原生逐帧夹具首轮失败在仅识别中文目录标签，尚未触达白屏断言；已同时支持 Local／本地，不把该失败作为缺陷复现。
- 接着发现 `Document::active_node` 排除了禁用节点，不能据此判断可见内容是否消失。改用结构遍历定位资源，另以原生可见树行及画布提交文本检查每个中间帧；早期这两次夹具失败均不计缺陷红灯。
- 修正观察器后的同一真实包回归：0.1.19 失败在 `history/files: tree must stay mounted`（74.96s）；0.1.20 通过（80.74s）。日志为宿主 `branch-incremental-native-{old,new}-observer-2026-10-20.log`。此后增强夹具为已加载暂存子目录的真实文件增删和提交文字检查，最终结果另列。
- 提交预览文件补丁被预算／准入拒绝时，恢复之前的文件与状态，完整消息只在补丁成功后发布；相应访客回归通过。新测试初次因测试模块的 `model` 名称遮蔽编译失败，修正为明确导入 `HistoryChange`，不把该失败当作业务复现。

## 验证与交付

当前已完成的实际检查：

| 入口 | 结果 | 日志 |
| --- | --- | --- |
| 独立项目经宿主 `--plugin-cargo ./Cargo.toml test --lib` | 122 通过，0 失败／ignored，7.00s | 独立项目 `target/branch-incremental-unit-final-2026-10-20.log` |
| `cargo test --workspace --exclude editor-app --jobs 2` | 82 组，331 通过、323 ignored、0 失败；忽略项另列，不计通过 | `target/branch-incremental-nonui-final-2026-10-20.log` |
| 显式 `incremental_tree_patch_enforces_capability_ownership_permissions_and_artwork --ignored` | 冻结 database-tree 包引导当前实例，1 通过，24.20s；能力、授权、归属、SVG、退役与原子拒绝 | `target/branch-incremental-admission-2026-10-20.log` |
| editor-app 比较渲染／SDK／aligned／通用插件 UI／preview tabs | 126 通过、23 ignored，23.54s | `target/branch-incremental-ui-2026-10-20.log` |
| 真实 0.1.20 包双／三栏比较与单按钮来回切换、标签保留 | 显式运行 2/2 通过，163.03s | `target/git-toggle-real-package-2026-10-20.log` |

首次 `cargo fmt --check` 通过。之后共享工作区出现并发的 `ui.canvas 1.2`／`CanvasEvent::ScrollInput` 改动，后一次全量格式检查失败于该部分；新原生测试二进制构建因终端 `interaction.rs` 尚未覆盖该事件而失败（E0004）。此限制不改写为本任务检查通过，也不回滚或顺手修复其他任务源码。最终逐帧增强回归、候选构建和完整门禁待工作区恢复可构建后补齐。

并发滚动任务随后自行补齐消费者，当前 editor-app 测试二进制重新构建通过（1m54s）。它同时将独立插件提升为 0.1.21，并明确要求 `ui.canvas 1.2`；本任务保留该版本与滚动改动，最终需用当前宿主的 SDK 编译。中途用旧 SDK 尝试打包失败于尚在写入的 drawing 模块／缺失新滚动事件，未生成新 ZIP，未作为成功交付。此前 0.1.20 的已通过测试保留为阶段证据，最终包身份与验证另列。

增强原生回归已实际通过：暂存目录事先展开并加载文件，切换历史分支、实际外部 checkout、通过 Focus 刷新新增／删除文件时，每个已发布绘制帧都检查暂存树、提交文件树和真实提交文字，最后核对旧文件 ID、根目录及展开保留（1/1，60.28s，`git-incremental-native-add-remove-2026-10-20.log`）。单按钮新增的物理区段展开测试也通过，展开占位行后按钮立即恢复折叠动作，标签切换和原文本／选择保留（1/1，90.63s，`git-toggle-native-disclosure-2026-10-20.log`）。两者此阶段使用已保存的 0.1.20 中间包。

重跑通用 UI 时，较早测试二进制的并发 SVG 位移复用测试失败，结果为 131 通过／1 失败／23 ignored（`git-incremental-review-ui-final-2026-10-20.log`）。当时其修复仍在另一任务写入中，不能记作全组通过；最终使用重建二进制复验。

## 冻结候选与最终相关验收

候选位于 [git-incremental-review-2026-10-20](../../../target/git-incremental-review-2026-10-20/)；目录日期为本轮开始日期。宿主通过 `cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o <候选目录>/Nanobug.exe` 构建，3m52s。最终独立 0.1.21 由该宿主直接 `--plugin-package` 构建，仍经公开 SDK、独立源码及原生依赖交付，不进入宿主发行。与 0.1.19 比较，安装权限集合相同。

| 实际入口／范围 | 最终结果 | 日志（宿主 target/，访客日志另注） |
| --- | --- | --- |
| `cargo check --workspace --jobs 2` | 通过，30.45s，保留既有 warning | `git-incremental-review-check-2026-10-21.log` |
| 最终宿主 `--plugin-cargo ./Cargo.toml test --lib` | 独立 0.1.21 的 122/122，6.34s | 访客 `target/git-incremental-review-unit-2026-10-21.log` |
| 最终测试 EXE：比较渲染／SDK／aligned／通用 UI／preview tabs／显隐／焦点 | 132 通过，23 ignored，0 失败，25.58s；含已修正的并发 SVG 位移回归 | `git-incremental-review-ui-verified-2026-10-21.log` |
| 最终 0.1.21：分支查询及真实 checkout、文件增删逐帧回归，显式 `--ignored` | 1/1，63.12s；暂存树、提交文字、提交文件树持续可见，旧文件、展开及根目录缓存保留 | `git-incremental-review-native-final-2026-10-21.log` |
| 最终 0.1.21：比较标签及双／三栏历史预览，显式 `--ignored` | 2/2，176.96s；单一按钮位置与名称、往返切换、真实区段展开、标签恢复、主题背景及唯一滚动轨道、源文本与选择保留 | `git-incremental-review-comparison-final-2026-10-21.log` |
| 最终 0.1.21：分支显隐保留与原生右键菜单，显式 `--ignored` | 2/2，126.36s；保留页、展开、选择、非零滚动及焦点，禁用动作拒绝 | `git-incremental-review-branches-final-2026-10-21.log` |
| `git diff --check` | 通过，保留既有行尾转换提示 | `git-incremental-review-diffcheck-2026-10-21.log` |

最终非 UI workspace 与全量格式复查见结案段。ignored 未执行项不计为通过；没有把整个原生矩阵、历史工单或另一滚动任务标记为本轮完成。

| 冻结产物 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `Nanobug.exe` | 77,926,912 | `66030ef77d3cf65f3af9897918f449f3194bcbdee803520ee8c3302a2e1150e1` |
| `git-0.1.21.zip` | 867,060 | `dce2c4b27edd8f83ea251790801e988274f486ae3b2a4b7e751f7c996e7482f9` |
| 包内 `git.wasm` | 2,723,608 | `dc250063bb798cf4a6854fd9d5d8ccf7db316d9e08908d1c820505be9abdfb7f` |
| `git-for-Nanobug-0.1.21-source.zip` | 197,465，68 项 | `2eaf965cf04940707067c05a33072afb4407806a919a5d886f01934d8a16393d` |

包内原生 helper 仍为 448,000 bytes，SHA-256 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。最终编译与打包使用 SDK `f2b584195017562f56f1ea0a85b225b8ea271aa47b1d0ccfe8c0cef034f87f49`；其全部嵌入 Rust 源与当时 workspace 逐一比较相同，两个必需能力分别为 `ui.tree >=1.4` 与 `ui.canvas >=1.2`。68 项源码 ZIP 逐一读回匹配同目录的 `guest-source-inputs.json`。

此候选来自 `main` 的 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 上的既有脏工作区。`host-build-inputs.json` 记录了构建开始的 1,117 个选定输入；构建和验收期间另一滚动任务继续修改测试／原生验收代码、发布节拍、画布准入及访客绘制发布策略。候选与其源码 ZIP 按上表固定，不宣称包含另一任务后来写入的全部源码；`identity.json` 记录快照差异。不能用当前持续变化的工作区替代这些冻结产物来判断上述验收结果。本轮通用树补丁与折叠控件的目标源码未被这些后续改动覆盖。

## 结案与检查例外

`cargo test --workspace --exclude editor-app --jobs 2` 最终退出 0：82 组，332 通过、324 ignored、0 失败，见 `target/git-incremental-review-nonui-2026-10-21.log`。实际包、界面和编译检查的最终通过结果见上表，公开树准入的显式 ignored 回归见前表。最终候选文件、包内 WASM／helper 和源码 ZIP 均已计算摘要并读回核对。

最后的 `cargo fmt --check` 仍退出 1，差异在另一滚动任务正在修改的 `crates/plugin-protocol/src/ui/scroll_tests.rs`；独立项目 `cargo fmt --all --check` 同样退出 1，差异位于该任务新增／改写的 `src/history.rs`、`src/lib.rs` 和 `src/views.rs`。没有修改或覆盖这些并发工作，不将这两次失败计为通过。日志分别为宿主、访客 `target/git-incremental-review-fmt-last-2026-10-21.log`；本轮新增树补丁、预览保留、单按钮及其回归未出现在格式差异中。

本轮没有再次运行完整原生矩阵和系统 IME 候选窗人工验收；先前相应记录保持原结论。另一个滚动任务的后续源码及其验收由该任务继续承担，未借本轮候选将其标记完成。

未安装插件、替换用户运行程序、自动重启、提交、推送或发布版本。用户已有布局图摘要仍为 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6`。

返回[插件目录](../README.md)；相关[分支树保留记录](git-branch-retention-2026-10-19.md)、[比较区交付](../../ui/verification/comparison-chrome-2026-10-20.md)。
