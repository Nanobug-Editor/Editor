# 多文档复用服务后的启动状态回退修复

日期：2026-10-24。状态：原因已复现并修复；26 项针对性回归、规定工作区门禁和修复后的 Windows 原生交互复验通过。未提交、推送或发布。

## 症状与原因

用户的 Rust 文件已经有悬浮信息，底栏仍显示“外部服务启动中 1”。当前运行的是 `target/release/editor-app.exe`，实际安装 Rust 0.6.1；只读检查其公开声明已包含 `experimental.serverStatus` 就绪通知以及 `serverStatusNotification: true` 客户端能力。实际点击服务弹窗确认条目为 `rust/analysis`，不是其他服务。

用独立 profile、公开声明式资源包和真实 stdio 测试服务复现：首个文档的服务完成初始化，loading 消失；再打开第二个同语言文档，loading 持续重新出现。wire 中只有一次 `initialize`、两个 `textDocument/didOpen`，界面已接收到两个文档的诊断，说明原进程仍可正常处理请求。

证实的原因是文档需求与服务握手共用了状态表：`demand_open_language_servers` 在文档集合变化时重新申请文档使用资格；`publish_language_demand_states` 把这次 pending 覆盖成 `Loading`。同一个有效服务被复用，后续分支不会重新启动它，因此不会再次触发 `finish_server_loading` 把状态改回 `Ready`。这属于通用宿主状态回退，Rust 只是实际触发者。

曾按顺序核查：文档需求覆盖就绪状态、过期检查丢弃回调、就绪通知不匹配。受控非 Rust 服务的重复复现、成功请求及单次初始化，将问题收敛到第一项。

## 修复边界

仅修改 `app/language_servers/demands.rs` 的状态投影：在当前计划匹配、信任和有效性检查已经保留原适配器后，保留该适配器的握手状态，不用新文档需求覆盖它。没有有效适配器时，仍显示真正的准备中或准备失败；实例替换、禁用和过期回调保护沿用已有规则。

未改 Rust 包、公开 SDK、协议、服务启动策略或权限。没有通过超时强制隐藏图标，也没有以一次成功业务请求代替就绪通知。

开始时 HEAD 为 `561aa5b2006c38668a083482156646a16cd02b3e`，已有 257 项工作区改动。本轮文件快照、状态和日志位于 `target/service-readiness-2026-10-24/`，已有改动不计为本次实现。`metadata.json` 核对同一 checkout，Cargo 复用其原有独立输出 `target/exclusive-drag-replacement-2026-10-24/cargo/`。

## 真实回归

新测试 `app/plugins/status_tests/readiness.rs` 从 Package / Manager 进入原有宿主发布路径，使用真正的 `lsp_fixture` 子进程和 JSON-RPC，就绪后取得 `fixture-completion` 响应，再发布同一服务并打开第二个文档；最终断言底栏没有重新出现 loading，且原适配器继续复用。

```powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target/exclusive-drag-replacement-2026-10-24/cargo'
cargo build -p plugin-runtime --example lsp_fixture --offline --jobs 2
$env:NANOBUG_LSP_FIXTURE = Join-Path $env:CARGO_TARGET_DIR 'debug/examples/lsp_fixture.exe'
cargo test -p editor-app ready_native_service_stays_ready_after_publication_and_another_document --offline --jobs 2 -- --ignored --exact app::plugins::status_tests::readiness::ready_native_service_stays_ready_after_publication_and_another_document --test-threads=1
```

- RED：`red-native-readiness.log`，0 通过、1 失败，运行 0.56 秒；失败断言为“another document must reuse readiness of the same live service”。
- GREEN：`green-native-readiness.log`，1 通过、0 失败、0 跳过，运行 0.50 秒。
- 普通非 ignored 的 `status_popover_preserves_language_loading_and_keyboard_dismissal` 同步增加第二个文档的可见底栏断言，使常规检查也覆盖该状态路径。真实进程回归是显式运行的 opt-in 测试，未把跳过当作通过。
- `cargo test -p editor-app app::plugins:: --offline --jobs 2 -- --include-ignored --test-threads=1`：23 通过、0 失败、0 跳过，包含上述真实服务回归。
- `cargo test -p editor-app extensions::worker::worker_tests::language_failures --offline --jobs 2 -- --include-ignored --test-threads=1`：3 通过、0 失败、0 跳过，覆盖准入拒绝、准备取消及失败发布撤销，日志见 `extensions-worker-worker_tests-language_failures.log`。

规定门禁实际结果：`cargo fmt --check`、`cargo check --workspace --offline --jobs 2`、`cargo build -p editor-app --offline --jobs 2` 和 `git diff --check` 均退出 0；`cargo test --workspace --exclude editor-app --offline --jobs 2` 为 555 通过、0 失败、403 跳过。403 项跳过未计为通过。对应日志及退出状态见 `checks.json`、`workspace-tests.log`、`workspace-check.log`、`host-build.log`、`fmt-check.log` 和 `diff-check.log`；既有 unused 和链接器警告保留，未扩展范围清理。

## Windows 复现与交付验证

通过 computer-use 在独立开发实例操作两个测试文件。修复前的启动列表与底栏失败证据为 `native/second-file-stuck.png`、`second-file-stuck.a11y.txt`；用户实际 Rust 提示另存 `user-stuck-service.png`。夹具服务通过公开 `process.service.analysis` 权限启动，未替换用户已安装插件或个人 profile。

修复前使用上一轮实际宿主，开发控制器 `stop` 后 `native/report-before.json` 为 `status=passed, cleanup=passed, termination=normal`；它只表示生命周期正常退出，不代表错误的底栏行为通过。

修复后用重建的同一路径 Debug 宿主和相同包摘要创建全新隔离 profile：第一个文档正常完成启动；双击第二个文档，收到第二份真实诊断后底栏仍没有 loading；关闭第二个文档，诊断数量回到 1，焦点回到编辑区，loading 仍未出现。修复后 wire 单独增量为一次 `initialize`、两次 `didOpen`、一次 `didClose`，证明持续复用了同一个服务。证据为 `native/second-file-ready.png`、`second-file-ready.a11y.txt`、`closed-second-ready.a11y.txt` 和 `wire.jsonl`；`wire-before.jsonl` 固定了修复前日志边界。

实际验收宿主 SHA-256 为 `0434000681ec5dc949c371e9c6a819900f411d6fc269b0d641b69356e1a16250`，构建时间 UTC `2026-10-24T13:11:11.4078033Z`，见 `host-identity.json`。退出后的 `native/report-after.json` 为 `status=passed, cleanup=passed, termination=normal`，报告中的宿主摘要一致，且与修复前使用相同包摘要；只读进程检查未发现本任务夹具残留。上述实际 UI 观察独立于生命周期报告的业务覆盖状态。

Windows 复验使用浅色中文与受控 stdio 服务；自动 UI 回归也覆盖深色中文。用户当前 Release 进程不会热替换宿主 Rust 代码，源代码修复需要重新构建运行后生效；本轮未中断用户工作窗口，也未将新 Debug 验收外推为重新运行了用户的 Rust 分析进程。

返回[插件文档目录](../README.md)。
