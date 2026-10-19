# Vue/TypeScript S5 通用运行时验证记录（2026-10-15）

本记录仅服务新场景 [S5 #125](https://github.com/Nanobug-Editor/Editor/issues/125)，不推进或关闭旧 #108–#118。工作目录为 `C:/Users/Tmiracle/.codex/worktrees/vue-typescript/Editor`。以下未知语言、固定 SDK 包证明通用机制，不能替代实际 TypeScript/Vue 包的组合验收；S1/S5 的原生联合测试由主代理另行记录。

用户于 22:51（+08:00）要求暂停全部工单，已停止后续实施与测试；精确源码、session 和未验证断点见 [S1/S5 续接点](vue-typescript-s1-s5-checkpoint-2026-10-15.md)。

## 已执行

| 命令 | 实际结果 | 可证明的边界 |
| --- | --- | --- |
| `cargo test -p plugin-runtime --test managed_dependencies -j1` | 8 passed，1 ignored；运行 2.05s | 私有依赖目录、缓存/离线重用、坏摘要/网络失败/取消保留旧版本、等待共享锁时取消、独立工作区租约、显式工具路径不回退、安装同意及非法依赖闭包 |
| `cargo test -p plugin-runtime --test fault_recovery wasm_fault_is_scoped_visible_and_independently_restartable -j1 -- --ignored --nocapture` | 先 RED：冷安装后直接 invoke 返回 `Plugin is not running`；按公开原始 Command Demand 迁移测试后 1 passed，运行 194.16s | 真实 SDK 访客的 fuel/内存故障、原生请求失效、独立恢复、私有数据保留、健康同级实例的身份和命令继续有效；不是实际 Vue/TS 后端退出验收 |
| `cargo test -p plugin-runtime --test data_migration upgrades_private_data_on_an_isolated_copy -j1 -- --ignored --nocapture` | 首轮 RED：冷安装后直接调用 `scope-write`，`Plugin is not running: capability-example`；显式 Demand 后 1 passed，运行 150.00s | 准备期间旧实例后续写入成为最终迁移输入、源项目不变、已禁用版本与另一元数据管理器的后续升级保留数据；历史 eager 假设已移除 |
| `cargo test -p plugin-runtime --test data_migration failed_migration_activation_and_commit_preserve_old_data -j1 -- --ignored --nocapture` | 首轮 RED：冷安装不再执行业务激活，与旧 install 失败预期冲突，28.72s；按原始命令首次激活后 1 passed，运行 280.78s | 首次业务激活失败没有 live owner；迁移拒绝、运行中候选激活失败、持久提交边界取消及重新打开均保留旧版本/原数据 |
| `cargo test -p plugin-runtime --test hot_update mixed_update_rollback_replaces_all_owners_and_preserves_latest_data -j1 -- --ignored --nocapture` | 首轮 RED：`service-consumer` 冷实例直接 invoke，55.60s；显式需求后 RED165.02s 暴露真实恢复缺陷；修正 `materialized` 后 1 passed，215.69s | 旧 native/request/service ownership 失效、新实例与新 stdio 握手、最新私有数据保留、重新打开插件服务、更新与恢复同时失败时保留两条错误 |

日志分别保存在工作树 `target/s1-fault-isolation-existing.log`、`target/s1-fault-isolation-green.log`、`target/s5-data-migration-upgrade-green.log`。数据访客输入固定为摘要账本中的 `capability-example-0.19.0`，不以历史 bare-ID 的 0.17 输入代替当前公开命令。

失败回滚的 RED/GREEN 日志为 `target/s5-data-migration-rollback-red.log` 与 `target/s5-data-migration-rollback-green.log`。冷安装成功与首次 Command Demand 激活失败分别检查；没有为保留旧 eager 断言而改变安装语义。

## 热更新发现的实际恢复缺陷

历史测试迁移完成后，组合更新继续暴露实际失败，不能归类为 eager 前提问题。`target/s5-hot-update-plan-trace.log` 的定向复现运行 164.49s，显示恢复后新 native plan 在发布时已 inactive，而源指纹匹配、此前没有可重用计划。

原因：切换临时移出旧实例时可创建 `owner=None/materialized=false` 的可用性记录；回滚恢复新 Main 后，原实现只把 `owner` 更新为 Some，未采用该实例已激活的状态。下一 Language Demand 再次激活替代 Main，但计划继承恢复 Main 的授权；提交替换恢复 Main 时使计划立即失效。`activation/mod.rs` 现只在发现实际 WASM owner 时将 `materialized` 升为 true，保留声明式无 owner 已完成记录，不改变首次冷安装行为。临时 `DEBUG-s5-plan` 探针已移除。原公开组合更新回归 1 passed，215.69s，日志 `target/s5-hot-update-final.log`。

## 当前 SDK 夹具准备

通过现有宿主公开入口执行：

```powershell
.\target\debug\editor-app.exe --export-plugin-sdk target/s5-runtime-sdk-audit
.\target\debug\editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/language-delegate --output target/s5-fixtures
```

两项成功。导出的 SDK 已核对包含 `Request.context_snapshot` 与 `service_sets`。插件构建使用宿主私有 SDK 缓存，输出为 `target/s5-fixtures/language-delegate-fixture-0.1.0.zip`；原固定 ZIP 未覆盖。`native_modules` 测试允许通过 `NANOBUG_LANGUAGE_DELEGATE_PACKAGE` 显式选择该新构建，仍走公开 `Package` 校验与 Manager/SDK 请求链。其真实生命周期 case 尚待本轮执行，不能计为通过。

新构建 ZIP：374147 字节，SHA256 `33C435A8A9899AD15A0F0C473F726F06A4F383320761726B751843997DD69201`。原固定 ZIP 的 SHA256 为 `2B59D3D625F9D1A389EA028F18C584A99393D282912FC04A1D863A010291DAAD`。两者不能混用为同一轮 SDK 证据。

## 待验证

- 数据迁移：真实子进程中断恢复、休眠工作区迁移；源码已补原始 Command Demand，尚未执行本轮这两个 case。
- 动态依赖计划收据、未使用下载跳过（`managed_dependencies` 中 ignored case）。
- 当前 SDK 委托夹具的模块撤销、进程归零、另一上下文不受影响及借出方重新准备。
- 实际 TS/Vue 的版本组合、离线准备、故障恢复及原生状态交互仍需联合验收。

没有提交、推送、关闭工单或发布版本；未执行本轮整库门禁，不能据此宣布 S5 完成。
