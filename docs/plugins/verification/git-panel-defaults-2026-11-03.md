# Git 新项目窗口默认关闭

日期：2026-11-03。状态：插件已打包，插件测试、原生回归及仓库检查通过。2026-11-04 按用户合并全部工作区的要求提交宿主测试与文档，并重跑实际包原生回归；本次不安装或发布插件。

## 行为与改动范围

新项目没有保存面板显隐选择时，Git 与提交面板默认关闭。两个窗口按钮继续保留，用户手动打开、关闭后的选择按各项目保存。已有项目的选择继续优先于清单默认值；冲突处理及提交详情窗口原本就默认关闭。

原因是独立插件清单原先省略了两个普通面板的 `default_visible`，公开协议将省略值解释为 `true`。0.1.56 显式声明 `false`，不改变权限、能力、安装 ID、后台观察或宿主生产代码。宿主只增加实际包的原生显隐回归测试。

插件源码：`C:/Projects/RustProjects/Git-for-Nanobug`，当前 `main`；宿主源码：`C:/Projects/RustProjects/Editor`，当前 `main`。沿用本聊天直接修改当前目录、不自动提交的选择。两个目录起始均干净；未提交、推送、发布或更改日常安装。

## 验证

- 修复前，通过宿主 `--plugin-cargo` 运行 `new_workspace_keeps_git_panels_closed`，在 0.01 秒内失败：`staging must stay closed until explicitly opened in this workspace`。
- 修复后，通过同一宿主运行全部插件测试：259 passed，0 failed，0 ignored。
- 通过宿主 `--plugin-cargo` 的 `check --target wasm32-wasip2 -j 1` 通过。
- `cargo fmt --check` 在插件与宿主均通过。
- 已通过宿主 `--plugin-package` 构建 `C:/Projects/RustProjects/Editor/dist/git/git-0.1.56.zip`。
- 原生实际包负控：同一个测试读取既有 0.1.55 ZIP，在 3.83 秒内失败，`git/staging` 实际可见为 `true`，期望为 `false`。
- 原生实际包正控：0.1.56 ZIP 为 1 passed、0 ignored（3.98 秒）。覆盖首次关闭、两个窗口按钮的真实点击、重建应用后的打开／关闭状态恢复，以及项目 A 打开后首次进入项目 B 仍关闭。
- 既有 `panel_visibility` 原生回归：4 passed、0 ignored。
- `cargo test --workspace --exclude editor-app -j 3`：888 passed、0 failed、447 ignored，包含 128 个测试／文档测试目标。ignored 不计入通过。
- `cargo check --workspace -j 3` 通过；`git diff --check` 在两仓均通过。保留既有未使用代码与链接警告。

原生回归命令：设置 `NANOBUG_PANEL_DEFAULTS_PACKAGE` 为候选 ZIP 后，运行 `cargo test -p editor-app --bin editor-app package_panel_defaults -- --ignored --nocapture --test-threads=1`。本轮先用 Cargo 构建测试，再直接运行对应的 `editor_app-e71e2bba8ba751e3.exe` 测试产物，避免占用 workspace 检查的 Cargo 锁。测试经公开 Package/Manager 准入原始 ZIP，随后使用生产 EditorApp、窗口按钮和工作区会话加载／保存路径。此测试检查原生显隐，不执行 WASM 业务和仓库写操作；插件逻辑由插件测试覆盖。

候选 ZIP SHA-256：`A74A0BB2B664B6240785D86299F5AF6BFA11DCDD4057DCFBC101B1A251E5C8B1`。已读取最终 ZIP 核对四个面板均为默认关闭，且权限与能力要求与 0.1.55 一致。更新已有安装后，已保存为打开的项目仍会打开；新默认仅在该项目没有保存选择时生效。

日志：`C:/Users/Tmiracle/.codex/task-artifacts/nanobug-git-panel-defaults-20261103/`。构建输出分别使用各仓库自身的 `target/`，未改系统、依赖或用户数据。
