# Git 0.1.49 宿主支持与安装

日期：2026-10-29。状态：接口实现、全部门禁及固定包冷恢复验证通过。实际安装与 Release 运行以项目外 `delivery-result.json` 回执为准。

## 范围

用户已要求补齐主程序接口、更新 Git 到 0.1.49，完成后构建并运行 Release；沿用 `codex/diff-switch-timing-save-20261029` 工作区和本地 main 合并授权，不推送。

- 本阶段起点为 `bf6f6b3e8ff9c35aa8f2f95b33c097f01509f093`，保留文件切换保存去重。
- 固定安装包：`C:/Projects/RustProjects/Editor/dist/git/git-0.1.49.zip`，SHA-256 `eb23bc203a3863981a92920bb68657d9ccf64597c34b656f7ad6109e2a6b8bd8`。
- 该包需要 `ui.native >=1.3,<2`、`editor.diff >=1.5,<2`。它是分栏布局记忆候选，区别于另一份同版本、摘要 `dd2f…e227` 的国际化候选。不得按文件名或版本混用。
- 宿主实现承接归档快照 `357d1904e136f7945849501f26d3467758a93a30` 相对 `1f328863` 的布局功能，原工作区和原插件源码不修改。
- 所有插件通过公开 `Node::split_proportions` 与 `Action::SplitResized` 使用此能力；宿主不保存 Git 专属偏好，不改变包字节或权限声明。

## 验收

通过公开序列化文档验证比例往返、非法数据和旧事件拒绝；通过原生 Base 控件验证真实拖动只在松手产生回执，新窗口及不同视口恢复比例且不写回。真实固定 ZIP 通过公开 Package、Manager、私有存储重新打开，验证布局和文字列宽冷恢复。

Red 已在旧代码实际执行：比例从公开 JSON 往返后丢失，`protocol-red.log` 为 1 failed。补入协议、验证、通用控件和宿主适配后，同一回归在 `protocol-green.log` 为 1 passed。

本机证据：`C:/Users/Tmiracle/.codex/visualizations/2026/10/29/01a18258-60da-76c0-8e33-72186a603e4d/diff-switch-save/git-0.1.49-host-support/`。构建使用本工作区自身 `target/`，合并后使用主目录自身 `target/`。

## 本轮回归与门禁

- `cargo test -p plugin-protocol --lib --locked --offline -j8 ui::layout_tests:: -- --nocapture`：5 passed / 0 failed。覆盖公开 JSON 往返、比例与回执约束及旧格式兼容。
- `cargo test -p editor-ui --lib --locked --offline -j8 split::tests:: -- --nocapture --test-threads=1`：最终 3 passed / 0 failed。包含真实控件拖动、新窗口 800/1400px 恢复、深色纵向恢复和极端比例恢复后的第一次拖动。
- `cargo test -p editor-app --bin editor-app --locked --offline -j8 ui::plugin::code_sync::alignment_tests::split_receipts::source_editor_rail_split_delivers_its_release -- --exact --nocapture --test-threads=1`：1 passed / 0 failed。通过原生 Editor 行侧 Column 检查松手单回执、旧源拒绝、IME 拒绝且组合态保留。
- 设置 `NANOBUG_GIT_CANDIDATE` 为证据目录中固定摘要的 ZIP 后，使用本工作区的 Debug 测试程序显式运行 `extensions::windows_tests::layout_preferences::git_layout_preferences_survive_a_cold_manager_restart --exact --ignored --nocapture --test-threads=1`：1 passed / 0 failed / 0 ignored，118.76 秒；父入口核对独立子进程恰有一个测试通过。
- `cargo test --workspace --exclude editor-app --locked --offline -j8`：最终 864 passed / 0 failed / 446 ignored，127 组汇总。已有 skipped 用例未计为通过，本次实际包用例单独显式执行。
- `cargo check --workspace --locked --offline -j8`、`cargo fmt --check`、`git diff --check`：通过。既有 unused/dead_code 和 Wasmtime 链接警告保留。

关键日志依次为 `protocol-layout-tests.log`、`native-split-final.log`、`source-rail-green.log`、`git-layout-cold-restart.log`、`workspace-tests-final.log`、`workspace-check-final.log`、`fmt-final.log`。

## 审查修复与证明

两条独立审查线覆盖相对 `bf6f6b3e` 的完整未提交差异和新增文件。规范线最终 0 发现；规格线最初发现两项问题，修复复核后 0 剩余发现：

1. 极端比例恢复时，Base 的零移动 resize 会同步旧画面尺寸，破坏上一项投影后的拖动状态。现在跳过已经相等的目标；回归在旧适配代码实际得到错误比例，修复后验证恢复 `[0.8,0.1,0.1]` 后首次拖动得到 `[0.75,0.15,0.1]`。日志为 `extreme-split-red.log` 和 `native-split-final.log`。
2. 行侧分栏原回调丢弃 Window，无法通过来源权限出口。现在沿用 `emit_version_in`，保留 source revision 与 IME 保护。有效 Column 夹具在旧代码 0 回执而失败，修复后松手恰有一个有效回执，旧源/组合输入均拒绝。日志为 `source-rail-red-column.log` 和 `source-rail-green.log`。

首个过宽 Row 夹具未作为行侧回归证明；后续已改用实际可见的 70×300px Column。一次测试上下文类型错误已修复并保存在 `source-rail-green-build-error.log`，不计为测试通过。

## 安装与 Release 交付

为避免同版本候选混用，固定 ZIP 另保存为证据目录的 `git-0.1.49.zip`，摘要与上述共享输出一致。仅导入这个现有包，不修改包字节、SDK 声明或已安装权限。

本地合并提交、主目录 Release 编译命令和结果、静态准入、真实注册表版本及运行窗口来源统一记录在同一项目外目录的 `delivery-result.json`。源码验证不等同于实际安装，必须结合该交付回执判断最终状态。原始较早验证只保留在[历史记录](git-layout-persistence-2026-10-28.md)，不冒充本轮测试。
