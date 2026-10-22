# 三个工作区整合与已安装插件更新

日期：2026-10-22，Asia/Shanghai。状态：整合验证进行中，尚未宣称全部交付。

## 范围与保留

用户要求保留全部现有改动，将当前编辑器仓库三个工作区的改动提交并推送，整合到主分支，然后更新全部已安装插件并启动编辑器。

原始文件、删除记录、暂存区、Git 二进制补丁和 SHA-256 清单保存在仓库外 `C:/Users/Tmiracle/.codex/workspace-backups/editor-main-sync-20261022/`。快照逐文件核对稳定后，形成以下普通提交并推送 `origin`；这三项是保存既有工作的快照，不单独宣称旧工作区已通过本轮验收。

| 原工作区 | 保存分支 | 提交 |
| --- | --- | --- |
| main，原 HEAD `fc9dc531` | `codex/main-workspace-20261022` | `d1bdbb6092a1db06f676a209edc4fcc72d87492a` |
| codex/vue-typescript | `codex/vue-typescript` | `f89f9e4bac07096876da41b981eb6379ab95565c` |
| codex/vue-typescript-remaining | `codex/vue-typescript-remaining` | `7fade327b82167523b261ec50cec64084b4d3aa0` |

本地 main 已通过 `git merge --ff-only origin/main` 同步至 `d498beff`。没有强制推送、重写原提交或删除原工作区；旧 stash 保留。

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
| `cargo build -p editor-app --release` | 待执行；沿用 IDE 的实际 profile |

日志保存在上述仓库外备份目录。普通测试不包含 ignored 测试，不把跳过项报告为通过。

第一次默认并发的 workspace 测试在编译阶段遇到 Windows 页面文件不足（`os error 1455`）、内存分配失败及继发的 rustc 内部错误。保持源码、工具链和输出目录不变，只将构建并发降到 1 后，同一套完整测试通过；没有修改系统页面文件或为环境错误改写产品代码。既有未使用代码及 Wasmtime/Tree-sitter 链接警告保留。

## 插件与启动

用户已明确选择更新全部已安装插件，不安装市场中未安装的条目。读取真实用户 profile 后确认共 8 项；Git 为 0.1.27、Markdown 为 0.18.2，其余 HTML/JavaScript 0.3.1、Rust/SVG 0.6.1、TOML 0.2.3、XML 0.2.2 已是本轮读取市场的最新版。Git 0.1.29 与 Markdown 0.18.5 的本地包已找到，待最终宿主验证后沿公开 Manager 更新事务安装并读回状态，再启动 Release 编辑器。

返回[文档总目录](../../README.md)。
