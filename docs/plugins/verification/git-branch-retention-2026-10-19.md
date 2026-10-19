# Git 分支树保留与菜单状态更新

日期：2026-10-19。状态：修复完成；配套宿主、独立 0.1.15 包、实际 ZIP／原生窗口回归及规定检查通过。

## 原因与修复边界

使用原独立 `git-0.1.14.zip` 从公开 Package／Manager 入口复现：已加载、展开的分支树单次选择分支后，epoch 从 6 变成 8，缓存页被清空。日志为 `target/git-branch-retention-red.log`。选择仅筛选历史，没有实际修改 refs。

根因是插件将 `ui_busy`、待确认操作和分支输入弹窗状态合成菜单戳；戳变化就调用 `tree::refresh`。历史读取开始与完成各触发一次结构失效，面板显隐要求的状态读取也会进入相同路径。宿主随后对每次树资源 revision 调用原生 `TreeState::set_items`，增加选择、滚动和焦点被打断的风险。

- 独立 Git 0.1.15 将菜单展示状态与分支成员变化分开。读取、弹窗、可用性及只有工作区文件变化时使用缓存节点更新；真实 refs／当前分支变化仍触发结构刷新。
- 只记录实际发布过的懒页节点；分支成员变更时清除旧映射，避免未请求节点或已删除分支不断扩大后续更新量。每批更新受节点数及字节预算限制。
- 宿主提供面向全部插件的 `ui.tree 1.2`／`tree::update`。同一资源内原子更新节点展示和命令禁用状态，保留 epoch、缓存页、展开、续页及未完成请求；不允许改变目录类型、命令参数、动作顺序或分隔身份。未知节点验证后忽略。
- 原生树只在 TreeItem／页面结构确实变化时替换条目集合；菜单、描述和图标变化只重绘，保留原生选择和滚动。
- 首次原生测试发现第二条路径：面板隐藏会丢弃整个 `native_ui`，重开虽复用资源页却丢失 Base 选择。`target/git-branch-retention-native.log` 记录 `history visible=true: lost selected branch`。宿主已将普通隐藏改为暂停场景和代码输入投影、保留 keyed 原生实体；实际来源／实例退役仍销毁控件。最终原生回归须同时验证页缓存及非零滚动／选中项，不能只看 epoch。
- 隐藏场景仍可撤销旧回调的 epoch，缓存页不会因此重载。这是来源权限撤销，不能以放宽版本验证代替修复。
- 保留控件的生命周期审查发现普通按钮未持有来源票据，隐藏／重开间的旧 Down 可能复活。原生测试 `retained_view_visibility_revokes_ordinary_held_presses` 已实际复现（无中间帧，1 失败，0.02s；`target/git-branch-retention-press-red.log`）。普通保留控件现使用独立种类的可见性票据，与 source 票据共享单调 nonce，撤销旧 Base 按压身份；按钮／选项使用显式稳定焦点句柄。票据不授予文档权限，IME 草稿及原生树仍保留，实例／来源销毁路径不变。

Git 保持独立插件；宿主没有按 Git 插件 ID 处理的业务路径。0.1.15 显式要求 `ui.tree >=1.2,<2`。typed `branch-context` 回调复用既有 `assets.read` 授权更新图标元数据，包级安装权限不增加。SDK 通过配套宿主的 `--plugin-cargo` 缓存提供，不复制上游或宿主 SDK 源码。

## 验证

以下为实际执行结果。未显式运行的 ignored 项不计入通过数。

| 范围 | 命令／入口 | 结果 |
| --- | --- | --- |
| 实际旧 ZIP 红灯 | `cargo test -p plugin-runtime --test git_plugin workbench::branch_retention:: --jobs 1 -- --ignored --test-threads=1`，指定原 0.1.14 | 1 失败；单次选择触发两次结构失效 |
| 公开 wire 红灯 | `cached_metadata_update_has_a_public_wire_operation` | 原协议不识别 `kind:update`，1 失败 |
| 通用缓存更新 | `cargo test -p plugin-runtime --lib cached_metadata_update --jobs 2` | 9 通过、1 ignored；忽略项另行显式运行 |
| 当前实例准入 | `instance::trees::admission_tests::cached_metadata_update_enforces_live_dispatch_ownership_grants_and_icon_admission`，显式 `--ignored` | 1 通过，19.54s；能力最低版本、授权、归属、未知节点无 SVG 读取、非法图标原子拒绝及退役 |
| 独立访客 | 最终交付宿主 `--plugin-cargo ./Cargo.toml test --lib` | 最终 110 通过，5.25s；包括只记录实际发布的懒页节点 |
| 非 UI workspace | `cargo test --workspace --exclude editor-app --jobs 2` | 78 组，304 通过、311 ignored、0 失败；忽略项不计通过 |
| 通用树来源切换 | editor-app 测试二进制 `ui::plugin::tree::refresh_tests:: --ignored --test-threads=1` | 最终 2 通过，79.28s；来源切换后的元数据恢复、已撤销／禁用来源不分配请求 |
| 真实 ZIP 缓存与图标／布局／版本门禁 | `git_plugin` 的 `workbench::branch_ --ignored --test-threads=1` | 缓存回归及图标、布局、门禁共 4 通过；首次运行的原菜单测试末尾仍假定旧页必定重新请求，见下条重跑 |
| 真实 ZIP 分支菜单 | `cargo test -p plugin-runtime --test git_plugin workbench::branch_menu::git_package_branch_context_owns_targets_and_rejects_disabled_actions --jobs 1 -- --ignored --test-threads=1` | 1 通过，64.84s；缓存页无需重复提供、禁用拒绝、切换、新建、合并预览及取消 |
| 最终原生树 | editor-app 测试二进制 `extensions::windows_tests::branch_retention::git_branch_filter_and_dock_toggles_retain_loaded_native_tree_state --ignored --test-threads=1` | 最终 1 通过，56.48s；缓存页／展开／选择／非零滚动／焦点保留，显隐后继续选择其他分支再返回也不重载 |
| 显隐与焦点 | editor-app 测试二进制 `extensions::visibility_tests::`、`extensions::window_focus_tests::` | 最终分别 2 与 3 通过，0.26s／0.46s |
| 原生分支右键菜单 | editor-app 测试二进制 `git_branch_menu_native_right_click_disables_unavailable_actions --ignored --test-threads=1` | 1 通过，49.90s；原生右键及禁用动作 |
| 通用插件 UI | `cargo test -p editor-app ui::plugin:: --jobs 1 -- --test-threads=1` | 最终 67 通过、17 ignored、0 失败，3.79s；包括按住普通／弹窗关闭按钮在有／无中间帧显隐后旧按压失效、新点击可用及菜单重开选择，忽略项不计通过 |
| 格式与补丁 | `cargo fmt --check`、`git diff --check` | 通过；独立项目格式检查同样通过 |
| workspace 编译 | `cargo check --workspace --jobs 1` | 最终通过，3.15s；既有警告保留 |

真实包回归覆盖选择、暂存／Git 显隐、只有文件变化时的 Stash 菜单更新，以及外部新增分支仍可见。原生回归通过实际鼠标展开、分支点击、滚轮和底栏按钮检查缓存页、展开、选择、非零滚动及焦点身份，不以编译或私有状态赋值代替操作。

## 交付范围

候选目录为 `target/git-branch-retention-2026-10-19/`。需配套宿主和独立 0.1.15 ZIP，不自动升级用户安装。不提交或推送工作区，不改既有工单状态。已有及并发改动保留，不计为本次成果；完整原生矩阵和系统 IME 候选窗不在本次验收范围内。

| 产物 | 大小 | SHA-256 |
| --- | --- | --- |
| `Nanobug.exe` | 118616064 bytes | `386184dfcc087d53621455d4b1263a4235ccc4fcdc47bac1b2826dd6aa8d51bd` |
| `git-0.1.15.zip` | 7 个归档项 | `2c90e198b7a19989bc1e94e03de18f88bc6199213faf6950a7622ff81940afa4` |
| 包内 `git.wasm` | 2554188 bytes | `546a3d154493bfbb5219aa452e70e3daaaa536f44fe0bca90886eb42dd495e48` |
| `git-for-Nanobug-0.1.15-source.zip` | 179267 bytes，61 项 | `616baf5ddab3345781b22f41847d8ceadd4e099d84c70512ce146c88e4a3a764` |

宿主最终通过 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o <候选目录>/Nanobug.exe` 构建，3m53s；未覆盖运行中的旧程序。最终宿主再次执行独立访客 110 项测试通过，复用的 SDK 缓存为 `4b474b34a680966add27b3d3674bc087bf5ee8c5ac8ec8b21862c8afaed9945e`；独立依赖记录核对相同缓存。

独立源码快照及包副本保存在 `C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.15/`；61 项逐一读回仍与当前源码完全相同，输入清单、逐文件摘要与两个 ZIP 的账本在同目录。原生 helper 448000 bytes，SHA-256 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`，与前版相同。候选目录另有 `identity.json`、`SHA256SUMS` 和使用说明；包安装权限与 0.1.14 相同。

最终日志：`target/git-branch-retention-{manager,menu-green,plugin-ui-final,native-final,tree-handover-final,visibility-final,focus-final,native-menu,nonui,workspace-check-final2,format-final2,diffcheck-delivery,host-build-final2}.log`；独立项目 `target/branch-retention-{unit-final,package}.log`。前述红灯、首次原生发现及修复前的记录保留，不以它们冒充最终结果。
