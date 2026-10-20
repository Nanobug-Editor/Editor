# 委托准备取消后的独立重新请求

日期：2026-10-20。对应新 S2 #122 / S5 #125 的取消与过期隔离；不更新旧工单，不代表场景整体验收完成。

## 输入与公开接缝

- 宿主工作区为 `C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor`，HEAD `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 加当前未提交实现。构建前 `cargo metadata --no-deps --format-version 1` 已核对 `workspace_root` 为本工作区，`target_directory` 为其 `target`。
- 独立当前 SDK 夹具：`target/s5-current-sdk-fixtures/language-delegate-fixture-0.1.0.zip`，SHA-256 `A784DAE13E887FA18F225504A5AF842728309D751DA3B5AE30CC6B2C34D5F3D5`。沿用已构建输入，没有修改访客、SDK 缓存或归档。
- 测试：`native_modules::delegation_retirement::renewed_source_does_not_join_an_abandoned_deferred_control`（测试二进制内过滤名没有前缀 `native_modules::`）。通过普通 `Package` 验证并派生未知语言 source / engine 包，实际 SDK WASM 经公开服务调用发出原生 `LanguageRequest`。
- 原生分派器持有第一条真实请求，取消原文档需求后立即以同一 `DocumentContext` 重新需求，再向旧请求返回 `StaleRevision`。新需求必须发出第二条控制请求，不能继承旧错误。成功回复使用公开 Manager 准备的真实 `LanguageService`、模块租约和一次性 binding。

## RED → 修复 → 完整 GREEN

| 阶段 | 实际结果 | 日志 |
| --- | --- | --- |
| 原实现 | FAIL，66.17s。新需求直接完成为旧控制请求的 `StaleRevision: Original source changed during the abandoned control`，没有发出第二条控制请求 | `target/s2-deferred-native-isolation-red.log` |
| 最小修复后首轮 | FAIL，66.98s。第二请求、绑定、同 Arc 与取消断言均已通过；最终 `shutdown_checked` 因固定 guest 未实现 `Input::Snapshot` 而失败。此轮不计 GREEN | `target/s2-deferred-native-isolation-green.log`（保留当时文件名及失败终态） |
| 修正测试清理后的完整运行 | PASS 1/1，68.78s。新调用独立完成；原请求仍为 Cancelled；借用相同活跃 Arc；公开停用两包后计划 inactive、进程数 0、原 owner 消失，最后 checked shutdown 成功 | `target/s2-deferred-native-isolation-final.log` |
| 同二进制普通准入回归 | PASS 2/2，4 ignored。两项闭包、摘要与权限准入通过；四个 ignored 不计本次普通回归通过 | `target/s2-deferred-native-isolation-admission.log` |

生产修复仅位于 `manager/language/delegation.rs::join_language_delegation`：旧等待者列表非空且全部终态时，撤销旧调用，再以新调用准备；刚创建且尚未加入首个等待者的空列表继续允许加入。仍有有效等待者时保持共享原调用和业务错误传播。后一分支经过代码核对，不将其称为新增独立实测用例。

最终测试编译为 PASS 4.44s，日志 `target/s2-deferred-native-isolation-final-no-run.log`；二进制 `target/debug/deps/native_modules-9cb1557223d3c540.exe` 的 SHA-256 为 `A8F5A3BFB77B8D92C908F47F139FDE6B5E722F6E33C1E6333CA9ADC90F454659`。

复放命令（PowerShell，工作目录为上述宿主工作区）：

```powershell
cargo test -p plugin-runtime --test native_modules renewed_source_does_not_join_an_abandoned_deferred_control --no-run -j1
$env:NANOBUG_LANGUAGE_DELEGATE_PACKAGE = (Resolve-Path target/s5-current-sdk-fixtures/language-delegate-fixture-0.1.0.zip).Path
& .\target\debug\deps\native_modules-9cb1557223d3c540.exe --exact delegation_retirement::renewed_source_does_not_join_an_abandoned_deferred_control --ignored --nocapture --test-threads=1
```

## 证明边界

本用例不启动原生进程，只检验公开委托调用、取消、绑定与撤销。进程数 0 不代表本例验证了物理 EOF、后端重启或实际 Vue 语义恢复；Manager 也不持有编辑器文本，未保存快照由其他原生 UI 用例验证。

固定 guest 不实现状态快照，没有不存在的清单字段可以关闭此契约。因此本用例显式停用临时包后执行 checked shutdown，仅证明取消路由和资源撤销，不证明其状态持久化。原生 .12/.13 初始准备失败并未由此回归建立因果关系；实际 .14 内层后端恢复已在独立入口通过，保留各自证据。
