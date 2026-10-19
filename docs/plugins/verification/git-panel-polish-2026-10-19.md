# Git 面板间距、文件图标与空菜单按钮

日期：2026-10-19。状态：实现、本轮原生回归与候选构建已完成；全仓格式／diff 检查存在下述既有空白问题。

## 变更范围

- 独立 Git 0.1.16 移除历史根节点 6px 内边距，通用原生树取消垂直 4px 留白，保留横向槽位和行内布局。
- 右侧改动文件容器及提交信息滚动内容分别增加 8px 内边距；原有可调分隔线、滚动和文件点击入口不变。
- 通用原生树未指定包内 artwork 时复用资源管理器的文件夹／文件类型图标解析、已启用插件与主题规则；只取显示名称的 basename 构造宿主显示路径，不授予提供者额外文件访问能力。显式分支图标继续按现有规则绘制。
- 通用插件标题栏仅在当前启用贡献确实声明菜单命令时显示菜单按钮；菜单项全部撤回时关闭空菜单。Git 的原生分支右键操作继续保留。
- Git 业务与布局仍在 `C:/Projects/RustProjects/Git-for-Nanobug` 独立项目，宿主没有插件 ID 分支，没有新协议字段或安装权限。

## 验证及产物

| 范围 | 命令／入口 | 结果 |
| --- | --- | --- |
| 独立访客 | 最终候选宿主 `--plugin-cargo ./Cargo.toml test --lib` | 110 通过，6.19s |
| 通用插件 UI | 最终 editor-app 测试二进制 `ui::plugin:: --test-threads=1` | 67 通过，17 ignored，4.17s；忽略项不计通过 |
| 树 artwork／文件图标 | `ui::controls::tree::`、`ui::icons::` | 分别 2、3 通过；含深浅主题文件夹原始色彩 |
| 真实 ZIP 布局 | `extensions::branch_menu_tests::git_headerless_history_native_layout_uses_the_full_body --ignored` | 1 通过，65.40s；深浅主题及 760／1100／1500px，检查主体上下贴齐和分支／日志共用高度 |
| 真实 ZIP 预览与点击 | `extensions::windows_tests::history_preview::git_inline_history_preview_opens_readonly_two_and_three_pane_tabs --ignored` | 最终 1 通过，67.32s；真实目录／JSON／Markdown／文本文件，8px 实测内边距、图标、空菜单按钮缺席，文件点击打开主编辑区双／三栏标签及只读行为 |
| 真实 ZIP 分支保留 | `extensions::windows_tests::branch_retention::git_branch_filter_and_dock_toggles_retain_loaded_native_tree_state --ignored` | 1 通过，78.82s；真实选择、展开、滚动、暂存／Git 面板显隐及重新筛选仍保留页面与原生状态 |
| 非 UI workspace | `cargo test --workspace --exclude editor-app --jobs 2` | 78 组，304 通过，311 ignored，0 失败 |
| workspace 编译 | `cargo check --workspace --jobs 1` | 通过，2m05s |
| 独立插件格式 | 独立项目 `cargo fmt --check` | 通过 |
| 全仓格式／diff | `cargo fmt --check`、`git diff --check` | 均仅报告既有 `crates/editor-app/src/ui/controls.rs:54` 行尾空白；任务开始即存在，未改动无关文件。限定本轮修改文件的 `git diff --check -- <paths>` 通过 |

第一次预览原生检查中，测试扩展的目录把待点击根文件挤出有限视口，导致后续比较标签断言失败；测试先折叠刚展开的目录再进行物理点击，最终完整场景通过。该失败日志与最终日志分别保留，不把首次失败计为通过。以上为 Windows GPUI 控件及布局测试，无桌面截图、系统 IME 候选窗或完整历史八组合矩阵验收。

## 交付身份

候选目录：`C:/Projects/RustProjects/Editor/target/git-panel-polish-2026-10-19/`。宿主通过 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o <候选目录>/Nanobug.exe` 构建，4m19s，未覆盖运行中的旧程序。

| 产物 | 大小 | SHA-256 |
| --- | --- | --- |
| `Nanobug.exe` | 76340224 bytes | `265f4d376ccd0023a935024b36fa47d62f19665d77c271fb702ac4fe1dca9a83` |
| `git-0.1.16.zip` | 825750 bytes，7 项 | `934528c1082793642ddac0a2c00bfb3f8047a4531cd7d928d1f9bd3c1401d95a` |
| 包内 `git.wasm` | 2554165 bytes | `f3e6f1e2bceb98ab5b243687da8b4802f34759fb9b5aebc449ec9f3a4ac314c1` |
| `git-for-Nanobug-0.1.16-source.zip` | 179577 bytes，61 项 | `db902df4ebf3af5b2dc78a4d6939ab9cb21d8fce1bead026167b92501eb2028b` |

独立源码快照及包副本另存 `C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.16/`；61 项逐一读回与当前源码完全相同，目录内有输入清单、逐文件摘要及 ZIP 摘要。安装权限及能力声明与 0.1.15 完全一致。原生 helper 的 SHA-256 仍为 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。

最终宿主执行的访客测试使用公开 SDK 缓存 `4b474b34a680966add27b3d3674bc087bf5ee8c5ac8ec8b21862c8afaed9945e`，依赖记录核对同一缓存。候选目录有 `identity.json`、`SHA256SUMS` 和使用说明。宿主由共享工作区构建，原有和并发修改保留，不计为本轮成果；未自动安装、提交、推送或发布。

日志：宿主 `target/git-panel-polish-{plugin-ui-final,tree-icons,file-icons,native-layout,native-preview-final,native-retention,nonui,workspace-check,host-build,format-final,diffcheck,scoped-diffcheck}.log`；独立插件 `target/panel-polish-{unit-final,package,format}.log`。首次预览失败保留在 `target/git-panel-polish-native-preview.log`。
