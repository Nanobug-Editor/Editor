# 本地比较迁移到 imara-diff Histogram

日期：2026-10-28。状态：实现、完整门禁及实际 Git 包交互验证通过；用户随后授权直接提交并快进合并到 main，本轮按要求不再测试或构建，不推送或发布。

## 范围和依据

用户确认采用真实 Rust、YAML、Cargo.lock 基准后，将本地比较算法替换为 imara-diff。基线为 `7edaa2bee5b14f5ea17b6d3d604a5e581ce1ae6e`；任务 worktree 为 `C:/Users/Tmiracle/.codex/worktrees/imara-diff/Editor`，分支 `codex/imara-diff`，开始时无改动。

原实现按每侧 2,000 行及 400 万格拒绝输入。实际 `app.yml` 有 2,600 余行，文件预览准入失败后仍保留上一文件，下一文件导航再次指向同一个失败目标。扩展名不参与比较判定。除差异标记外，原生行对齐还有独立 LCS；插件同步滚动及三栏比较也检查旧预算。

本轮将比较与对齐统一到 `editor-core::comparison`，固定 imara-diff 0.2.0，使用 Histogram 和 `postprocess_lines`。宿主提供所有消费者共用的文本机制，Git 插件继续负责提交、对象读取和文件顺序；未修改 Git 插件或能力协商。

- 比较键忽略 CRLF/LF 差别，保留末尾换行缺失和孤立 CR 的意义；返回原始 UTF-8 字节范围。
- 行对齐保留原生 EOF 光标行、三栏共同插入、折叠和几何映射；只借用不可变文本，原 EditorState、选择及 Undo 仍由既有宿主会话拥有。
- 每侧保留 1 MiB 上限，最多 100,000 个文本行；对齐另允许一个原生 EOF 行。
- 共享预算为 400,004 个输入行次，按每次左右输入行数之和收费，包含相同前后缀及三栏插入桶。缓存命中不重算；模型退休归还预算。这是资源准入额度，不是严格执行时间上限。
- 实际包回归暴露了独立的 240,000 px 原生布局上限：Rust/YAML 通过，但万行锁文件无法对齐。将上限调整为 8,000,000 px（低于 2²³，仍保留至少半像素精度），同一回归随后通过；超高缩放/换行仍有明确回退。
- `editor.diff` 的版本、权限、来源归属、revision 和取消检查保持现行契约，仅放宽可接纳规模并更新公开源码注释。

## 验证记录

- Red：`large_comparison_aligns_ten_thousand_rows_without_matrix_budget` 在原实现返回 `SourceRows { pane: 0 }`。
- Green：共享比较 4 项测试通过，覆盖真实项目三种文件、Unicode、CRLF、末尾换行、输入边界及预算。
- Green：原生行对齐 21 项测试通过，包括 10,500 行插入、三栏共同插入、原始行顺序、折叠与坐标往返。
- 新增实际包回归 `git_adjacent_large_rs_yaml_lockfiles_keep_navigation_and_alignment` 显式执行通过（1 passed / 0 ignored，85.80 秒）。通过 Git 包、公开 Manager 和 GPUI TestWindow 验证 `a.rs → b.yml → c.lock → b.yml → a.rs → b.yml`、完整文本、对齐及资源退休。Rust/YAML/锁文件分别为 3,090 / 2,644 / 10,226 行。测试更改第一行标记，完整正文来自当前项目。
- 实际独立包为 `C:/Projects/RustProjects/Editor/dist/git/git-0.1.48.zip`；SHA-256 为 `B51BC8A7397717A60A272B0A14D38CE646EDF81E5E3A363F0DC5CF488A082E38`。未重打包或修改插件。
- `cargo test --workspace --exclude editor-app --locked --offline -j8 --no-fail-fast`：853 passed / 0 failed / 445 ignored；布局上限调整后的最终复查结果相同。未执行的 ignored 测试不算通过。
- `cargo test -p editor-app comparison --locked --offline -j8 -- --test-threads=1`：12 passed / 0 failed / 13 ignored。第一次编译发现刷新路径仍返回旧的 `Err(())`，已改用共享错误类型，重跑通过。
- `cargo test -p editor-app code_sync --locked --offline -j8 -- --test-threads=1`：5 passed / 0 failed / 14 ignored。
- `cargo check --workspace --locked --offline -j8`、`cargo fmt --check`、`git diff --check` 全部通过。编译仍有既存的 unused/dead-code 和 Wasmtime 链接警告，本轮未顺手清理。
- 显式执行 `cargo test -p editor-app git_native_comparison_probe --locked --offline -j8 -- --ignored --test-threads=1 --nocapture`：1 passed / 0 ignored，229.59 秒。`NANOBUG_GIT_CANDIDATE` 指向上述包，`NANOBUG_GIT_NATIVE_COMPARISON_ONLY=1`，`NANOBUG_GIT_NATIVE_OUTPUT` 指向证据目录内的 `imara-native/`。
- 真实 Windows/D3D 窗口两组均通过：`dark-zh-CN-100`、`light-en-150`。验证比较预览不新增普通文件标签、原编辑器焦点与预编辑、取消/提交/Undo、真实滚轮、固定表单及正常退出。两份 `evidence.json` 的 `shutdown_complete=true`，没有 unexpected error logs；已查看深色截图与浅色 150% 预编辑截图，插入空隙及中文/emoji 高亮对齐。
- IME 验证覆盖原生输入处理链，不包含系统候选窗人工验收（记录中的 `os_ime_candidates_verified=false`）。真实窗口用的是既有小文件交互夹具；万行文件通过实际包 + GPUI TestWindow 回归，不将两者混为同一个桌面性能测试。

基准和实现日志位于项目外 `C:/Users/Tmiracle/.codex/visualizations/2026/10/28/01a1824f-0938-7f61-8534-8a3a5bd669fc/`。`diff-bench/` 保存此前 7 组真实 Git 版本、28 组合的测量、补丁和复现说明；那些是独立算法基准，不冒充完整原生视图性能结果。
