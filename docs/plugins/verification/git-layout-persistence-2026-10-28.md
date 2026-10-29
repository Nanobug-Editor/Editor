# Git 内部布局持久化

开始：2026-10-28；完成：2026-10-29。状态：实现、门禁与实际包冷启动回归通过；保留隔离工作区，未提交、合并或安装。

本页保留原任务的历史验证。2026-10-29 当前主线承接及实际安装结果见 [Git 0.1.49 宿主支持](git-0.1.49-host-support-2026-10-29.md)，不能将下列旧日志当作本次重跑。

## 根因及改动

原内部可拖动 Row/Column 仅使用 Base 的窗口内存状态，没有公开比例字段或松手回执。历史文字列宽仅进入热更新 Snapshot，普通退出／启动没有持久化读写入口。宿主外层停靠会话保存不能替代插件内部布局。

- `ui.native 1.3`：`Node::split_proportions` 为全部插件提供初始归一化比例（空数组表示只启用反馈）；`Action::SplitResized` 仅在松手时产生。拒绝子项数量不符、非正数、非有限数、未归一化、未启用、禁用和过期场景事件。
- 原生控件复用锁定 Base 的 `ResizableState`、`resize_panel` 和捕获处理。按实际视口投影，保留最小尺寸；拒绝旧视口的延迟回调，投影后刷新缓存帧。程序恢复和窗口缩放不发出持久化反馈。
- Git 0.1.49 通过现有 `storage.private 1.1`，将各容器 ID 的比例及既有文字列宽保存到工作区私有 `window-layout` 偏好。启动激活工作台前读取；格式版本为 1，最多 32 个分割项，每项 2–16 个有限正数。采用 revision CAS，冲突／写入错误显式提示。损坏或未来格式保留并回退初始布局，Git 仍能启动。
- 布局回执不取消历史查询、不改变提交选择或滚动；历史文字列宽仍遵守紧凑尾列的逻辑像素规则。外层停靠位置和高度继续由宿主会话负责。

## 工作区与验证

宿主复用已清洁的附加 checkout `C:/Users/Tmiracle/.codex/worktrees/imara-diff/Editor`，新分支 `codex/git-layout-persistence`，基线 `1f328863`。Git 独立仓库存在多项前序未提交内容；复制到 `C:/Users/Tmiracle/.codex/worktrees/git-layout-plugin/Git-for-Nanobug` 作为输入基线，原仓库未修改。任务元数据记录了该基线目录，不能将这些既有内容算作本任务新增成果。

- Red：`cold_start_split_proportions_survive_the_public_document` 在原公开契约的序列化往返中丢失比例，得到 `null`。
- 插件 231 项测试通过，覆盖冷重建、嵌套比例、列宽、损坏偏好及布局回执保留查询和选择。
- 原生 `split_drag_restores_proportions_in_a_fresh_window` 通过：真实 Base 鼠标拖动松手只回传一次，在新窗口 800／1400 px 视口恢复实际比例，无缩放写回；深色纵向高度投影回归同样通过。
- `cargo test --workspace --exclude editor-app --locked --offline -j8 --no-fail-fast`：857 passed / 0 failed / 445 ignored。跳过的测试不计入通过。
- 实际 Git ZIP 的 `git_layout_preferences_survive_a_cold_manager_restart` 显式执行通过，1 passed / 0 ignored，最终包耗时 122.02 秒：写入后禁用并销毁 Manager，重新打开同一私有目录，恢复分割比例和列宽。第一轮与全 workspace 重编译竞争时激活需求超时取消，停止编译竞争后原样重跑通过（121.05 秒），未修改公开 60 秒需求期限或放宽断言。
- `cargo check --workspace --locked --offline -j8`、`cargo fmt --check` 和 `git diff --check` 通过。插件通过配套宿主 `--plugin-cargo` 运行全部 231 项测试，0 failed / 0 ignored。保留既存编译及 Wasmtime 链接警告。

候选包按当前打包约定输出 `C:/Projects/RustProjects/Editor/dist/git/git-0.1.49.zip`，SHA-256 为 `EB23BC203A3863981A92920BB68657D9CCF64597C34B656F7AD6109E2A6B8BD8`。要求提供 `ui.native >=1.3` 的配套宿主，已构建的开发宿主为 `C:/Users/Tmiracle/.codex/worktrees/imara-diff/Editor/target/debug/editor-app.exe`。原安装未改动。

`git-layout-plugin.patch` 为相对于已保存源码基线的本任务插件补丁，`git apply --check` 已确认可应用于原独立仓库的现有改动之上；未执行应用。它不混入该仓库前序未提交的比较／导航修复。

原始日志、原插件源码基线及包证据集中在 `C:/Users/Tmiracle/.codex/visualizations/2026/10/28/01a1824f-0938-7f61-8534-8a3a5bd669fc/`，文件前缀 `layout-`，基线子目录 `git-layout-baseline/`。原生测试采用 GPUI TestWindow；实际包用公开 Manager/存储重新打开，未把它表述为人工完成的宿主进程重启验收。
