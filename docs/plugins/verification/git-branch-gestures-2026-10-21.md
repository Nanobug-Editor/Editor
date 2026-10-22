# Git 分支树双击选择与菜单悬停

日期：2026-10-21。状态：本轮专项行为验收通过，配套宿主与 Git 0.1.23 候选已生成；全仓检查例外见下文。未安装或发布。

## 行为与实现

- 分支叶子单击只聚焦，不改变原生选中行、不发出历史筛选事件；双击提交选择并发出一次筛选事件。实际 checkout 继续走原有菜单、影响预览和确认。
- 非选中行悬停采用 ExplorerRow 主题灰色（内置浅色 `#ebecf0`，深色 `#393b40`），选中行保留原有蓝色。右键菜单打开期间保留目标行的灰色背景；关闭菜单、场景失效及资源撤销清理保留状态。
- 公共 `ui.tree 1.5` 的 `create_with_options(Options { selection: DoubleClick })` 固定资源整个生命周期的选择手势。原 `create()` 仍单击。显式 required 下限、权限、配额和委托归属沿用现行校验；宿主不识别 Git 身份或分支领域数据。
- 独立 Git 包提升到 0.1.23，仅分支树启用双击；暂存、历史文件与冲突文件继续单击。协议版本仍为 7、SDK crate 仍为 0.2.0，未增加权限。虚拟列表方案仍未执行。
- 修复实际菜单回归发现的迟到读取覆盖弹窗：历史局部发布原来只构造内容树，漏带已接受的确认框。现在完整／局部发布共用 `apply_dialogs`，从当前状态生成相同的确认、分支输入、变基或仓库弹窗，不重建兄弟面板或更换已确认的操作目标。

共享外观位于 `crates/editor-ui/src/tree/mod.rs`；宿主原生鼠标与菜单绑定位于 `crates/editor-app/src/ui/plugin/tree.rs` 及 `tree/interaction.rs`；公开契约和准入分别位于 `crates/plugin-protocol/src/tree.rs`、`crates/plugin-runtime/src/instance/trees.rs`。

## 验证依据

全部日志位于 `target/branch-tree-gestures-2026-10-21/`。开始时工作区已有大量其他任务的未提交改动，`baseline/status.txt`、`baseline/workspace.patch` 和本轮主要文件原件保存其依据；`baseline/git-source.zip` 保存独立访客初始来源。未把既有差异计为本轮成果。

红测试使用真实 Git 0.1.22 ZIP，通过公开 Package／Manager 安装和原生控件输入：单击 topic 后，原生选择从 main 改成 topic，`red.log` 中“single click must retain the selected branch”断言失败。构建成功后才执行新测试程序，`red-build.log` 记录对应可执行路径。

SDK 构建宿主 `sdk-host.exe` 和原生测试通过 Cargo Release 构建，为缩短 UI 验证时间仅将 editor-app、editor-ui 的包优化级别设为 0；二者不是标准优化级别的发行候选。独立访客通过该宿主的 `--plugin-cargo` 及 `--plugin-package` 入口构建，不引用仓库 crate 路径，也不复制 SDK 到访客工程。Cargo metadata 确认访客 workspace 为独立 Git 目录，单元测试 target 为其独立 `target/branch-gestures-2026-10-21`，完整包构建使用受管缓存。

最初原生菜单检查中，双击、悬停和菜单目标保留均通过，但点击 switch 后确认框消失。追加的事件观察证明发出的是目标节点的准确菜单槽；增加实际确认框等待仍失败，`native-gestures-diagnostic.log` 和 `native-await.log` 保留依据。公开 typed command 的冷筛选回归同样复现：命令成功，但迟到历史回执清除了弹窗，`cold-menu.log` 记录红例。修复访客局部发布后，同一个原生程序配新 ZIP 通过完整确认及实际 checkout；公开运行时回归亦通过，没有通过延长等待或放宽断言消除失败。

| 检查 | 实际结果与日志 |
| --- | --- |
| 独立访客 `--plugin-cargo ... test --lib --jobs 2` | 132 通过、0 忽略；`guest-tests-final.log` |
| 通用运行时树测试 | 18 通过、2 忽略；`runtime-tree.log`；忽略项未计为通过 |
| 新选择配置的真实 WASM 准入 | 显式 `--ignored`，1 通过；旧 required 范围拒绝且不分配资源，刷新／撤销及授权约束通过；`admission.log` |
| 冷筛选后 typed 菜单与确认框 | 新 ZIP、当前测试源码显式 `--ignored`，1 通过、52.92 秒；`cold-menu-current-source.log` |
| 原生双击、深浅主题悬停及菜单操作 | 1 通过、5.80 秒；检查绘制 quad、鼠标移入菜单后保持、关闭清除、原蓝色选择、禁用菜单、确认及真实 checkout；`native-gestures-final.log` |
| 原生分支默认选择／分页／显隐保留 | 2 通过、24.39 秒；`native-retention-final.log` |
| 原生增量筛选与非空审查 | 1 通过、9.30 秒；`native-incremental-final.log` |
| 原生历史文件单击预览 | 1 通过、10.56 秒，双／三栏比较继续工作；`native-file-selection-final.log` |
| 普通数据库树与场景生命周期 | 单击、键盘、重试／菜单 1 通过；源撤销与恢复 2 通过；`native-database.log`、`native-source-lifecycle.log` |
| `cargo test --workspace --exclude editor-app --no-fail-fast --jobs 2` | 本次运行 432 通过、3 失败、339 忽略；`workspace-tests.log`，见下文 |
| `cargo check --workspace --jobs 2` | 首次被并行报告模块类型错误阻挡，原任务修复后重复同一入口通过；`workspace-check-final.log` |
| 格式／依赖／差异检查 | 访客格式通过；18 个专项 Rust 文件格式通过；全仓格式仍有并行差异；Cargo 依赖树不含 gpui-component；最终 `git diff --check` 通过 |

全仓测试失败为 `capability_packages` 的 `incompatible_required_capabilities_and_base_versions_are_rejected_at_inspection`、`cloud_capability_versions_are_admitted_independently_from_package_version`，以及 `retired_ui_contract` 的 `retired_presentation_and_host_artwork_are_rejected_before_installation`。均为范围外诊断文本断言，与此前比较导航验收中的失败一致；本轮未修改这些测试。全仓格式差异见 `fmt-final.log`；共享 `capabilities.rs` 的主机版本诊断也在本轮中被并行修改，故它的完整文件格式检查仍失败，本轮只修改其 `ui.tree` inventory 一行。`scoped-fmt-final-excluding-shared-admission.log` 明确不包含该共享文件；`instance.rs`、`instance/plugin_services.rs` 的并行整体差异亦不冒充已格式化。

## 候选与源码身份

标准 Release 入口为：

```powershell
cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/branch-tree-gestures-2026-10-21/Nanobug.exe
```

构建成功、5 分 23 秒，未覆盖包优化级别；日志 `release.log`。构建期间有 8 个测试／并行诊断与报告文件变化，清单为 `changed-during-release.json`；本轮生产选择／悬停／协议文件在构建期间稳定。该宿主包含当前共享工作区的其他改动，不能当作冻结源码快照，亦不代表其他任务已由本轮验收。前后输入及根目录记录为 `release-inputs-before.json`、`release-inputs-after.json`、`release-metadata.json`。

| 产物（均在候选目录） | 大小 | SHA-256 |
| --- | ---: | --- |
| `Nanobug.exe` | 79,958,016 字节 | `9459ebc13d047a3a6d95c3bbf2adbe2b1346fdf769668e6ba686bd6aface7fd9` |
| `packages/git-0.1.23.zip` | 887,008 字节 | `7d0ea99e484afafe343902a1cc30baaebbb7a618ae1c4ecee4144579727fe5bd` |
| `sdk-host.exe`（仅 SDK／构建验证） | 97,408,000 字节 | `3a590f3a7754308b9f34b3a99e6751dccd9ab5c08b64148c495ee425207fd531` |
| `host-tests-await.exe`（专项原生测试） | 137,257,984 字节 | `8c3fcd15f89d094b7ebd8938d445a7f853e482ee2a388bb72628db92fdeaea85` |

`artifacts.json` 保存可读回的完整路径。`git-source-0.1.23-final.zip` 保存 70 项独立源码／清单／图标／原生辅助源码；逐项摘要为 `guest-source-final-inputs.json`，不包含构建缓存或 SDK 副本。旧的失败 ZIP 另存 `git-0.1.23-before-modal-fix.zip`。最终包只包含当前 WASM、清单、图标和声明的原生辅助程序；`package-manifest.json` 记录实际包内清单。

## 交付限制

本轮不自动替换用户已安装的宿主或 Git 插件，不提交、推送或发布。上述原生输入／绘制验收来自 GPUI 测试窗口，未宣称对用户当前已运行的安装实例执行了操作。使用本轮行为需要配套宿主及 Git 0.1.23；既有插件数据与安装记录未改写。
