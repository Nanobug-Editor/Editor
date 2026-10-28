# 启动与文件监测突发事件的历史任务修复

日期：2026-10-27。用户反馈消息中持续出现“本地历史未能更新：history worker is busy; retry after the pending operation”。

## 诊断

`Worker::call` 在构造 Future 时立即提交，原有队列最多接受 16 个任务。主窗口恢复标签和一次 `apply_reconciliation` 在同一同步调用中逐条提交；此时背景调度器还未消费，普通的多文件突发也会耗尽槽位。`Busy` 是实际拒收，并非仅诊断文本，因此不能通过屏蔽消息解决。

真实 `new_with_session` 恢复 24 个小文件的回归，在原实现稳定失败于相同的 Busy 警告（0.22 秒）。没有替换历史存储或新增测试专用宿主接口。

## 修改边界

- 启动文档恢复与一次文件监测结果各使用一个同步历史批次。在初始化检查之前立即入队，关闭时先提交批次，再等待已有 FIFO 屏障。
- 批内按顺序逐条执行，分别返回成功、跳过或失败；不按路径合并，也不在执行时重读已经捕获的保存／外部变化内容。A/B/A 保留三个时间点，一项失败不阻断后项。
- 回执继续校验原文档 incarnation 和路径；Saved 的 identity、恢复来源、策略跳过以及真正失败的可见提示保持原语义。
- 每批最多 4096 条，内容加保守估计的路径及回执元数据最多 64 MiB。超限仅拒绝新条目，已接收前缀仍提交。提交仍通过 Worker 原有的 16 槽、共享 64 MiB 预留与取消计费检查；没有无界重试或扩大 worker 队列。
- 观察调度移入 `editor/history/recording.rs`；存储格式、历史策略及用户已有数据不变。

## 自动验证

| 入口 | 实际结果 |
| --- | --- |
| `cargo test -p editor-app history_ --locked --offline -j 4 -- --nocapture --test-threads=1` | 40 通过、0 失败、14 按原标记忽略 |
| 其中 4 项新增回归 | 24 标签全部捕获；24 删除事件加文档更新；A/B/A 与关闭屏障；首项真实 I/O 失败后继续捕获，均通过 |
| 固定 capability-example Package/WASM 的 `history_public_wasm_save_records_the_exact_successfully_written_bytes --ignored` | 1 通过，保存回执与历史字节一致 |
| `cargo test --workspace --exclude editor-app --locked --offline -j 4` | 831 通过、0 失败、441 按原标记忽略 |
| `cargo fmt --check`、`cargo check --workspace --locked --offline -j 4` | 通过 |
| Release 候选构建 | 通过，2 分 08 秒；与正式运行文件 SHA-256 一致 |

日志：本机 `target/history-busy-*.log`。忽略项没有计为通过。

## 原生验收限制

显式运行 `extensions::worker::native_acceptance::local_history::history_native_matrix --exact --ignored --nocapture --test-threads=1`。首次深色中文 100% 的历史场景及内部清理结果均为 Ok，但进程退出时，`accesskit_windows 0.34.0` 的异步 `BoundingRectangle` 回调因 `HRESULT(0x80070578)` 无效窗口句柄导致进程 `0xc0000409` 退出。

单独复验时深色中文 100% 子进程完整通过，浅色英文 150% 的历史场景与内部清理结果同样为 Ok，但其进程退出再次出现上述异常。因此原生矩阵整体仍记为失败，没有把场景证据或 `evidence.json` 的 Ok 当成父进程通过。失败日志保存在 `target/history-busy-native.log`、`target/history-busy-native-retry.log`；两次各场景的证据在同名目录。未在本次调度修复中扩展修改 AccessKit 依赖。

## 正式运行文件复查

正式文件：`C:/Projects/RustProjects/Editor/target/release/editor-app.exe`。SHA-256：`31EA8F7C11F03B1A594D0F064E0C8B4E97D6E8F7C3D5EEBEE5B86E36A477F036`。原项目窗口已关闭后更新文件，由原生应用启动入口启动，当前进程为 `53036`。

在用户默认 profile 和同一项目的中文浅色 100% 配置上，通过 Computer Use 实测恢复原有五个文档、等待语言服务启动、打开文件历史并读取已有内容，再查看工作区历史。消息列表保持“暂无宿主消息”，清空按钮不可用；工作区列表出现本次启动在 `2026-10-27 15:56:50.014` 实际捕获的 `crates/editor-app/src/main.rs` 新版本。关闭历史后正常返回编辑界面，消息仍为空。

工作区为 `C:/Projects/RustProjects/Editor` 的 `main`，沿用用户“保留改动、不自动提交”的选择，保留此前插件与消息边框修复。

关联：[基础文件历史方案](../specs/local-history.md)、[历史目录](../README.md)。
