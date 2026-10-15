# Git 插件暂停与恢复记录（2026-10-15）

状态：用户明确要求暂停，已停止实施和测试。源码、候选包与日志保留在本地；未提交、推送或发布。恢复时先读本记录、[规格](../specs/git-plugin.md)及[实施验证记录](git-plugin-2026-10-15.md)，再核对现场。实施验证记录中的部分旧结果尚未合并本轮结果，最新事实以下表为准。

后续：用户已批准保留本现场并将剩余工作按场景拆分，已发布[两张工单](../tickets/git-plugin-remaining/README.md)（#119/#120）并恢复实施。本文件保留为暂停时的历史快照；恢复后的结果单独记录，不将这里的失败或已通过证据覆盖为新候选结果。

## 已确认范围

主界面采用审查优先布局：左侧暂存提交树，底部 Git 工具栏、分支树和带拓扑的提交列表，主编辑区持续比较。冲突和提交详情使用独立窗口，左侧文件树、右侧两栏或三栏；合并结果编辑借用唯一原始文档。保留暂存、提交、分支、stash、网络操作、合并、变基、交互式变基、挑拣和重置等已确认范围，具体约束以规格为准。

用户最新授权已替代此前“补白延后”：允许在本地 UI 内基于上游公开能力制作新的补白组件，禁止复制或 fork gpui-kit、gpui-base 等上游项目。此项已经实现，不能再次按“display_map 私有，所以不可行”退回延后。

## 现场与产物

- 宿主：`C:/Projects/RustProjects/Editor`，已有大量其他任务的未提交变更，不把整个工作区差异计为本任务成果。
- 独立插件：`C:/Projects/RustProjects/Git-for-Nanobug`，没有 `.git` 或远程。
- 当前候选：`target/git-plugin-final/git-0.1.1.zip`，792597 字节，SHA-256 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`。
- 清单协议 7、SDK 0.2.0、`ui.code_editor >=1.1,<2`；插件版本 0.1.1。原生变基辅助程序仍为 0.1.0，其 SHA-256 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a` 与清单一致。
- 包仅含 `git.wasm`、`manifest.json`、`native/nanobug-git-rebase.exe`。后续尚需修复的是宿主绘制逻辑，未改变访客 WASM；除非修改插件或公开契约，不需要为了该修复重打包。
- 审查基线参考 `1379f58df76622bdf4d6a7c4167469f124d17a0b`，已有审查针对未提交现场，不是固定点至 HEAD 的正式提交审查。

## 补白实现与关键不变量

本地组件为 `crates/editor-app/src/ui/controls/aligned_rows.rs`、`aligned_pointer.rs`、`aligned_editor.rs` 及同名子目录。没有复制上游源码。不可变行槽映射与实际原生软换行几何产生像素补白；补白不进入文本。裁剪原生绘制片段，最终原生挂载保留同一个 `EditorState` 的输入、IME、选择和 Undo；根级 RootPlugin 转发真实指针，输入代理仅转换几何。

主比较接入位于 `editor/comparison/` 与 `editor/source.rs`；插件代码组接入位于 `ui/plugin/code_sync.rs` 及子目录、`containers.rs`、`extensions/surface.rs`。公开契约 `Document.code_alignment` 显式启用 `ui.code_editor` 1.1；1.0 默认行为保留。插件冲突和历史窗口已启用此字段。

每源限制 1 MiB / 2001 行（含 EOF），同场景共享 4M 比较单元；有限测量高度。超限明确退回独立滚动。搜索或代码操作菜单打开时整组暂停补白。独立横向滚动、共享纵向视口、点击、跨补白拖选、分页、轨道拖动和候选框几何均已有原生自动测试。

IME 预编辑只通知、不发送普通 Change 事件，已通过只读 COW Rope 快照与 chunk 身份检测修复；不是第二份可变文本。普通滚动/光标通知不重算，超限内容不保留旧大缓冲。Measuring 阶段也必须保留原始 InputHandler，不能绕过原文档代理。永久卸载使用 retire，暂时切换用取消手势；未完成 MouseUp 必须在原窗口安全释放后再销毁。

## 最新实际验证

| 检查 | 结果 | 日志（均位于宿主 target/） |
| --- | --- | --- |
| `cargo test --workspace --exclude editor-app -j1` | 全通过；普通命令不执行 ignored | `git-alignment-workspace-tests.log` |
| `cargo check --workspace` | 通过；后续修复后需重跑 | `git-alignment-workspace-check.log` |
| `cargo build -p editor-app` | 通过；后续修复后需重建宿主 | `git-alignment-host-build.log` |
| `cargo test -p editor-app align -- --include-ignored --test-threads=1` | 34/34，通过，包含真实原生布局与输入 | `git-alignment-native-tests.log` |
| 已编译 UI runner：`native_code_sync --test-threads=1` | 3/3，通过 | `git-alignment-legacy-sync.log` |
| 已编译 UI runner：`panel_visibility --test-threads=1` | 3/3，通过 | `git-alignment-visibility.log` |
| 宿主 `--plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib` | 55/55，通过 | `git-alignment-guest-tests.log` |
| 当前 0.1.1 ZIP 经公开 Manager、真实 Git 的 ignored 套件 | 17/17，通过，487.57 秒 | `git-alignment-package-tests.log` |
| 当前 0.1.1 ZIP 的独立窗口 ignored 套件 | 2 通过、3 失败，321.28 秒，尚未验收完成 | `git-alignment-work-windows.log` |
| fmt / diff 检查 | 此前通过；最近测试夹具调整后需做最终复查 | `git-alignment-diff-check.log` 等 |

当前包 17 项实际包测试使用：设置 `NANOBUG_GIT_CANDIDATE=C:/Projects/RustProjects/Editor/target/git-plugin-final/git-0.1.1.zip`，直接运行当时已编译的 `target/debug/deps/git_plugin-bedae8640c39723e.exe --ignored --test-threads=2`。恢复时核对构建产生的 runner 文件名，不猜测哈希。

窗口套件使用同一环境变量，运行 `target/debug/deps/editor_app-d4e1859b30e4cb6a.exe extensions::windows_tests --ignored --test-threads=1`。两项通过覆盖刷新/隐藏快照回收和普通比较/编辑/固定标签。

## 恢复时首先解决的三个失败

1. 软换行滚动测试的夹具锚点在滚轮后出屏导致 unwrap(None)。已修改 `extensions/windows_tests/comparisons.rs`：先 reveal 第 030 行，继续检查第 040 行，新增预先保证超过 160px 可见空间的断言。真实滚轮、几何对齐、选择和文本不变断言均保留。仅该测试文件改动，尚未重编译执行。
2. `plugin_work_window_edits...` 和复杂 merge-edges 窗口测试在构造绘制时 panic：`cannot read editor_app::EditorApp while it is already being updated`。已取得 RUST_BACKTRACE=1 实证，日志 `target/git-work-window-borrow-backtrace.log`。调用链：`extensions/surface.rs:228 action_source_current` 内 `parent.read(cx)` ← `ui/plugin/containers.rs:40 layout_node` 构造边缘操作的 current(...) ← PluginView 绘制。独立窗口初始化时父 EditorApp 正被可变借用。尚未修改此生产缺陷。
3. 建议修复：绘制阶段通过已有 `CodeSync.source` 的预先绑定元数据判断边缘操作是否过期，不在绘制时调用读取父实体的权威回调；可在 `code_sync.rs` 增加局部 paint-only helper。保留 `emit_version` 发出操作时对来源/精确 revision 的权威检查。`step_code_groups` 也应只在确实存在旧 1.0 原生语义映射组时调用旧权威回调，1.1 对齐组不需要该回调。切勿以去掉事件权限校验来消除 panic。

上面的第二项已定位，最后一次单跑用现有 EXE，失败 50.29 秒后正常退出，未进行 Cargo 或源码修改。暂停时已确认该 test runner 没有残留进程或文件锁，三个实施子代理均停止。

## 恢复顺序与交付边界

1. 核对工作区及子目录规则，修复绘制期父实体重入；只格式化本次文件，个别 host 文件可用 rustfmt 的 `skip_children=true` 避免递归触及其他任务。
2. 重编译 UI runner，先验证窗口缺陷，再完整运行上述 5 项实际包窗口测试（约 5 分钟）。没有干净的 5/5 前不能宣称完成。
3. 生产逻辑变更后重跑 34 项 align、3 项旧 native_code_sync，执行 workspace check、宿主 build、fmt 和 diff 检查。非 UI / 访客未变时，已经通过的 17 项包测试、55 项访客测试和非 UI 全仓测试无需无理由重复。
4. 将本记录最新结果合并回实施验证记录，更新规格/文档索引状态，区分旧 0.1.0 与当前 0.1.1；保持未发布状态。必要时再做最终审查。

当前没有生产界面截图及人工整体外观验收。GPUI 自动测试覆盖真实布局和输入，不代替深浅主题、缩放和整体外观人工验收；真实右键菜单锚点有自动测试，OS 菜单本身未显示实测。macOS/Linux 未构建，原生资产仅 Windows x86-64。

外部 Git 可并发；内部队列不是整个仓库的独占事务。stash drop 的 CLI 无 reflog CAS，核对 OID/selector 只能拒绝已观察到的变化，不能宣称完全消除外部竞争。
