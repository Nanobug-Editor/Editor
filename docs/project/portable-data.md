# 安装根目录的数据存储

日期：2026-11-03。存储路径、迁移与回归实现已落到当前 `main` 工作区，未提交；格式、workspace 和相关宿主测试通过。原生保存／恢复已验证，完整原生矩阵受下述既有断言不匹配阻断，不能标为全量验收通过。

## 路径契约

默认根由实际运行的可执行文件位置确定，为 `<可执行文件所在目录>/`，与启动工作目录、打开的项目无关。安装版默认对应 `%LOCALAPPDATA%/Programs/Nanobug/`；便携复制时携带完整安装目录及其中的数据。源码构建使用其实际 `target/<profile>/`，仍遵守各 checkout 独立构建输出的规则。

本轮用户进一步明确：不使用外层 `data/`。因此 `runtime-plugins/`、`history/`、`cache/`、`run/`、`gui/` 与程序文件直接并列；设置 JSON 直接写入该根目录。插件内部的私有 `data/` 是插件存储结构，不是宿主外层数据根。

| 内容 | 相对所选数据根的位置 |
| --- | --- |
| 快捷键、设置、项目记录及工作区会话 | 根目录中的既有 JSON 文件，保持原格式 |
| 插件包、权限、私有数据、依赖、市场缓存 | `runtime-plugins/` |
| 文件历史及其设置 | `history/`、`history.json` |
| 运行配置与终端快照 | `run/` |
| 插件开发实例 | `plugin-development/` |
| 编译后的语法、访客 SDK、插件构建缓存 | `cache/grammars/`、`cache/plugin-sdk/`、`cache/plugin-build/` |
| 文件转移操作的临时恢复副本 | `cache/tmp/` |
| GUI 实例通信与租约 | `gui/` |

GUI 的 `--profile` 与 `ME_EDITOR_PROFILE_HOME` 仍显式选择隔离 profile，缓存也跟随该 profile。普通启动不会向子进程注入 profile 覆盖，因此从终端启动另一份安装仍按新可执行文件定位数据。独立插件工具 CLI 的构建缓存使用调用进程的 profile，其 `--profile` 参数指定开发实例的数据目录。`ME_EDITOR_PLUGIN_HOME` 保留显式插件存储覆盖。数据目录不可写时显示启动错误，不回退到 AppData 或系统临时目录。

项目文件、用户选择的导出文件和插件 ZIP 仍写入各操作指定的目标。原子保存的临时文件仍必须位于目标文件旁以保证同卷替换；这些是编辑项目的输出，不是额外的宿主数据根。系统凭据库、外部 Git/Rust 等用户工具链的既有配置和下载缓存维持原归属。

## 旧数据迁移

首次普通 GUI 启动在启动任何 profile 消费者前准备目标目录。显式隔离 profile 和独立工具 CLI 不自动导入用户数据。

1. 获取目标迁移租约及旧 GUI、插件数据事务、历史写入租约；旧实例仍运行时提示正常退出并重试。
2. 将旧配置根（Windows 为 `%APPDATA%/MeEditor`）及旧历史根（`%LOCALAPPDATA%/MeEditor/history`）复制到目标内的临时目录。保留文件内容与历史迁移所需的修改时间，不复制旧 GUI 端点。
3. 完整文件和目录逐项发布；已有目标文件保留，文件与目录类型冲突明确报错。已有非空的历史、插件、运行和开发实例存储整体保留，避免将两份数据库／注册表及事务日志混合；对应旧数据仍在源目录备份中。失败可重试，旧源文件始终保留。
4. 全部发布后原子写入 `legacy-import-v1.complete`。之后不再次导入，避免用户删除的设置或卸载的插件被旧备份恢复。

迁移拒绝跟随数据子目录中的符号链接或 Windows reparse point，防止复制边界之外的文件。旧缓存留作原目录内容，新位置按需重建。迁移不改动插件协议、授权和实例隔离，不重新安装插件或启动旧协议包。

升级安装只覆盖安装器拥有的程序与资源；卸载保留运行时生成的数据文件和目录。发行载荷不得收录开发者的个人数据目录。

## 验证状态

安装根目录修正后重新执行：格式检查、workspace 编译检查、workspace 测试（887 passed、447 ignored）、6 项迁移回归及下表全部宿主定向测试（合计 63 passed、5 ignored）通过。新增断言确认不生成宿主外层 `data/`，且迁移不会覆盖安装根目录中已有的程序文件；对应本机日志使用 `root-` 前缀。372 个本地文档链接与 `git diff --check` 通过。下述原生矩阵是上一阶段记录，本次只调整默认根路径，未重新执行该矩阵。

构建前及首次全量失败后均核对 `cargo metadata --no-deps --format-version 1`：源码根为 `C:/Projects/RustProjects/Editor`，编译输出为该 checkout 自身的 `target/`，没有 `CARGO_TARGET_DIR` 覆盖。首次默认并发的 workspace 测试遇到 Windows `os error 1455`（分页文件不足，无法 mmap 依赖），随后使用单任务编译成功重跑，未清理或改动系统配置。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo test --workspace --exclude editor-app --jobs 1` | 887 passed、0 failed、447 ignored；ignored 未计为通过 |
| `cargo check --workspace --jobs 1` | 最终源码通过 |
| `cargo test -p editor-app --jobs 1 portable_profile_consumers_share_one_root` | 1 passed；子进程分别验证可执行文件旁默认根与显式 profile，实际生成 SDK 缓存和临时目录 |
| `cargo test -p editor-app --jobs 1 app::session` | 13 passed |
| `cargo test -p editor-app --jobs 1 editor::history` | 29 passed |
| `cargo test -p editor-app --jobs 1 plugin_development` | 9 passed、1 ignored |
| `cargo test -p editor-app --jobs 1 terminal::tests` | 11 passed、4 ignored |
| 本地 Markdown 链接、`git diff --check`、初始改动反向检查 | 通过；保留任务开始时已有插件修复 |

workspace 中的新增回归覆盖真实历史内容迁移、实际资源插件包迁移后重新加载和启停、现有文件与完整存储保留、失败重试、运行中实例阻止迁移、目录链接边界和隔离 profile。

原生 Windows/D3D11 命令为 `cargo test -p editor-app --jobs 1 history_native_matrix -- --ignored --test-threads=1`，设置 `NANOBUG_HISTORY_NATIVE_OUTPUT` 为项目外证据目录。深色中文 100% 场景已实际验证三处历史菜单入口、历史只读／当前可写对比、保存／恢复／Undo／Redo、IME preedit 取消、删除保护与恢复后普通保存，并保存截图。OS 输入法候选窗未验证。

该矩阵随后在 `diff limit changed for 2001-lines` 失败，未继续浅色／缩放和最终正常关闭流程。任务起点 `11c11cca` 已同时包含核心 `MAX_SOURCE_LINES = 100_000` 与旧原生夹具的“2001 行必须拒绝”断言；两文件均无本次改动，故保留原失败证据，不扩展为修复差异限额任务。

本机证据：`C:/Users/Tmiracle/.codex/task-evidence/nanobug-portable-data-20261103/`，含首次失败、单任务重跑、宿主定向日志及 `native-history/dark-zh-CN-100/evidence.json` 和 PNG。最终 Debug 程序位于 `C:/Projects/RustProjects/Editor/target/debug/editor-app.exe`。未替换用户已安装的程序，也未主动迁移用户真实数据；运行新版的普通 GUI 入口才执行旧数据导入。未构建 Release 安装包或验收 macOS/Linux。
