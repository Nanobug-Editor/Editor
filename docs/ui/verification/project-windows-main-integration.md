# 项目窗口方案主分支集成验证

日期：2026-10-13。用户明确要求将 `codex/windows-project-entry` 合并到 `main`、解决可能冲突、验证后推送 GitHub。快进合并、针对性验证及完整 UI 基线核对已完成，已普通推送 main 并核对远程 SHA。

## 合并输入与现有改动

- 原本机 main：`ee59a5692a95e9c32f1cea9948497f4ebff393c9`；抓取后的 origin/main：`d1a81f9b33f3ce51d8f6b963990f1d74c9531b42`，是本机 main 的祖先。
- 目标分支本机与 origin 均为 `d640d50991d7f5454efb8c2017ad7ed7c46d760f`；本机 main 是其祖先，故 `git merge --ff-only codex/windows-project-entry` 保留全部 14 个原有提交，没有生成代码冲突或重写历史。
- 合并后的 main 与目标分支树均为 `84755510a92fc2a4fd159d528ec2bf36078b7869`，源码、依赖和安装器相同，复用三单已有双轴审查及真实 Windows 行为证据。
- 最初 `Cargo.lock` 显示修改，但规范化 blob 与原 main 同为 `fd1f7dd4149d78c422bf491b5e325f0cc8d28469`；仅行尾差异仍导致 Git 拒绝覆盖。原文件先备份到仓库外，再单独刷新该项索引，确认未暂存任何内容变化后快进；未丢弃用户代码。
- 检查期间主工作区出现另一项插件界面重构的修改与新文件。它们未提交到本次集成，且未修改、暂存或撤销；后续使用既有干净工作区的 `codex/project-windows-main-integration` 验证与准备文档，该候选与已合并 main 的提交完全相同。

## 编译检查的差分诊断

主工作区第一次 `cargo check --workspace --locked -j 1` 返回 10 个错误：未提交 popover 缺少 trait 引入，以及 `directory_identity`、Manager 并行/安全收尾接口不可见。不能把它们直接归为合并缺陷，也不能将包含另一项修改的工作区结果当作本次提交通过。

在干净且树相同的隔离工作区运行同一命令，popover 错误消失，仍出现 9 个依赖接口缺失；源码中公开模块和方法均存在。反馈循环缩短到约 6 秒，记录 `isolated-workspace-check.log`。随后只刷新隔离工作区 `platform-windows/src/lib.rs`、`plugin-runtime/src/lib.rs` 的时间戳，逐项确认 git 内容哈希前后相同；Cargo 重新检查依赖后，全部缺失接口错误消失，同一命令在 5.33 秒内通过。没有为缓存现象修改产品接口或清空共享 target。

差分结果证实旧的检查产物复用导致接口不可见；主工作区另外出现的界面代码属于并发未提交工作，不混入本次合并。Cargo 的 target/build 输出及 dep-info 边界参见[官方 Build Cache 文档](https://doc.rust-lang.org/cargo/reference/build-cache.html)；根因结论来自上述真实红/绿检查及源码哈希对照，而非仅凭文档推测。

## 验证命令与结果

使用现有 Rust/Cargo `1.98.1`，Windows x64，`CARGO_TARGET_DIR=C:/Projects/RustProjects/Editor/target`，Cargo 单 job；没有安装或升级工具。

| 命令与对象 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --check`，合并后干净 main | 通过 | `main-fmt.log` |
| `cargo test -p editor-app --locked -j 1 external_ -- --test-threads=1`，合并后 main | 15 passed、0 failed、3 ignored | `main-external-tests.log` |
| `cargo test --workspace --exclude editor-app --locked -j 1`，合并后 main | 267 passed、0 failed、255 ignored | `main-workspace-tests.log` |
| `cargo check --workspace --locked -j 1`，相同提交的干净隔离副本、依赖检查产物刷新后 | 通过 | `isolated-workspace-check-refreshed.log` |
| `cargo test -p editor-app --locked -j 1 -- --test-threads=1`，相同提交的干净隔离副本 | 533 passed、7 failed、230 ignored，143.34 秒；退出码 101 | `isolated-ui-full.log` |

完整 UI 的失败清单与[此前干净 main 对照](project-home.md)、[最终分支验收](windows-project-entry.md)完全相同：状态 popover、Rust 超时/状态指示器三个用例，Ctrl/中键定义导航三个用例，以及 Worker 命令归属错误文案用例。没有把完整 UI 记为全绿，也未修改这些已有缺陷；本次提交没有新增失败。`isolated-ui-exit.json` 保存实际候选 SHA 和退出码，交付前按七个完整测试名称逐项核对。

日志与原 Cargo.lock 备份保留在 `C:/Users/Tmiracle/.codex/visualizations/2026/10/12/01a12e28-69b5-7ea2-b14e-f155ef6bf763/main-integration/`。未执行的 ignored 不计通过；真实 WASM/原生程序显式场景与安装/资源管理器联合验收复用[三单完整记录](windows-project-entry.md)，输入未改变，不机械重复整个系统安装流程。

源代码完整性及补丁检查完成后，文档提交 [`239358a75ba215ccf1c49291cd09136509158174`](https://github.com/T-miracle/Nanobug/commit/239358a75ba215ccf1c49291cd09136509158174) 先在隔离副本建立，再快进到本机 main，普通推送 `origin/main`；`git ls-remote` 读回同一 SHA，且 `codex/windows-project-entry` 是 main 的祖先。后续仅补本段关闭记录，不改变受测源码。

更新主工作区时，对当时 17 个并发修改/新文件记录更新前后 SHA-256，全部相同；没有混入本次提交。主工作区仍可保留另一任务的未提交修改，不把其工作区状态称为干净或已验证。证据为 `parallel-work-before-main-docs.json`、`parallel-work-after-main-docs.json`；最终推送再次核对 SHA 后交付。
