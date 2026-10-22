# 工作区文件着色与顶部插件入口验收

日期：2026-10-22。对应[确认规格](../specs/workspace-presentation.md)。本地实现及本批 Windows 原生行为已通过；未提交、推送、安装到用户配置或发布。共享工作区同时存在其他任务改动，本记录只归属本批文件着色、顶部入口及分层菜单变更。

## 实现范围

- 公开 `explorer.decorations 1.0`、`ui.workspace 1.0`，资源绑定插件实例及规范化工作区；路径、权限、协商版本、命令声明、图标归属、更新预算及撤销均由通用宿主管理。Git 和独立 Review 消费者使用同一接口，宿主没有按 Git ID 分派业务。
- Explorer 仅调整存在的文件行名称颜色及说明，目录不着色、不新增文件节点。用户设置保存开关、工作区提供者选择、深浅主题三种语义颜色。唯一候选自动记住；加入第二个提供者不会改写有效选择，缺失路径和退役提供者不会静默回退。
- 项目名右侧接受多个插件入口，默认按 `(order, plugin_id, entry_id)` 排序；用户可以排序和隐藏，窄窗收进“更多”。通用原生菜单支持搜索、可折叠组、级联子菜单、检查标记、禁用状态和懒准备动作。
- Git 标题显示实际 HEAD，与历史分支筛选分开。当前、本地、远程、最近分支共享 IDEA 式菜单；当前 HEAD 省略自签出和删除，底部分支树右键复用同一动作生成器。目录单击展开，底部叶子双击筛选历史。
- 新增分支比较、重命名、安全删除、远程跟踪签出等已确认动作；写操作保留既有预览和确认，目标引用及 tip 校验继续生效。重命名和删除后同步所选历史筛选，最近分支只记录成功签出。全部面板隐藏后仍按至少 30 秒间隔刷新本地状态，不自动访问远程。

提交虚拟列表仍只属于此前分析范围，本批没有实施；不新增 worktree 管理、任意修订签出或多根仓库产品能力。

## 候选与消费者

独立 Git 源码位于 `C:/Projects/RustProjects/Git-for-Nanobug`；清单与 Cargo 版本统一提升至 **0.1.25**，SDK `0.2.0`、协议 7，并声明新必需能力和 `ui.tree >=1.6`。通过宿主 `--plugin-package` 构建的实际包：

- `target/workspace-presentation-2026-10-22/package/git-0.1.25.zip`
- SHA-256：`b704b24ce12f21ee6666d4675cb08dd483a9ed520ce4e1a79a4cc02ec5e1f3e0`

普通 Release 宿主候选为 `target/workspace-presentation-2026-10-22/Nanobug.exe`，82,008,064 字节，SHA-256 为 `7e63461fa586df3449dab0b7f130deec78a9f373c9b0919ee5fd90efee8cc361`。最终 `cargo build -p editor-app --release --jobs 2` 已生成并链接 `target/release/deps/editor_app.exe`（12:49:52），随后替换正在使用的 `target/release/editor-app.exe` 因 Windows 文件占用返回 `os error 5`；没有结束用户进程。候选从新链接的独立产物复制，已用 `--export-plugin-sdk target/workspace-presentation-2026-10-22/sdk-delivery` 验证可执行及 SDK 导出成功。构建失败的实际尾因保留在 `workspace-presentation-host-delivery-build.log`，不将整个命令报告为成功。

非 Git 消费者源码为 `crates/plugin-runtime/tests/fixtures/workspace-presentation/`，使用导出的 SDK 经 `--plugin-cargo` 独立构建，没有访客到宿主 `crates/` 的路径依赖。实际组件为 `target/workspace-presentation-fixture-2026-10-22/wasm32-wasip2/release/workspace_presentation_guest.wasm`，原生使用的 ZIP 为 `target/workspace-presentation-2026-10-22/review-package/review-queue-0.1.0.zip`。它发布审阅文件颜色和队列入口，覆盖提供者竞争、禁用动作、动作重新启用及停用清理。

## 实际执行

以下命令均在本机实际执行；日志保存在 `target/`，属于本地验证产物，不随源码发布。

| 检查 | 结果与证据 |
| --- | --- |
| Git 独立访客经宿主 `--plugin-cargo test` | **138 passed**；`workspace-presentation-guest-final-tests.log` |
| `cargo test -p plugin-runtime --release --test workspace_presentation -- --ignored --nocapture --test-threads=1`，设置 `NANOBUG_PRESENTATION_COMPONENT` | **2 passed**；`workspace-presentation-runtime-final.log` |
| 实际 Git Package/Manager `git_workspace_` 两项，设置 `NANOBUG_GIT_CANDIDATE` 后显式 `--ignored` | **2 passed，78.47 秒**；`workspace-presentation-git-manager-verified.log`。包含所有面板隐藏后等待真实计时器更新，不以手动刷新代替 |
| 既有 `git_package_branch_selection_and_panel_visibility_keep_loaded_branch_pages --ignored` | **1 passed**；`workspace-presentation-retention-final.log`。新菜单仍保留加载页和分支筛选行为 |
| `cargo test -p editor-ui menu::action_menu::tests` | **2 passed**；`workspace-presentation-menu-tests.log`。检查输入法组合 Enter 不误触及“更多”子菜单搜索 |
| `git_branch_menu_native_right_click_disables_unavailable_actions --ignored` | **1 passed**；`workspace-presentation-branch-menu-native-tests.log` |
| `git_native_workspace_presentation --ignored` | **1 父测试、2 个真实窗口场景通过，32.01 秒**；`workspace-presentation-native-final.log` |
| `cargo check --workspace` | **通过**，既有警告保留；`workspace-presentation-workspace-delivery-check.log` |
| `cargo test --workspace --exclude editor-app`，较早完整执行 | **461 passed，363 ignored**；`workspace-presentation-workspace-final-tests.log`。ignored 不算通过 |
| 同命令最后一次重跑 | **未通过**；`workspace-presentation-workspace-verified-tests.log`。并行新增的 `editor-history/tests/history.rs` 引用尚不存在的 `SkipReason`、`CaptureOutcome::Skipped`、`Policy` 字段及 `set_policy`，共 9 个编译错误；这些文件不在本批修改范围 |
| `cargo fmt --check`，最后执行 | **未通过**；`workspace-presentation-format-delivery.log`。差异限于并行任务的 `editor-history` 文件及 `editor-app/src/extensions/worker/java_package_tests/rename.rs`，未替其他任务格式化 |
| Git 访客 `cargo fmt --check`，使用导出 SDK 的 Cargo patch | **通过**；`workspace-presentation-git-format.log` |
| `git diff --check` | **通过**，只有既有 CRLF 提示；`workspace-presentation-diff-check.log` |
| 源码、Cargo 清单、锁文件检索及 `cargo tree -i gpui-component` | 没有组件依赖或 `gpui_kit::component` 调用；同名文本仅为禁止依赖的测试断言，Cargo 返回不存在该包 |

原生测试执行前以 `cargo rustc -p editor-app --release --tests --jobs 2 -- -C opt-level=0` 构建测试宿主：仅 app 测试代码关闭优化，依赖仍使用 Release；这与普通宿主 Release 构建区别记录。已验证测试二进制留在 `target/workspace-presentation-2026-10-22/native-test-verified.exe`，原生诊断开关为 `NANOBUG_GIT_NATIVE_WORKSPACE_PRESENTATION=1`，同时设置 `NANOBUG_GIT_CANDIDATE`、`NANOBUG_REVIEW_CANDIDATE` 包路径及 `NANOBUG_GIT_NATIVE_OUTPUT`。

## Windows 可见行为证据

实际使用隔离用户配置和真实 HWND，输入由原生控件路由，截图取自 Windows D3D11 渲染目标。两组均完成以下流程：文件着色和目录排除、实际 HEAD 菜单及自操作省略、历史筛选不改标题、第二提供者不抢占、设置开关和提供者切换、颜色输入与持久化、入口隐藏和移动、700 逻辑像素窄窗溢出及搜索、停用后撤销颜色、入口和旧回调。正常关闭产生 `shutdown_complete: true`。

- `target/workspace-presentation-2026-10-22/native-final/dark-zh-CN-100/`：深色、中文、100%。
- `target/workspace-presentation-2026-10-22/native-final/light-en-150/`：浅色、英文、150%。

每组包含 `evidence.json` 及 `presentation-files.png`、`presentation-head-menu.png`、`presentation-settings.png`、`presentation-overflow.png`、`presentation-withdrawn.png`。已人工查看两种主题的关键菜单和设置截图。缩放仅对测试自己的窗口发送 DPI 变化，未修改操作系统显示设置；未验收操作系统输入法候选窗口，不将本次窄场景视为整个 Git 原生矩阵重跑。

## 审查与边界

以 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 为固定点，Standards/Spec 两轴只审查本批可识别变更。已修正树更新丢失菜单图标、图标累计预算检查、溢出搜索遗漏子项、菜单本地 ID 拼接碰撞和延迟回调捕获、分支元数据写操作遗留历史筛选、禁用动作重新启用仍持有失效权限、提供者候选未按实际协商过滤等问题；相应实际包、控件及原生回归通过。没有尚未解决的本批确认项。

最终全仓测试和格式门禁受以上并行改动影响，因此不声明整个当前共享工作区全部门禁通过。macOS/Linux、系统输入法候选窗口和未显式运行的 ignored 场景不在本次验证结果中；没有改变用户正在运行的宿主、既有安装或私有数据。
