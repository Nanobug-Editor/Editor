# 提交按钮响应与提交并推送（2026-10-24）

后续用户已明确改为 IDEA 变更列表模式，0.1.44 的两类勾选均只维护本次提交范围；本页保留 0.1.43 当时的实现与测量，不作为当前勾选契约。现行范围见[后续记录](git-commit-ranges-2026-10-25.md)。

## 范围与来源

用户确认延迟发生在「勾选文件后，提交按钮很久才变为可点击」，并要求在提交按钮右侧增加「提交并推送」。保留已确认的策略：已跟踪文件勾选立即进入串行暂存，未跟踪文件勾选只改变本地选择；提交只消费实际暂存区，不保存或覆盖未暂存内容。

独立插件源码为 `C:/Projects/RustProjects/Git-for-Nanobug`，本轮从 0.1.42 更新到 0.1.43；原生辅助程序从 0.1.0 更新到 0.1.1。宿主生产代码、公开协议和安装权限没有改变。宿主工作区已有大量其他任务改动，不属于本轮交付。

## 原因与修复

原流程在 `git add` 成功后，先读取一次完整状态，再串行启动八次 `git rev-parse` 读取 merge、cherry-pick、rebase 和 sequencer 标记。所有查询完成后才发布新的暂存状态。`commit_button_is_ready_within_two_post_stage_reads` 的初始失败明确记录：按钮等待了 **9 次**暂存后的进程完成。

在 Windows 上复用插件已经声明、独立安装的原生辅助程序，一次读取八个固定标记。辅助程序从实际 checkout 的 `.git` 和 `commondir` 重新解析元数据目录，再与请求中的仓库身份核对；只读、大小受限，读失败或格式异常不能伪装成操作结束。指向缺失对象的有效 OID 仍保留操作身份。reftable 存储及其他平台继续使用原有 Git 引用解析路径。

常规 Windows 仓库的暂存后流程成为一次 Git 状态查询和一次辅助进程，共 **2 次**。仍然等待完整状态确认；两个提交按钮共享实际暂存、非空说明、无冲突、无待处理勾选及完整状态校验。文件树可继续接收勾选，不因此提前开放提交。

「提交并推送」先执行 `git commit -F -`，只有成功回执才能排入普通 `git push`。提交失败时不推送；推送失败或无法启动时说明本地提交已成功并保留。错误使用独立、可关闭且高度受限的弹窗，不恢复永久反馈区。已有「先提交再继续操作」流程仍保留清洁状态复查和重新审阅。

主要改动位于独立插件的 `src/commit_panel.rs`、`src/operation_observation.rs`、`src/repository.rs`、`src/completion.rs`、`src/operations.rs`、`src/processes.rs` 和 `native/src/main.rs`。宿主只新增公开管理器集成测试 `crates/plugin-runtime/tests/git_plugin/index_edges/commit_submit.rs` 及模块注册。

## 测量

真实 WASM 包通过公开管理器，在临时仓库中执行同样的勾选动作；从发出勾选事件计时，到实际提交节点可点击为止。每版三次，排除激活/JIT 编译及测试工具的空闲防抖。测试不设置依赖机器负载的时间断言；进程次数回归单独断言。

| 候选 | 三次耗时（毫秒） | 中位数 |
| --- | --- | --- |
| 0.1.42 | 978、780、823 | 823 |
| 0.1.43 最终包 | 280、264、308 | 280 |

该小样本中位数减少约 66%；这是临时仓库中「勾选到可点击」的实际管理器链路时间，不是所有项目的固定耗时。原始记录位于 `target/commit-submit-2026-10-24/package-before.json` 与 `package-final.json`。首次实包检查的 270、363、236 毫秒另保存于 `package-after.json`，不混入最终包统计。

## 验证与交付记录

本轮使用宿主 `target/release/editor-app.exe --plugin-cargo` 执行独立插件测试，并通过 `--plugin-package C:/Projects/RustProjects/Git-for-Nanobug --output target/commit-submit-2026-10-24/package` 打包。格式检查使用 `cargo fmt --manifest-path ... --check`；`--plugin-cargo` 仅接受 build/check/test，不接受 fmt。

真实包测试使用独立配置、临时本地工作仓库和本地远端，未提交或推送用户工作区：

- 提交成功后推送，远端分支与新提交一致；部分暂存文件只提交暂存内容，未暂存文件内容保留。
- 远端拒绝推送时保留新本地提交，显示明确失败弹窗并可关闭。
- 提交无法获取真实 index.lock 时不创建提交，也不推送此前未发布的本地提交。
- 暂存完整刷新、普通与 linked worktree 的 rebase 身份、无效批次、过期回执、损坏标记及替换仓库目录均有回归覆盖。

最终验证结果：

| 检查 | 结果 |
| --- | --- |
| 独立插件 `--plugin-cargo ... test --lib` | 195 通过，0 失败 |
| 真实包 `cargo test -p plugin-runtime --test git_plugin index_edges::commit_submit -- --ignored --nocapture --test-threads=1` | 3 通过；含两个成功/拒绝子场景、三次延迟采样及提交锁失败 |
| `cargo fmt --check` | 通过 |
| 独立插件及原生辅助程序 `cargo fmt --manifest-path ... --check` | 通过 |
| `cargo test --workspace --exclude editor-app` | 558 通过，406 ignored；跳过项不计通过 |
| `cargo check --workspace` | 通过 |
| `git diff --check` | 通过 |

本轮没有修改 `editor-app` 生产代码；没有将其他任务的宿主原生改动计入本轮。完整日志在 `target/commit-submit-2026-10-24/`：`guest-tests.log`、`package-submit.log`、`workspace-fmt.log`、`workspace-tests.log`、`workspace-check.log`。

最终候选为 `target/commit-submit-2026-10-24/package/git-0.1.43.zip`，SHA-256 为 `e71da8f2188c4839f7313055ae9f54114b15fd9c9cb01ec642c68bdc041696ea`。用户明确回复「已手动安装」；随后只读核对 `%APPDATA%/MeEditor/runtime-plugins/registry.json`，Git 已启用且选择同一摘要，安装包清单为 0.1.43、原生辅助程序为 0.1.1，权限列表与此前一致。

当前原生窗口已可见「提交」右侧的「提交并推送」，两个按钮在同一行，文件树及提交说明仍显示。截图保存为 `target/commit-submit-2026-10-24/native-installed.png`。未在用户仓库执行提交、推送或为验收改变暂存区；真实提交效果与失败恢复在上述临时仓库中验证。此处没有宣称所有项目都具有相同响应时间，也没有将自动化管理器测量冒充屏幕帧率或显示器呈现延迟。

未执行 Git 提交、推送或 Release 发布。构建日志保留原有未使用项警告及 WASM 构建中不使用的原生存储错误常量警告，不将其记为检查失败。
