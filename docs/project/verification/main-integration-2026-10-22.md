# 三个工作区整合与已安装插件更新

日期：2026-10-22，Asia/Shanghai。状态：工作区整合、远程推送、已安装插件更新和 Release 启动完成；启动诊断的剩余限制见文末。

## 范围与保留

用户要求保留全部现有改动，将当前编辑器仓库三个工作区的改动提交并推送，整合到主分支，然后更新全部已安装插件并启动编辑器。

原始文件、删除记录、暂存区、Git 二进制补丁和 SHA-256 清单保存在仓库外 `C:/Users/Tmiracle/.codex/workspace-backups/editor-main-sync-20261022/`。快照逐文件核对稳定后，形成以下普通提交并推送 `origin`；这三项是保存既有工作的快照，不单独宣称旧工作区已通过本轮验收。

| 原工作区 | 保存分支 | 提交 |
| --- | --- | --- |
| main，原 HEAD `fc9dc531` | `codex/main-workspace-20261022` | `d1bdbb6092a1db06f676a209edc4fcc72d87492a` |
| codex/vue-typescript | `codex/vue-typescript` | `f89f9e4bac07096876da41b981eb6379ab95565c` |
| codex/vue-typescript-remaining | `codex/vue-typescript-remaining` | `7fade327b82167523b261ec50cec64084b4d3aa0` |

本地 main 已通过 `git merge --ff-only origin/main` 同步至 `d498beff`。没有强制推送、重写原提交或删除原工作区；旧 stash 保留。

实际内容整合提交为 `e3fc1e8f9df14dc6df9f35674e359d54ba693268`；随后 `10f7d7abcc25c82a29927c747a0ab6bde7f3e2a8` 记录两个 Vue 工作区快照的合并关系，前后源码树一致。两项均已推送 `origin/main` 并读回远程提交核对。三个快照分支相对 main 的未合入提交数均为 0，`git branch --no-merged main` 与 `git branch -r --no-merged main` 均无结果。

## 整合依据

远程新增 4 个提交、207 个变化文件，与主工作区后续改动产生 40 个冲突文件。先在仓库外候选目录逐块处理，再回写主工作区：保留 `editor-ui` 提取、文件历史、语言文档租约、焦点与取消隔离、完整工作区事务、工具/HTTP/SDK 等本地改进，同时补入远程安装预检、服务兼容性、选区格式化、补全、恢复和回归覆盖。修复重复模块、旧 UI 路径及不一致的 Worker 参数。

旧 Vue 工作区以自身 HEAD 到快照的 195 个变化路径为审计范围，未将分支落后造成的文件缺失当作删除。41 个路径与原主工作区相同、7 个与远程相同、142 个已有后续版本、5 个为独有路径。恢复两份历史验证文档和两个仍被测试引用的固定 ZIP，并记录真实摘要；旧恢复测试已被更完整的新原生恢复场景覆盖。另迁入独有的跨文档权限及非法后缀原子拒绝回归。

两份旧重命名辅助实现已由本地更完整的 `workspace/refactor` 路径替代，远程完整响应回归迁入现行路径；单文档降级路径也明确拒绝额外 command，避免仅应用合法编辑前缀。Vue 历史版本适配的三份资料及目录入口完整保留为规划和调研资料，不升级为实施或验收结论。

## 本轮验证

使用真实主工作区 `C:/Projects/RustProjects/Editor`，`cargo metadata --no-deps --format-version 1` 确认输出为其自身 `target`。仓库外候选没有借用主工作区 Cargo 输出目录构建。

| 命令 | 实际结果 |
| --- | --- |
| `cargo fmt --check` | 最终源码通过 |
| `cargo test -p plugin-runtime --test language_declarative` | 4 passed，0 failed，0 ignored |
| `cargo test -p editor-app language::navigation::` | 11 passed，0 failed，27 ignored |
| `cargo build -p plugin-runtime --example lsp_fixture` | 通过，供以下真实 stdio 测试使用 |
| `cargo test -p editor-app packaged_rename_rejects_illegal_tail_without_exposing_a_valid_prefix -- --ignored` | 1 passed，0 failed |
| `cargo test -p editor-app recognition_preserves_the_actual_control_language_and_retires_only_mismatches -- --ignored` | 1 passed，0 failed |
| `cargo test --workspace --exclude editor-app --jobs 1` | 108 组，516 passed，0 failed，382 ignored |
| `cargo check --workspace --jobs 1` | 通过 |
| `cargo build -p editor-app --release --jobs 1` | 通过，7 分 49 秒；沿用 IDE 的实际 Release profile |

日志保存在上述仓库外备份目录。普通测试不包含 ignored 测试，不把跳过项报告为通过。

第一次默认并发的 workspace 测试在编译阶段遇到 Windows 页面文件不足（`os error 1455`）、内存分配失败及继发的 rustc 内部错误。保持源码、工具链和输出目录不变，只将构建并发降到 1 后，同一套完整测试通过；没有修改系统页面文件或为环境错误改写产品代码。既有未使用代码及 Wasmtime/Tree-sitter 链接警告保留。

## 插件与启动

用户已明确选择更新全部已安装插件，不安装市场中未安装的条目。真实用户目录为 `C:/Users/Tmiracle/AppData/Roaming/MeEditor/runtime-plugins`，共 8 项。本轮对照官方市场目录及本地独立插件仓库的新包核对，安装后状态如下；新版宿主的插件管理界面已逐项显示这些版本。

| 插件 ID | 更新前 | 更新后 |
| --- | --- | --- |
| git | 0.1.27 | 0.1.29 |
| markdown | 0.18.2 | 0.18.5 |
| html | 0.3.1 | 0.3.1 |
| javascript | 0.3.1 | 0.3.1 |
| rust | 0.6.1 | 0.6.1 |
| svg（界面名称 Image） | 0.6.1 | 0.6.1 |
| toml | 0.2.3 | 0.2.3 |
| xml | 0.2.2 | 0.2.2 |

市场目录读取于 20:57:44，来源为 [官方 catalog](https://nanobug-editor.github.io/Plugin-Marketplace/catalog.json)。Git 与 Markdown 使用本地独立仓库已生成的较新版本，没有因市场发布滞后而降级已安装包。包来源及 SHA-256：

- `C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.29.zip`：`be6fdc4bcd84a63872c6f05867464d05ab85e30a2f5ec4eaa15d2b5c6f9fe404`。
- `C:/Projects/RustProjects/Markdown-for-Nanobug/dist/markdown-0.18.5.zip`：`383a0d5be242eac1c47be54bf219f4c0ba9ce314beb94d5ede0924a39f0aafe0`。

两个包都通过新 Release 宿主的 `--plugin-check <ZIP> --format json`。沿现有 `plugin-runtime/examples/update_installed.rs` 的公开 Manager 安装事务依次更新，均返回成功；该入口要求插件已经安装并启用，拒绝新增权限。更新前完整备份 993 个文件（279.22 MiB），并逐文件验证摘要。更新后读回注册表，确认原有授权、启用状态、项目覆盖及实例作用域保留，另 6 个插件记录完全相同，所有既有私有数据文件字节相同。证据为备份目录中的 `plugins-update-audit.json`、`plugins-after.json`、`check-*-package.json` 和 `update-*.log`，原 profile 位于 `plugins-before/`。

21:15:48 启动 `C:/Projects/RustProjects/Editor/target/release/editor-app.exe`，进程 ID 为 72896。原生窗口恢复 Editor 项目和既有标签页，分支显示 main，Git 提交图及插件管理均可见。启动后 Cargo.lock 曾显示元数据变化，但工作文件与 HEAD 的内容哈希完全一致，刷新 index 后无实际 diff，没有进行依赖升级。

## 启动验收限制

原生 UI 启动成功不等于全部交互无缺陷。本轮看到本地历史提示 `history worker is busy; retry after the pending operation`；Rust 分析器另记录一次约 120 ms 的事件循环耗时警告。问题面板最初显示 main.rs 的 4 个语言诊断错误和 3 个相关提示，涉及两个测试支持路径及 `ActiveTheme`、`builtin_theme` 导入。磁盘上的支持文件存在，主题通过 `pub use editor_ui::theme::*` 导出，同一源码的 app 测试编译、workspace check 和 Release 构建均通过。通过插件管理重启 Rust 插件一次后，问题面板恢复为 0，重启按钮恢复可用；没有修改源码或删除缓存。该现象尚不足以确定原诊断的根因，历史繁忙提示与分析器耗时日志仍保留，不宣称原生交互全部通过。

返回[文档总目录](../../README.md)。
