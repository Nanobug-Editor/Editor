# Vue/TypeScript S1/S5 运行时续接点（2026-10-15 22:51 +08:00）

状态：用户已要求暂停全部工单；代码和证据已保存，停止新增实现、构建与测试。仅归新 [S1 #121](https://github.com/Nanobug-Editor/Editor/issues/121) 与 [S5 #125](https://github.com/Nanobug-Editor/Editor/issues/125)，旧 #108–#118 仅作历史记录。未提交、推送、关闭议题或发布版本。

工作树：`C:/Users/Tmiracle/.codex/worktrees/vue-typescript/Editor`，分支 `codex/vue-typescript`。原工作区没有改动。

## S1：源版本更新撞上未退役冷准备

实际混合包测试先报告 `InvalidState: Abandoned activation is still retiring`，当时尚未编译组件。公开未知语言 Package → Manager 回归精确复现：拿走 detached preparation、取消原需求、立即以当前可用性发起同文档新需求；旧实现直接拒绝新需求，RED 0.03s。

已保存修改：

- `crates/plugin-runtime/src/manager/activation/mod.rs`：增加独立 `deferred` 等待队列与模块连接。
- `crates/plugin-runtime/src/manager/activation/deferred.rs`：新等待与旧 worker 的共享 completion 分离，计入全局 64 项限额；保留原 deadline/取消/源授权，旧 flight 释放后重新验证原 token，再捕获新准备输入。
- `crates/plugin-runtime/src/manager/activation/driver.rs`：running abandoned flight 接受新排队需求；源关闭、配置退休与失效 token 的取消遍历覆盖队列；`take_demand_preparation` 先回收旧工作再推进新需求。旧候选无法由新等待重新获得发布权。
- `crates/plugin-runtime/tests/language_contexts.rs` 与 `language_contexts/retirement.rs`：通过原公开 Manager worker 队列验收正常返回、丢弃 preparation、64 项预算及源关闭取消；没有测试专用宿主 API。

实际执行：

```powershell
cargo test -p plugin-runtime --test language_contexts a_new_source_waits_for_cancelled_preparation_to_retire -j1 -- --nocapture
cargo test -p plugin-runtime --test language_contexts -j1 -- --nocapture
```

定向 GREEN 1/1、0.03s；最后整套常规结果 **10 passed、2 ignored、0 failed，0.12s**。日志为 `target/s1-renewed-demand-red.log`、`target/s1-renewed-demand-green.log`、`target/s1-renewed-demand-suite-green.log`。忽略的实际 SDK hooks/编译预算测试不计本轮通过。最后常规 suite session `90641` 已 exit 0，无该测试进程。

尚未验收：包含此修复的真实 production Worker 混合 TS/Vue 包重跑；实际 WASM activation budget/ABA 回归；工作区完整门禁。不能由 0.12s 通用 case 宣称 S1 完成。主代理在本次暂停前另有 app 构建/原生联验调度，状态以其统一记录为准。

## S5：恢复实例未被采用为已激活 owner

详见 [S5 通用运行时记录](vue-typescript-s5-runtime-2026-10-15.md)。先将旧数据/热更新测试迁移为公开 Command/Language Demand，随后发现真实缺陷：回滚恢复 Main 后，availability record 的 `owner` 已更新但 `materialized` 仍 false，下一角色需求再次激活替代 Main，撤销了刚签发 native plan 的原 owner。

最小生产修改同在 `activation/mod.rs`：发现实际 WASM owner 时将 `materialized` 升为 true，不降级无 WASM owner 的声明式完成状态。临时 `DEBUG-s5-plan` 探针已移除。

实际证据：

| case | 结果 |
| --- | --- |
| managed_dependencies 常规 | 8 passed、1 ignored，2.05s |
| 真实 SDK fuel/内存故障隔离与独立重启 | 1 passed，194.16s |
| 数据升级、准备后追加写入及禁用后再迁移 | 1 passed，150.00s |
| 迁移拒绝/激活失败/commit 取消回滚 | 1 passed，280.78s |
| 混合热更新完整回归 | 冷测试迁移后真实 RED165.02s；探针 RED164.49s；修复后 **1 passed，215.69s** |

热更新最终日志 `target/s5-hot-update-final.log`，session `65560` 已 exit 0。该完整热更新 GREEN 发生在随后 S1 deferred 修改之前，后续最终工作树重跑不能省略。S1 常规 suite 包含最新 deferred 与 materialized 修复并通过，但不替代该实际 SDK 全路径。

测试源修改已保存于 `tests/data_migration.rs`、`tests/hot_update.rs`、`tests/native_modules.rs`。后者仅增加 `NANOBUG_LANGUAGE_DELEGATE_PACKAGE` 选择当前 SDK 构建 ZIP，原固定输入保留。新夹具经宿主 `--plugin-package` 成功构建至 `target/s5-fixtures/language-delegate-fixture-0.1.0.zip`，摘要见 S5 记录；**真实模块生命周期 ignored case 尚未运行**。

暂停前未执行项目：

- 数据迁移 `interrupted_commits_recover_before_any_guest_is_activated`、`enabling_a_dormant_workspace_migrates_and_activates_its_own_data`；源已补 Command Demand，尚未验收。
- `managed_dependencies` 动态 hook 收据/跳过未使用下载的 ignored case。
- 当前 SDK `native_modules` 真实委托、卸载后进程归零、另一上下文不受影响及借出方重新准备。
- 实际 TS/Vue 兼容组合、离线准备、后端退出恢复、原生状态交互及整库门禁。

## 停止与资源状态

本代理所有 runtime Cargo/test sessions 均已结束；最后只读进程核对没有 `data_migration`、`hot_update` 或 `language_contexts` 测试进程。不持有 Cargo/链接槽，没有为了暂停终止其他代理的进程。未启动下一项测试。

已改 Rust 文件执行了定向 rustfmt（`skip_children=true`）；最后 `git diff --check` 对本轮受影响已跟踪文件 exit 0，仅有 Windows LF/CRLF 提示。尚未执行整库格式、非 UI workspace 测试及 workspace check，不能当成已完成门禁。
