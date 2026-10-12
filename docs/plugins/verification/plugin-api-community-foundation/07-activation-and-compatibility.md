# 07：按需激活、配置热应用与兼容升级验收

状态：已验收交付。普通源码提交 `8ee28952014a6533253fc83e17c3410d88a76e48` 已推送 `origin/codex/plugin-api-community` 并核对完整SHA；2026-10-11T22:20:41Z [#99](https://github.com/T-miracle/Nanobug/issues/99)已关闭，随后独立读回为 `closed / completed`。七张实施工单均已关闭，父设计议题 #92 保持开放且正文未修改。阶段快照及失败保留其当时真实状态。

依据：[工单](../../tickets/plugin-api-community-foundation/07-activation-and-compatibility.md)、[共同完成条件](../../tickets/plugin-api-community-foundation/README.md)、[社区生态方案](../../specs/plugin-api-community-foundation.md)。接口用法维护在公开协议源码注释，独立 SDK 只导出 Rust、Cargo、WIT 与 JSON。 [独立双轴记录](07-activation-and-compatibility-review.md)分别保存 Standards 与 Spec 结论。

## 当前验收与交付状态

- 最终规定三门禁已实际通过：格式 exit 0，非 UI `260/0/246 ignored`，Workspace check exit 0。246 项 ignored 未在默认门禁中执行，不计作通过覆盖；新增 Query 窄 probe 已另行实际通过。
- [十九项 Runtime 矩阵](#门禁后的-runtime-指纹与十九项计划)均已显式执行，实际 `19/0/0`；T32 三次及 SDK5 一次历史 Red 保留，额外窄 Query 用例的 `1/0/0` 独立计数。
- [应用结果](#新宿主构建sdk-字节核对与当前应用结果)：原十二项实际 `12/0/0`，最终强观察、冷 restart 及 FileView 的完整 Green 均已记录；四项原失败和 FileView 追加诊断 Red 保留。
- 最终生产 Host `E81B1F97...DBC0` 导出 50 个 SDK 文件、0 Markdown，与旧 1D29 导出的文件字节全部相同。T35 实际包 GPUI 短组合和 C03 Windows 新增交互均已验收；[Windows 物理记录](native-07.md)明确具体场景与环境限制，不声称八包全量物理组合。最终独立双轴均零问题，普通源码交付及议题关闭已实际读回。

## 固定基线与输入

- 共同工作树 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/Editor`，分支 `codex/plugin-api-community`。固定审查起点 `90ceab915d63c2e6b8cb260c6f28acfc45adc7fc`，相对已验证的 04 交付点 `2d140d1` 仅增加真实交付状态文档。
- 2026-10-11T15:17:51Z 读回 #96 为 `closed / completed`；#97、#98 随后也重新读回同状态。依赖满足后开始本单，未修改父设计议题。
- 现有 Rust/Cargo 1.98.1、VS 18 Community x64、`wasm32-wasip2`；直接导入已有开发环境，`CARGO_TARGET_DIR=../target01`、jobs=2，不安装新工具、不调用旧脚本。
- 应用与运行时冷启动回归使用既有 capability-example 0.19.0 正式 ZIP：SHA256 `BDFC5D3327B5B7A4F3D25BA1AE95E2A6A47B62634FB040E17FB4430C9AA46773`。这是当时源码 SDK 38 文件的实际 WASM，不把本单新 SDK 的编译替代旧 SDK 兼容证明。
- 保留旧云插件 0.1.1 两个包：cloud-translation ZIP `C1D6D01DD5D725F1686E71BEBFFCE8F1CDBD8CFFB8CAED64665314D096A0C67C`；code-host-query ZIP `8E245135A2EC3745B8D5195E3C4F48378DC0746B3156D91B6FEB7E438776A6C4`。新配置消费者已通过本单内嵌 SDK 50 独立构建并分别记录产物；下文新宿主再次确认 SDK 五十文件字节完全一致，不重复构建未变更 WASM，完整行为仍另行验证。

## 兼容准入的首个纵切

`cargo test -p plugin-runtime --test compatibility local_package_rejects_an_unsupported_contribution_host_range -- --exact --nocapture` 经公开 `Package::from_files` 提交一个真实声明式资源包，`plugin.toml` 要求 `host_version=">=2.0.0"`。

Red：实际包被本机 0.1.0 宿主错误准入，测试 `0 passed / 1 failed / 0 ignored`，exit 101，总时长 37.01 秒；`../target01/07-host-range-red.log` 与对应 `07-host-range-red-result.json`。它是行为失败，不是编译错误。

修复使 ZIP 与隔离开发的统一准入入口在任何安装变更前检查作者声明的 SemVer range。Green：相同过滤词加 `--offline`，`1/0/0`、exit 0，编译 5.73 秒；`../target01/07-host-range-green.log`。最低宿主 `minimum_host` 使用完整 SemVer 下限，与 TOML 的兼容范围及能力范围独立。

## 默认冷启动纵切

公开协议与准入/恢复契约的阶段针对性检查已经通过：`cargo test --offline -p plugin-protocol` 共 56 passed、0 failed、0 ignored，14.13 秒；`cargo test --offline -p plugin-runtime --test compatibility -- --nocapture --test-threads=1` 实际 8 passed、0 failed、0 ignored，行为 0.52 秒、总 28.75 秒。后者覆盖未来 host range、有效/无 minimum floor、忽略 SemVer build metadata 的下限、unsupported/malformed floor 拒绝、旧协议与有限别名迁移保留、热值的最低配置能力/钩子/授权和后台用途声明。声明准入使用最小组件头时只证明声明检查；完整新旧 SDK WASM 行为仍另外验证。日志 `07-protocol-targeted.log`、`07-compatibility-targeted.log` 和 `07-contract-targeted-results.json`，均在 `../target01/`。

运行时实际旧 SDK 包测试 `installation_enable_and_reopen_keep_old_sdk_business_dormant` 首轮 `0/1/0`、exit 101，行为 27.54 秒、编译 35.01 秒。公开安装后业务实例已运行，违反“静态贡献可发现、业务保持冷”的新规格；日志 `../target01/activation-cold-old-sdk-red.log`。

保持真实旧包且不添加伪后台声明的最小 Green 为 `1/0/0`、exit 0，行为 30.63 秒、编译 15.56 秒；日志 `activation-cold-old-sdk-green.log`。安装、disable→enable 与 Manager reopen 均保持静态贡献可见、业务 instance_id 无值、资源数零；运行中的替换仍保留最终 Snapshot、停止与 fresh owner 恢复，冷准备没有执行 business Activate。各类需求、并发/取消及配置变更的后续阶段证据分别记录于下文，不以此单条 Green 宣称生命周期完成。

应用首轮取得 `metadata discovery cannot run Main` 的实际失败，但测试在持锁时断言，污染 publication mutex 后析构再次 panic，进程 exit `0xc0000409`；日志 `07-cold-command-actor-red.log`，不将其记作常规 `0/1` 结果。仅修正测试，使它读取状态后释放锁、正常关闭 Worker 再断言，未修改产品行为来掩盖失败。

随后一次 `cargo test --offline -p editor-app --no-run` 编译 exit 0、约 72 秒，从同一个实际测试 EXE 执行两项行为 Red。EXE SHA256 `B6CE990CCC16D94AF642B27247E4764E3C1BC5F369D981D4F8D885648B5DB063`，使用 `--ignored --nocapture --test-threads=1`：

| 过滤词 | 实际结果 | 独立可观察失败 |
| --- | --- | --- |
| `cold_commands_are_discoverable_without_starting_the_guest` | `0/1/0`，exit 101，行为 59.52 秒 | 静态命令元数据发布时已经执行业务 Main |
| `native_plugin_menu_rejects_an_edit_while_its_original_command_waits` | `0/1/0`，exit 101，行为 27.36 秒 | 菜单接受之后、分派之前原文档已编辑，旧命令仍到达真实 WASM |

日志为 `../target01/07-red-<过滤词>.log`，构建及两命令摘要分别为 `07-actor-intent-red-build-result.json`、`07-actor-intent-red-results.json`。失败之后才连接默认冷启动及原目标意图的修复；第二项的后续 Green 还必须通过新的有效菜单请求正控。

## 共享首次激活与服务/语言边界

运行时第二个纵切 `first_command_demand_coalesces_without_changing_static_identity` 使用同一真实旧 SDK ZIP。初轮仅因测试对无 `PartialEq` 的 `CancellationEffect` 使用 `assert_eq!` 编译失败，日志 `activation-demand-old-sdk-red.log`，不计作行为 Red。修正为 `matches!` 后，真实 Red 为 `0/1/0`、exit 101，行为 30.64 秒、编译 5.23 秒；`activation-demand-old-sdk-behavior-red.log` 明确返回 `CapabilityUnavailable: Demand target has not been materialized`。

接入共享后台准备后，相同用例 Green 为 `1/0/0`、exit 0，行为 53.92 秒、编译 18.29 秒；`activation-demand-old-sdk-green.log`。两次原命令需求共用一次 first birth；取消一方得到 `NotExecuted`，另一方取得确切 owner，静态 availability 不因出生变化；disable→enable 后旧 receipt 不再准入。

服务纵切 `cold_service_metadata_and_first_call_preserve_reference_and_original_deadline` 由两个真实 SDK 包进入公开 Manager 安装与服务选择。首轮合法安装后没有任何 `example.echo` 选择项，Red `0/1/0`、exit 101，行为 54.03 秒、编译 3.44 秒、总 57.72 秒；`07-service-cold-red.log` 与 `07-service-cold-red-result.json`。随后才接静态登记、共享一次 owner binding 和保留原绝对 deadline 的等待队列。冻结的实际 EXE Green `1/0/0`、exit 0、108.26 秒；同时验证 metadata/open 仍冷，已过期或已取消的调用未执行业务，未取消的调用绑定一次 owner，同一个 open reference 可继续调用同一 owner，disable→enable 后旧 reference 不复活。

`cold_native_execution_binds_its_pending_session_once` 使用同一冻结服务测试 EXE，通过现有公开执行会话请求冷提供者，实际受控 Windows 子进程退出。Green `1/0/0`、exit 0、54.28 秒；原会话从 Starting 等待首次出生、绑定确切 instance、报告 Exited，禁用后资源归零。两项 EXE SHA256 `9C1727D11A7BB5127AE06461A541192D185DB48823327D928DAB83AA11A8A17B`；直接执行日志 `07-green-<过滤词>.log`，摘要 `07-demand-stage-green-results.json`。

语言目录首轮错误使用未知声明字段 `lsp`，被 Package 正确拒绝，日志 `activation-language-catalog-red.log`，不计作行为 Red。改为现行 `primary` 后，实际 Red `0/1/0`、exit 101，行为 0.05 秒、编译 26.00 秒；`activation-language-catalog-behavior-red.log`。两个合法声明式提供者仅因发现目录即已准备，违反默认冷启动。修复后从冻结 EXE 执行 Green `1/0/0`、exit 0、0.04 秒：静态发现无需空 WASM，只准备实际选中语言角色。EXE SHA256 `A4A891A25A7E47FDC605BFEEC1E40B354FA02ABE2E41E32A5BF6008D804B031B`，日志 `07-language-selected-green.log`。

现有调试服务消费者也需保留冷启动等待。首次夹具漏声明合约方法权限，被 Package 正确拒绝，`07-cold-debug-red.log` 不计作行为 Red；补上真实权限后，`cold_debug_start_waits_for_its_original_provider_birth` 实际 Red `0/1/0`、exit 101、行为 27.57 秒，原 start 被错误退役为 `Cancelled / InvalidHandle / NotExecuted`。测试不伪造调试成功：同一真实 SDK 组件明确不实现 debug 方法，修复后的 Green 必须在实际出生后收到其 `UnsupportedOperation`；其实际结果见下一段。

阶段集成构建 `cargo test --offline -p plugin-runtime --test service_activation --test sdk_lifecycle --test language_declarative --no-run` 的 Runtime 生产代码及服务/语言测试编译成功，但新 SDK 夹具 11 处既有 API 接线错误令命令 exit 101，未将其计作 Red。修正为现有 `Manager::update_setting` 与 `manifest.api.as_ref()`，未增加测试专用宿主 API。应用首次 `cargo check --offline -p editor-app` 因 6 处 import/可变借用接线错误 exit 101、14.92 秒；该次失败没有继续 Host build。随后已修正并实际完成阶段检查与宿主构建，结果见“50 文件 SDK 与热配置纵切”。

调试同一实际旧 SDK 用例 Green `1/0/0`、exit 0、53.79 秒，保留原实例意图并实际得到 `UnsupportedOperation`；EXE SHA256 `5C78261B7D2C0A9873DF7FAEBF0300E8897DF1B47FE3D82CF0E1E775132D2478`，日志 `07-cold-debug-green.log`。后台用途实际 Red `0/1/0`、147.86 秒，合法显式用途在 120 秒实际 poll 后仍没有 owner，`activation-background-behavior-red.log`；接入同一个 64 waits / 4 preparations 池后，`explicit_background_purpose_activates_only_after_trusted_polling` Green `1/0/0`、52.75 秒，EXE SHA256 `3E120E2B9808C4B6721709C211D6AA8D1F5AE680F75DBAEE145C815BB2FFE6E4`。受限状态不能执行，信任后的一次尝试可以准备，失败终态不进入无限自动重试。预算、deadline 与 scope ABA 用例阶段 Green `1/0/0`、28.09 秒，日志 `activation-budget-cancel-scope.log`。

## 50 文件 SDK 与热配置纵切

应用接线修正后的 `cargo check --offline -p editor-app` exit 0、7.94 秒；实际宿主 `cargo build --offline -p editor-app` exit 0、77.06 秒。宿主 SHA256 `D5157F2298463EA15F9AD34DBD4DBB371B04DE058C1392F7CA4A323E4676A237` 通过 `--export-plugin-sdk ../target01/sdk07` 导出 50 项、0 Markdown；内容摘要 `488a6baccbb61c29dc6f5020f22bac1b813a0529dbf3a7768c9f760a9245f5fd`，日志 `07-sdk-export.log`、`07-sdk-export-result.json`。该 SDK 源码随后冻结，不把这个阶段 Host 误记为最终原生候选。

同一宿主直接 `--plugin-package plugins/cloud-translation plugins/code-host-query --output target/cloud-api-test` 成功、81.79 秒；两个新消费者版本均为 0.1.2，实际从内嵌 SDK 缓存构建。随后 `--plugin-package plugins/capability-example --output target/plugin-api-test` 成功、29.60 秒，0.19.1 ZIP SHA256 `E9313ADFC6041B0C6151E2944FFC4FB1E625CF6326BBB903B9275D58A650B46B`。三个既有真实旧 ZIP 的 SHA256 仍与固定输入完全相同，未覆盖旧 SDK 兼容夹具。

新 SDK 夹具补完 `VersionReq` 类型比较后，三测试 `--no-run` exit 0、5.46 秒。首次 exact 过滤词漏掉 `scenarios::`，实际执行 0 tests，明确不计作 Red；使用 `--list` 确认后，正确 `scenarios::t32_sdk_cloud_hot_configuration_changes_live_io_and_preserves_scope_and_validation --exact --ignored --nocapture --test-threads=1` 从同一实际 EXE 得到 Red `0/1/0`、exit 101、77.41 秒：更新 timeout 的 HotApply 声明值替换了真实 owner。日志 `07-sdk-hot-actual-behavior-red.log`，EXE SHA256 `22128E5A55A0A2C4D251341278234C88061BCFEE820C90DD51640A7B163313D2`。

红阶段 cloud ZIP 指纹为 translation `976C2F4C057E9B78949469AD52001572A4A78F9B894E32E412C3A6B12BF0BA22`、query `4F0A3B648988E09BDCB67ADB90AA5B93D93362F4471A43FCDBDAFDC5AA933942`，原字节另存 `../target01/07-red-inputs/`。在实际 Red 之后才接 Runtime 同 owner Changed 和两个 Guest 的策略处理。仅重建改变的两个消费者，exit 0、26.79 秒：新 translation ZIP `61273EF15C3750B933AD706FC92A5921B5173A98FE60A8D017E9B60CAA06B74B`、query ZIP `1049771D8AFA45E603901CA7F23423CA37B601BB4FEB58112E8C1AD5FB9349B7`。这次重建只证明独立构建成功；之后 T32 的三次失败、完整 Green、T34 旧能力兼容 Green、迁移 Red→Green 分别见十九项矩阵节，不借下文原生方案的 Green 代替。

## 选中语言失败、准备任务退役与热配置原生方案

本轮结果取自 `../target01/07-runtime-owned-targeted-results.json` 及其中的真实日志，计数均按 `passed / failed / ignored`。`green2` 是第二次修复后的运行名称，实际只执行一项测试。

选中语言计划失败使用旧 SDK 实包，经公开 Package 修改为获准但不存在的服务路径，并让原访客在错误到达业务 Activate 时报告故障。首次 `activation-selected-language-failure-red.log` 是不存在字段的编译错误，随后 `activation-selected-language-failure-behavior-red.log` 被现行 Package 拒绝为无效 LSP 声明；两者都不计作产品行为 Red。合法声明后的实际命令为：

```powershell
cargo test --offline -p plugin-runtime --test activation failed_selected_language_plan_does_not_activate_business_or_hide_its_error -- --ignored --nocapture --test-threads=1
```

| 阶段 | 实际结果 | 失败或验证边界 | `../target01/` 日志 |
| --- | --- | --- | --- |
| valid Red | `0/1/0`，编译 3.82 秒、行为 53.38 秒 | 原选中计划的错误未留在可观察目录 | `activation-selected-language-failure-valid-red.log` |
| 第一次修复后的失败 | `0/1/0`，编译 17.97 秒、行为 146.73 秒 | 尚未发布的发现配置与原冷配置指纹不符，等待未完成为原计划错误 | `activation-selected-language-failure-green.log` |
| Green2 | `1/0/0`，编译 9.12 秒、行为 52.68 秒 | 原需求完成为错误，目录保留对应 Err，没有业务 instance、没有 `wasm/activate` 记录、资源零 | `activation-selected-language-failure-green2.log` |

丢弃准备任务与立即 shutdown 从已构建 EXE 直接执行，不经过 Cargo 再编译：

```powershell
& ../target01/debug/deps/language_declarative-95d3220f5980e3b8.exe dropping_taken_or_finished_preparation_releases_original_wait_and_capacity --exact --nocapture --test-threads=1
& ../target01/debug/deps/language_declarative-95d3220f5980e3b8.exe --nocapture --test-threads=1
```

第一条 Red `0/1/0`、0.08 秒，`activation-preparation-drop-shutdown-red.log`：公开 shutdown 后原等待仍未立即变为 `Cancelled / InvalidHandle`。修复后第二条运行声明式文件全部三项，Green `3/0/0`、0.26 秒，`07-language-declarative-current-green.log`。丢弃尚未运行的 preparation、丢弃已运行但未提交的结果都令原请求终止并释放槽位；后续显式需求仍可成功；shutdown 无须另一次 actor tick 即撤销等待。另两项同时验证仅选中角色准备、无需空 WASM，以及 disable、enable、uninstall、shutdown 后原请求、Batch 与原 native authority 不复活。这里只验证准入、资源和生命周期，不将没有启动真实进程的声明式方案测试记作 stdio 或编辑 UI 通过。

热配置原生方案测试也直接使用真实新 SDK capability-example 0.19.1 构建的 EXE：

```powershell
& ../target01/debug/deps/activation_native_hot-67fb4cbe5aef6e64.exe hot_configuration_reuses_only_an_unchanged_selected_native_plan --exact --ignored --nocapture --test-threads=1
```

Red `0/1/0`、105.14 秒，`activation-native-hot-plan-red.log`：同 owner 热配置尚在检查选中方案时就退役了原传输。Green `1/0/0`、210.59 秒，`activation-native-hot-plan-green.log`：配置变化撤销原 availability 并在重新检查前隐藏目录，业务 owner 保持；选中方案实际不变时保留确切同一个 Arc、传输和 authority，实际可执行路径变化时才退役旧传输并建立新 seal；disable 后资源零。这项没有启动服务进程，不代替真实 LSP/原生交互验收，也不代替 T32 云配置的完整行为测试。

服务外层取消新增了另一条真实 Red，不因较早服务纵切 Green 而隐藏。`cargo test --offline -p plugin-runtime --test service_activation cold_service_metadata_and_first_call_preserve_reference_and_original_deadline -- --ignored --nocapture --test-threads=1` 得到 `0/1/0`，编译 8.66 秒、行为 107.46 秒；`07-service-ready-cancel-red.log`。原调用者已经取消时，`commit_demand` 仍接受已准备好的 candidate 并发布 owner。该 Red EXE SHA256 `66352F1AD20EFA2AD05407A0AED3BA0F8AB04BEF19FAA769A49778D9BA92C3A1`；原 caller/source/绝对 deadline 的接线修复后，当前矩阵 case8 已在新的 `70E08871...4707` EXE 实际取得 `1/0/0`，行为 131.68 秒、总 131.7130404 秒；日志 `07-runtime-matrix-case8-service_activation-current.log` 与同名 `-result.json`。这个新增边界绑定此次实际结果，不沿用较早版本的 108.26 秒 Green。

## 嵌套服务祖先取消与自然 deadline

独立 `demand-relay` 0.1.0 Guest 通过正式服务能力产生真实嵌套需求，没有新增宿主测试专用 API。首次行为回归使用红阶段 ZIP `51FF4FC216F0B811A69E9DD9DD44A74466AA6D8729A537FD2A0241659107AA3B`，原字节封存 `../target01/07-red-inputs/demand-relay-0.1.0-ancestor-red.zip`。实际命令含完整模块过滤词：

```powershell
cargo test --offline -p plugin-runtime --test service_activation nested::nested_service_ancestors_cancel_before_birth_without_expiring_completed_sources -- --exact --ignored --nocapture --test-threads=1
```

| 阶段 | 实际结果 | 测试 EXE SHA256 | `../target01/` 证据 |
| --- | --- | --- | --- |
| Red | `0/1/0`、exit 101；行为 112.16 秒、总 125.51 秒 | `BAB466D27F97022A0390ED05425B70F4A43FE20CBBE3CC148362DA10DC35E3D5` | `07-service-nested-ancestor-red.log`、`07-service-nested-ancestor-red-result.json` |
| Green | `1/0/0`、exit 0；行为 141.71 秒、总 164.05 秒 | `66E80E1204149E84509BD64A9C564297AF9DF8EB6D4CF1ED20A01B5AB26A000D` | `07-service-nested-ancestor-green.log`、`07-service-nested-ancestor-green-result.json` |

Red 明确在“已取消的祖先不能发布已准备好的子 Main”断言失败。修复使请求状态、激活提交和服务等待三处门禁在克隆共享等待前绑定祖先的真实状态；未观察 `status`、未再次 poll 的自然 deadline 也会撤销后代，已经 Completed 的祖先则不继承旧 deadline。相同用例的 Green 实际覆盖祖先取消、无 poll 自然超时、已完成祖先超过旧 deadline 后仍合法续调三个边界。只此嵌套用例得到当前 Green，不将它代替上一节 waiting-ready 原调用者取消的独立边界；该边界的后续 Green 由十九项 case8 单独给出。

现行格式 fixture 经同一公开宿主独立构建：`../target01/debug/editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/demand-relay --output target/plugin-api-test`，exit 0、总 5.71 秒，`07-demand-relay-final-source-build.log` 与 `07-demand-relay-final-source-build-result.json`。使用同一个 SDK 50 缓存，公开接口源码未变；Green 包 `target/plugin-api-test/demand-relay-0.1.0.zip` SHA256 `F558C7164E7BBB90DA214B0D4CC453F91772767489E84058BFEA1CB8EC3C44C3`。前期包准备错误不计作行为 Red，独立构建成功也未代替上表真实测试。三处门禁与当前 fixture/test 源指纹分别保存在 `../target01/07-final-nested-cancel-source-fingerprints.json`。

## 当前编译固定输入与阶段门禁

此前七目标 Runtime 阶段命令实际 exit 0、40.38 秒，日志 `../target01/07-current-runtime-no-run.log`：

```powershell
cargo test --offline -p plugin-runtime --test activation --test activation_native_hot --test sdk_lifecycle --test language_declarative --test cloud --test service_activation --test plugin_services --no-run
```

七目标均只是编译成功，没有行为测试计数。`../target01/07-current-runtime-exe-fingerprints.json` 保存 2026-10-11T17:12:33Z 至 17:12:49Z 的实际 EXE 输入：

| 目标 / `../target01/debug/deps/` 文件 | SHA256 |
| --- | --- |
| `activation-102683a7d1d79a8c.exe` | `EA5E94A4B93987C91C973A6942987084F34EDF7DBD3BFD26EB7BA700FD5811E3` |
| `activation_native_hot-67fb4cbe5aef6e64.exe` | `8A34EAC0DEF57CB4F42023032D4690E03618A0395C5950B6C52D3A0093C47B6F` |
| `cloud-8b2301a2340f94b6.exe` | `B843715624558D92118BB8CF3C92DD77AE91F74392D3BC2F9380366B187C69DB` |
| `language_declarative-95d3220f5980e3b8.exe` | `64573BD73BCF1E4402BE274739E055324074FE90E37C100128A46A383653CB7C` |
| `plugin_services-fd7740afef716deb.exe` | `6D3F5FA9622169A24CE82C0C3045C9FC84626556A56AD3F6CBCDF1ECB5E26611` |
| `sdk_lifecycle-869c97e3114cd1d1.exe` | `319558C03B3C0A42241936390B1C3C1E534F61C1F3E4B1FAD92F13608BF4C56B` |
| `service_activation-17275e4a35729684.exe` | `5EBD2144EF59414A0844CA95471B50E2C0C434ACA39BF47F31268C7EEA2CF481` |

以上指纹是该次编译的记录，不把同路径后续重编译的字节当作同一冻结输入。`development_projects` 未包含在这七目标中，其两条实际隔离开发目录测试已经改为首次 install/reopen 业务事件前通过公开 Availability → 原 panel 的 View demand → poll → admit；原 reload 失败、私有数据、权限及正常安装隔离断言保留。下文已记录公开 Host 的开发候选构建成功，该测试目标随后纳入八目标编译；随后两条均已显式执行，实际结果见十九项 case17/18；这里的 no-run 仍只计编译。

嵌套守卫修复后的八目标命令实际 exit 0、总 20.19 秒，`../target01/07-final-runtime-eight-no-run.log` 与 `07-final-runtime-eight-no-run-result.json`：

```powershell
cargo test --offline -p plugin-runtime --test activation --test activation_native_hot --test sdk_lifecycle --test language_declarative --test cloud --test service_activation --test plugin_services --test development_projects --no-run
```

八目标也仅证明编译，不计 ignored 矩阵通过。`07-final-runtime-eight-fingerprints.json` 保存该次实际输入：

| `../target01/debug/deps/` 文件 | 该次编译的 SHA256 |
| --- | --- |
| `activation-102683a7d1d79a8c.exe` | `368F68BF99BBE57793CD8586FA3445EFE5A3507605BCF0AAE83C586F4C916E4E` |
| `activation_native_hot-67fb4cbe5aef6e64.exe` | `7915F8EFA3BDD542CADABCDA2F7150EF21B743BECE62C00E3F288E828DD18869` |
| `cloud-8b2301a2340f94b6.exe` | `BE7CDCBF927DD5884ED8BFFE6300A8DAD9370914A9CCD29FE1747BA9FAF39DA2` |
| `development_projects-7b8856594159362d.exe` | `3A8834486C0244BEC43A3C248C6E8EA9B1367B70F26D89B0F62D9F91839AE3F5` |
| `language_declarative-95d3220f5980e3b8.exe` | `20DF64D4731A4BDC490FB1181013B6D8782277BBD9FE91F0BE77DDF6A349A5F0` |
| `plugin_services-fd7740afef716deb.exe` | `D1B4F4A2A594BC43B5C00AF364F778B08EC2BE6960C7DF0AD8F1366A54528ECF`（后续测试源码有变，已由下文门禁后指纹替代） |
| `sdk_lifecycle-869c97e3114cd1d1.exe` | `0001410290D1560CBB651FB55ECBEDD7DFC7EDD6C309115A318ABF53D7755E2E` |
| `service_activation-17275e4a35729684.exe` | `66E80E1204149E84509BD64A9C564297AF9DF8EB6D4CF1ED20A01B5AB26A000D` |

八目标编译后，`plugin_services.rs` 的两个恢复 enable 场景及 legacy Restart 后新增三次显式 View demand；上表该目标的旧 hash 不得作为最终测试输入。随后已完成实际新编译并保存下文门禁后的八目标指纹；旧指纹仅保留阶段历史。随后十九项 ignored 矩阵的实际终态另记于下文，不以这次 no-run 代替。

## 官方非 UI 门禁与旧夹具适配

四次均直接执行规定命令 `cargo test --workspace --exclude editor-app`，保持默认测试并发；`CARGO_BUILD_JOBS=2` 只限制编译，没有把门禁改为缩减测试、单线程替代或跳过失败。计数来自相应 `../target01/07-final-workspace-non-ui-default[-retry,-retry2,-retry3]-result.json`，前三次失败日志原样保留：

| 运行 | `passed / failed / ignored` | 已运行 binaries | exit / 总时长 | 日志 |
| --- | --- | --- | --- | --- |
| 首轮 | `198/1/2` | 8 | 101 / 206.36 秒 | `07-final-workspace-non-ui-default.log` |
| retry | `218/2/144` | 38 | 101 / 160.87 秒 | `07-final-workspace-non-ui-default-retry.log` |
| retry2 | `227/6/145` | 42 | 101 / 10.18 秒 | `07-final-workspace-non-ui-default-retry2.log` |
| retry3 / 最终完整 Green | `260/0/245` | 75 | 0 / 27.42 秒 | `07-final-workspace-non-ui-default-retry3.log` |

首轮是旧 scope 测试写入空 `plugin.toml`，被此前统一静态准入正确拒绝；补合法 id/version/host range 声明与冷启动注释，原项目 override/reopen 断言未削弱。随后两次是旧语言工具/依赖测试仍假设 install 即初始化 LSP，读取未准备目录得到 `no entry`；依次补回真实选中 LSP role 的需求前提。仅改旧测试与测试 support；最后嵌套守卫的三个生产文件没有因这些门禁失败新增行为改动。

窄验证实际命令、计数及精确指纹如下，日志与 JSON 同名并位于 `../target01/`：

| 用例 / 实际命令 | 实际结果 | EXE SHA256 / 证据 |
| --- | --- | --- |
| `cargo test --offline -p plugin-runtime --lib manager::scope_tests::project_override_survives_reopen -- --exact --nocapture` | `1/0/0`，行为 0.04 秒、总 6.29 秒 | `7FA9BA458A532572972740F9EB9C9CFCA9D6B7A6D2BCC7F9910CD37201DED61E`；`07-scope-static-fixture-green.log`、`07-scope-static-fixture-green-result.json` |
| `cargo test --offline -p plugin-runtime --test language_configuration -- --nocapture` | `3/0/0`，行为 0.34 秒、总 5.06 秒 | `AC33771533A5D5212D135D5714380CB386DD12E1F000200DEACDCAFC73A44131`；`07-language-configuration-cold-adapter-green.log`、`07-language-configuration-cold-adapter-green-result.json` |
| `cargo test --offline -p plugin-runtime --test language_configuration --test managed_dependencies --test optional_language_dependencies -- --nocapture` | 总 `11/2/1`、exit 101、8.20 秒；language `3/0/0` / 0.33 秒，managed `8/0/1` / 2.12 秒，尚未适配的 optional `0/2/0` / 0.08 秒 | language `FD2BED1D8614540F1F4A1976B18BAE75A7B129EC087413F856574F09D524089A`；managed `B0D26A4197DCBC0B843251E3083539BA6DD855F845B0A800A1035D9289E44B27`；`07-cold-dependency-adapters-targeted.log`、`07-cold-dependency-adapters-targeted-result.json`、`07-cold-dependency-adapters-targeted-sections.json` |
| `cargo test --offline -p plugin-runtime --test optional_language_dependencies -- --nocapture` | `2/0/0`，行为 0.12 秒、总 4.44 秒 | `C934768B537AB6178179259B1B308345BDBF0980D173EAF512F51550DEB21FE4`；`07-optional-dependency-cold-adapter-green.log`、`07-optional-dependency-cold-adapter-green-result.json` |

optional 的适配前 Red 在联合日志中真实发生，其旧 EXE SHA 未在重编译前捕获，结果记录明确为 null，不补造指纹。该轮 managed 的一项实际 WASM ignored 未执行，不把 `8/0/1` 写成九项通过；其后的显式执行结果另记于下文。

夹具继续使用公开 Availability 原 token、实际 selected role、`begin_demand`/poll/admit；共享 `tests/support/demand.rs::try_ready` 只组合现有公开接口并保留 typed failure，不增加宿主测试 API、不伪造后台或准入回执。原下载/cache、lease、quota、坏工具/取消错误、信任、长路径和旧版本保留断言均继续检查，失败需求仍可观察为原错误，而非只等待成功。

完整非 UI 门禁的通过覆盖实际执行的 260 项。245 个 ignored 在该门禁中未运行，日志中的 0 tests 输出也不代表新增行为覆盖；随后选定十九项已另行显式执行，不能将其通过扩大为全部 245 项。应用与物理验收仍按各自实际证据记录。

## 门禁后的 Runtime 指纹与十九项计划

`../target01/07-final-runtime-post-gate-eight-fingerprints.json` 保存官方门禁完成时重新编译的八目标；其中 `plugin_services` 已含三处真实 View demand。下表是该时点的输入，随后 SDK 夹具重编译的新指纹单独记录，不覆盖原失败 EXE：

| `../target01/debug/deps/` 文件 | 门禁后 SHA256 |
| --- | --- |
| `activation-102683a7d1d79a8c.exe` | `838CA8326D1D12704CFE3523F65BF19136F61261513901DE38DAE1EA9551B265` |
| `activation_native_hot-67fb4cbe5aef6e64.exe` | `8D135D983F46A894D9933078CE0F518D5820C89BE88E76BF59FDA78A3E48569B` |
| `cloud-8b2301a2340f94b6.exe` | `15097CB12CCADDA0253003786910745C98357EB271C919DC9F71B49404980CDE` |
| `development_projects-7b8856594159362d.exe` | `1A5C53ED9DF9BD96DCFC9C329E7814A4F8DAA9D05C4AC6E7FD9E811379B754AC` |
| `language_declarative-95d3220f5980e3b8.exe` | `2BDBC59D088584959AF66ED5A512C3B56A7FEF75114BA77074029A69BC7A5B45` |
| `plugin_services-fd7740afef716deb.exe` | `052B76FA9A830893DD0772438196197369DE737E0599C0C5E826D836739F1676` |
| `sdk_lifecycle-869c97e3114cd1d1.exe` | `9F6EAA93B0F10CFC90911CB2F87978110C8C6FB039813835CE84679419FC43C0` |
| `service_activation-17275e4a35729684.exe` | `70E088713D11966859EA6740BCD6EB8CB0AE07145B89038C3C81EAA166684707` |

`../target01/07-final-runtime-post-gate-matrix-plan.json` 保存每项完整 qualified filter、EXE 与 hash，已通过 `--list` 核对十八项 exact 名称：activation 2、sdk_lifecycle 3、service_activation 4、plugin_services 2、cloud 5、development_projects 2。这里只准备并确认运行计划，没有行为通过计数；后续实际执行须使用相应 EXE 的 `--exact --ignored --nocapture --test-threads=1`，T32/T34 的 `scenarios::` 前缀和 cloud 子模块前缀不能省略。waiting-ready 原调用者取消的后续 Green 现由矩阵 case8 实际结果给出，不能用门禁后编译或嵌套单项 Green 替代。

随后 Spec 指出 `managed_dependencies::hook_plan_receipts_accumulate_and_dynamic_program_skips_unused_downloads` 仍依赖旧 eager 前提，这正是此前 managed `8/0/1` 的一项 ignored，当时尚无实际 receipt Green。仅为该例四阶段补原 token 的真实 LSP demand，保留原 cache collect 零值、receipt 累积、未使用 URL、ignore 和 deadline 断言；未增加宿主 API。

`../target01/07-final-runtime-nineteen-matrix-plan.json` 在原十八项 inventory 之外追加这一项，使用实际旧 SDK `target/plugin-api-test/capability-example.zip`，SHA256 `BDFC5D3327B5B7A4F3D25BA1AE95E2A6A47B62634FB040E17FB4430C9AA46773`。新增项最初 EXE、hash 与已核 qualified 数均为 null，未把源码修改或 inventory 当通过。随后 Core 的 `cargo test --offline -p plugin-runtime --test managed_dependencies --no-run` 实际 exit 0、总 5.44 秒，`07-managed-hook-current-no-run.log` 与 `07-managed-hook-current-no-run-result.json`；EXE `managed_dependencies-312805e9ee4f148a.exe` SHA256 `22561933309920835357DBD4B423BE5783F050B28595B7E4B30071403DEA780D`。计划已补这个指纹，exact qualified 的 `--list` 核对为一项；十九项核对记录为 `07-final-runtime-nineteen-exact-list-fingerprints.json`，不把名单当行为通过。

Hook 的首次显式行为运行已取得 Green。实际直接执行上述 EXE，完整过滤词 `hook_plan_receipts_accumulate_and_dynamic_program_skips_unused_downloads` 加 `--exact --ignored --nocapture --test-threads=1`，`1/0/0`、exit 0；行为 263.30 秒、总 263.3302364 秒。证据为 `../target01/07-runtime-matrix-case19-managed_dependencies-current.log` 与同名 `-result.json`，EXE SHA256 为上述 `22561933...780D`。此前 ignored 现在确实执行；当时只有这一项矩阵通过，不能写成十九项全部通过。

随后 T32 使用完整过滤词 `scenarios::t32_sdk_cloud_hot_configuration_changes_live_io_and_preserves_scope_and_validation` 与相同的四个参数运行，三次均实际失败：

| 阶段 | `passed / failed / ignored`、exit | 行为 / 总时长 | 测试 EXE SHA256 | `../target01/` 日志与同名 `-result.json` |
| --- | --- | --- | --- | --- |
| 当前矩阵首次运行 | `0/1/0`、101 | 217.27 / 217.3056622 秒 | `9F6EAA93B0F10CFC90911CB2F87978110C8C6FB039813835CE84679419FC43C0` | `07-runtime-matrix-case3-sdk_lifecycle-current.log` |
| 修正公开错误码显示判据后 | `0/1/0`、101 | 428.63 / 428.6618922 秒 | `165C1788F01DF6B161A1412278FBCB7A4B961E3D1E09CC7BD50D6DE0E9C8DDAC` | `07-runtime-matrix-case3-sdk_lifecycle-fixture-green.log` |
| 明确等待阶段的诊断运行 | `0/1/0`、101 | 426.79 / 426.820716 秒 | `B52BD8DAB9D0F3A7EF58B041698E2CA2962CB1B3A22784DD30D0B443279B0204` | `07-runtime-matrix-case3-sdk_lifecycle-phase-probe.log` |

首轮检查发现 `/stall` 原判据 `Timeout` 与公开 `ErrorCode::TimedOut` 的 SDK 显示不一致，仅将夹具改为该枚举的 Debug 显示值。第二轮仍超时，文件名中的 fixture-green 不代表通过，字符串不一致不能记为整项失败的已确认根因；两次原日志只定位到 `sdk_lifecycle.rs::pump_until`，尚未显示具体插件及等待阶段。首轮原 EXE 按精确字节封存 `../target01/07-runtime-red-artifacts/sdk-lifecycle-9F6EAA93-before-stage-probe.exe`，没有修改生产或 Guest 来掩盖失败。

诊断夹具为等待增加 `#[track_caller]` 和插件/阶段/对应服务器参数；仅失败时输出公开 SDK 文本、instance_id、resource_count 和 server.count，不输出认证头、凭据或配置 JSON。200ms HTTP 时限、10s 等待及全部资源/owner/认证/旧 Input/scope 断言保持原样。`cargo test --offline -p plugin-runtime --test sdk_lifecycle --no-run` 实际 exit 0、总 4.0333789 秒，`07-sdk-phase-current-no-run.log` 与 `07-sdk-phase-current-no-run-result.json`；EXE 为上表 `B52BD8DA...0204`。第三次实际运行明确失败于 `code-host-query / held-request-start`，公开文本 `InvalidRequest: Invalid HTTP request or bounds`，服务端 HTTP 请求计数 0、资源数 1，原业务 instance 仍在；没有把正常拒绝当成 HotApply owner 变化。

据此只缩小到同一真实 code-host-query 0.1.2 包的公开 Manager 用例 `scenarios::query_sdk_rejects_unicode_header_before_transport_and_preserves_unicode_response`。它通过冷安装、原 token Demand/admit 取得 owner，在相同 `/plain` URL 上先发送 `hello 世界😀`，实际看到公开 `InvalidRequest`、服务端 HTTP 请求计数 0、资源保持基线；仅将 text 换成 `Nanobug` 后看到一个 GET、空 body、Complete 与 `hot 世界😀` 响应，owner 和资源基线保持，shutdown 后资源零。`endpoint::Server::count` 统计完整 HTTP Capture；传输前拒绝的代码依据另来自冻结 SDK 的 [http.rs](../../../../crates/plugin-protocol/src/http.rs)：Guest `http::start` 先执行 `Request::validate`，非法头值在发出宿主 `Http::Start` Operation 前返回错误。

该独立窄用例确实执行，命令为实际 SDK EXE 加上述完整 qualified 与 `--exact --ignored --nocapture --test-threads=1`，`1/0/0`、exit 0，行为 51.25 秒、总 51.2777442 秒；`../target01/07-sdk-query-header-actual-probe.log` 与 `07-sdk-query-header-actual-probe-result.json`。EXE SHA256 `6ED437F07235143FDB6A5EA4EB51543965AAECBBB80FA37F42E0246BF1EEC41E`，其 `--no-run` exit 0、总 4.2151113 秒，证据 `07-sdk-query-input-current-no-run.log` 与同名 `-result.json`。这组负/正控确认第三次 held-request-start 的夹具输入原因：GET 插件将原 text 放入只接受 ASCII 的请求头；正常拒绝是该窄用例的预期通过，不是新增产品 Red。

T32 仅按实际请求领域区分输入：POST 仍发送 `hello 世界😀`，GET 使用已有 `Nanobug`；两个消费者仍断言 Unicode 响应，所有 200ms 时限、HotApply 同 owner、认证、资源、旧 Input、scope 隔离边界保持。生产、Guest、SDK 及 ZIP 没有修改或重建。统一队列随后从同一个 `6ED437F07235143FDB6A5EA4EB51543965AAECBBB80FA37F42E0246BF1EEC41E` EXE，使用完整 T32 qualified 与四个参数实际取得 `1/0/0`、exit 0，行为 741.27 秒、总 741.3050562 秒；`../target01/07-runtime-matrix-case3-sdk_lifecycle-ascii-domain-current.log` 与同名 `-result.json`。这是两个新 SDK 消费者的完整 T32 Green，额外窄 probe 的 `51.25 秒 / 1/0/0` 保持独立记录，前三次 Red 均保留。

同一 `6ED437F0...C41E` EXE 接着实际执行 SDK case4/case5，使用表中完整 qualified 加 `--exact --ignored --nocapture --test-threads=1`；结果分别由对应 JSON 和日志核对：

| case / qualified | `passed / failed / ignored`、exit | 行为 / 总时长 | `../target01/` 日志与同名 `-result.json` |
| --- | --- | --- | --- |
| 4：`scenarios::t34_delivered_old_sdk_minor_optional_and_required_host_floor_are_real_archives` | `1/0/0`、0 | 121.76 / 121.7942117 秒 | `07-runtime-matrix-case4-sdk_lifecycle-current.log` |
| 5：`scenarios::t34_sdk_managed_migration_failure_recovers_latest_private_data` | `0/1/0`、101 | 79.22 / 79.2440391 秒 | `07-runtime-matrix-case5-sdk_lifecycle-current.log` |

case4 实际使用前述 0.1.1 的旧 SDK WASM 字节：稳定 configuration minor 仍运行并接收 Unicode 响应；缺少协商的 optional HTTP 能力给出 `CapabilityUnavailable`，服务端 HTTP 请求计数及资源不增加；required 未来稳定/实验能力、过高最低宿主及历史 protocol 声明经真实包重新准入时拒绝。旧 SDK 输入没有重建，兼容正控与拒绝边界都在此完整用例中执行。

case5 原 Red 真实失败于记录时 `lifecycle_scenarios.rs:528` 的 `prepare_installation(...).unwrap()`，错误链为 `OperationFailed: Fixture rejected migration after a staged write`。此时尚未抵达 prepared token 返回之后的旧 Guest 写入 latest 或 commit 阶段断言，不能把失败误记为后面的 scope-write。契约定位另依据现有 [preparation.rs](../../../../crates/plugin-runtime/src/manager/preparation.rs)：准备阶段在可丢弃的私有数据副本上预演 migration，切换时再从最新数据副本执行最终迁移。随后 Core 仅修正一个 cfg(test) 文件的阶段前提，生产、Guest、SDK 和包未变：migration 拒绝支先由旧 Guest 写 latest，确认 prepare 的确切公开错误、旧 owner 不变及旧版本/私有 latest 保留；activation 拒绝支先写 initial 并成功 prepare，再写 latest 后确认 commit 错误与 fresh owner 恢复。两支均保留 registry 1.0.0、后续成功 v2:latest、项目源文件不变及 shutdown 零资源断言。

迁移阶段夹具修正后的 `cargo test --offline -p plugin-runtime --test sdk_lifecycle --no-run` exit 0、总 4.2897609 秒，`../target01/07-sdk-migration-stage-current-no-run.log` 与 `07-sdk-migration-stage-current-no-run-result.json`；新 EXE SHA256 `F8FD99D72BE288AAD8D464FB1D5E1D5C3F638CAF8936E357C2733903733FD510`。随后从它使用 SDK5 完整 qualified 与四个参数实际取得 Green `1/0/0`、exit 0，行为 160.76 秒、总 160.7874744 秒；`07-runtime-matrix-case5-sdk_lifecycle-migration-stage-current.log` 与同名 `-result.json`。原 79.22 秒 Red 保留，不重复已有 `6ED437F0...C41E` 的完整 T32 和 SDK4 Green，也不把它们记为新 `F8FD99D7...F510` EXE 的运行。

随后十九项矩阵全部取得真实 `19/0/0`，每项 exact 名称恰一项、显式运行 `--ignored`，没有 0 match 冒充通过。窄 Query probe 的 `1/0/0` 另计；下表保存实际输入与每项日志，历史四次 Red 不覆盖。

阶段 `cargo fmt --check` exit 0、2.87 秒，`../target01/07-fmt-check.log` 与 `07-fmt-check-result.json`。这是该阶段的格式检查，不代表之后修复的最终三门禁完成。

### 十九项实际终态、源码与可复核输入

唯一实际清单为 `../target01/07-final-runtime-nineteen-actual-manifest.json` 与 `.tsv`，生成于 2026-10-11T20:29:06.1411785Z；其中保存原完整命令、qualified、全 EXE hash、log/JSON、两种耗时与 retainedEXE。`07-runtime-nineteen-current-results.json` 另保留 23 次矩阵尝试，包括 T32 三次及 SDK5 一次历史失败。逐项读取日志的 test result 与 JSON，十九项均为 `1/0/0`、exit 0；每项 exact --list 恰一项。下列日志均位于 `../target01/`，各自存在同名 `-result.json`；直接执行其实际 EXE 加完整 qualified 和 `--exact --ignored --nocapture --test-threads=1`。

| case | 目标与完整 qualified | passed / failed / ignored | 行为 / 总时长 | 实际日志 |
| --- | --- | --- | --- | --- |
| 1 | `activation`：`first_command_demand_coalesces_without_changing_static_identity` | `1/0/0` | 53.30 / 53.3298861 秒 | `07-runtime-matrix-case1-activation-current.log` |
| 2 | `activation`：`pending_waiter_budget_deadlines_and_scope_aba_never_activate_abandoned_intent` | `1/0/0` | 26.79 / 26.8245316 秒 | `07-runtime-matrix-case2-activation-current.log` |
| 3 | `sdk_lifecycle`：`scenarios::t32_sdk_cloud_hot_configuration_changes_live_io_and_preserves_scope_and_validation` | `1/0/0` | 741.27 / 741.3050562 秒 | `07-runtime-matrix-case3-sdk_lifecycle-ascii-domain-current.log` |
| 4 | `sdk_lifecycle`：`scenarios::t34_delivered_old_sdk_minor_optional_and_required_host_floor_are_real_archives` | `1/0/0` | 121.76 / 121.7942117 秒 | `07-runtime-matrix-case4-sdk_lifecycle-current.log` |
| 5 | `sdk_lifecycle`：`scenarios::t34_sdk_managed_migration_failure_recovers_latest_private_data` | `1/0/0` | 160.76 / 160.7874744 秒 | `07-runtime-matrix-case5-sdk_lifecycle-migration-stage-current.log` |
| 6 | `service_activation`：`cold_debug_start_waits_for_its_original_provider_birth` | `1/0/0` | 53.02 / 53.0788547 秒 | `07-runtime-matrix-case6-service_activation-current.log` |
| 7 | `service_activation`：`cold_native_execution_binds_its_pending_session_once` | `1/0/0` | 52.73 / 52.7634085 秒 | `07-runtime-matrix-case7-service_activation-current.log` |
| 8 | `service_activation`：`cold_service_metadata_and_first_call_preserve_reference_and_original_deadline` | `1/0/0` | 131.68 / 131.7130404 秒 | `07-runtime-matrix-case8-service_activation-current.log` |
| 9 | `service_activation`：`queued_cold_service_keeps_its_configuration_while_open_reference_survives_hot_apply` | `1/0/0` | 191.03 / 191.0553691 秒 | `07-runtime-matrix-case9-service_activation-current.log` |
| 10 | `plugin_services`：`service_calls_preserve_source_and_cannot_borrow_provider_authority_or_reenter` | `1/0/0` | 104.98 / 105.0180523 秒 | `07-runtime-matrix-case10-plugin_services-current.log` |
| 11 | `plugin_services`：`delegated_resources_and_async_continuations_follow_the_original_source_lifetime` | `1/0/0` | 183.30 / 183.3351087 秒 | `07-runtime-matrix-case11-plugin_services-current.log` |
| 12 | `cloud`：`ownership::t30_backpressure_cancel_disable_and_trust_revoke_stop_workers` | `1/0/0` | 61.13 / 61.1678427 秒 | `07-runtime-matrix-case12-cloud-current.log` |
| 13 | `cloud`：`ownership::t29_timers_are_fair_bounded_disposable_and_debounce_resets_delay` | `1/0/0` | 81.29 / 81.3181401 秒 | `07-runtime-matrix-case13-cloud-current.log` |
| 14 | `cloud`：`secret_publication::t27_t30_unpublished_snapshot_and_publish_failure_preserve_system_token` | `1/0/0` | 326.05 / 326.0776707 秒 | `07-runtime-matrix-case14-cloud-current.log` |
| 15 | `cloud`：`temporary_ownership::t29_t30_temporary_owner_survives_rejected_configuration_and_cleans_on_retirement` | `1/0/0` | 172.33 / 172.3622291 秒 | `07-runtime-matrix-case15-cloud-current.log` |
| 16 | `cloud`：`updates::t30_failed_cloud_update_retires_candidate_and_recovers_data_without_replaying_http` | `1/0/0` | 99.64 / 99.665729 秒 | `07-runtime-matrix-case16-cloud-current.log` |
| 17 | `development_projects`：`real_directory_reload_preserves_state_and_rolls_back_failed_component` | `1/0/0` | 60.41 / 60.4442365 秒 | `07-runtime-matrix-case17-development_projects-current.log` |
| 18 | `development_projects`：`real_directory_new_permissions_require_confirmation` | `1/0/0` | 30.13 / 30.1570011 秒 | `07-runtime-matrix-case18-development_projects-current.log` |
| 19 | `managed_dependencies`：`hook_plan_receipts_accumulate_and_dynamic_program_skips_unused_downloads` | `1/0/0` | 263.30 / 263.3302364 秒 | `07-runtime-matrix-case19-managed_dependencies-current.log` |

上述十九项的实际 EXE 输入如下。SDK5 的 cfg(test) 阶段前提修正不改变 T32/SDK4 的用例与共享 helper；它们的原 EXE 已按原字节保留，未把相同路径的后续覆盖当作相同输入。以下 retainedEXE 实际 SHA 全部再次核对匹配记录。

| case | 相对于 `../target01/` 的 retainedEXE | 完整 SHA256 |
| --- | --- | --- |
| 1、2 | `debug/deps/activation-102683a7d1d79a8c.exe` | `838CA8326D1D12704CFE3523F65BF19136F61261513901DE38DAE1EA9551B265` |
| 3、4 | `07-runtime-red-artifacts/sdk-lifecycle-6ED437F0-before-migration-stage-fix.exe` | `6ED437F07235143FDB6A5EA4EB51543965AAECBBB80FA37F42E0246BF1EEC41E` |
| 5 | `debug/deps/sdk_lifecycle-869c97e3114cd1d1.exe` | `F8FD99D72BE288AAD8D464FB1D5E1D5C3F638CAF8936E357C2733903733FD510` |
| 6、7、8、9 | `debug/deps/service_activation-17275e4a35729684.exe` | `70E088713D11966859EA6740BCD6EB8CB0AE07145B89038C3C81EAA166684707` |
| 10、11 | `debug/deps/plugin_services-fd7740afef716deb.exe` | `052B76FA9A830893DD0772438196197369DE737E0599C0C5E826D836739F1676` |
| 12、13、14、15、16 | `debug/deps/cloud-8b2301a2340f94b6.exe` | `15097CB12CCADDA0253003786910745C98357EB271C919DC9F71B49404980CDE` |
| 17、18 | `debug/deps/development_projects-7b8856594159362d.exe` | `1A5C53ED9DF9BD96DCFC9C329E7814A4F8DAA9D05C4AC6E7FD9E811379B754AC` |
| 19 | `debug/deps/managed_dependencies-312805e9ee4f148a.exe` | `22561933309920835357DBD4B423BE5783F050B28595B7E4B30071403DEA780D` |

三个 T32 失败 EXE 分别保留为 `07-runtime-red-artifacts/sdk-lifecycle-9F6EAA93-before-stage-probe.exe`、`sdk-lifecycle-165C1788-after-predicate.exe`、`sdk-lifecycle-B52BD8DA-before-query-input.exe`；其 SHA256 与上文三 Red 表逐一相同。原 `6ED437F0...C41E` 字节保留为 `sdk-lifecycle-6ED437F0-before-migration-stage-fix.exe`，同时承载完整 T32/SDK4 Green 和 SDK5 原 Red，不能因为归档目录名含 red 就抹去其有效 Green。当前 SDK5 使用 `F8FD99D7...F510`，复核命令由唯一清单的 reproduce_command 按 retainedEXE 给出。

包输入另保存于 `../target01/07-runtime-nineteen-fixture-inputs.json`：0.1.2 两云包、0.1.1 两旧云包及 capability-example 0.19.0/0.19.1 的完整 SHA256 均与本文固定输入一致。未为夹具修正重建生产、Guest、SDK 或 ZIP。HTTP header 窄 probe 的一项 Green 独立于十九项。

该时点源码指纹清单为 `../target01/07-final-runtime-verified-source-fingerprints.json`，清单 SHA256 `0C0928E882E34E84ACC8277F4D737FFCD14A015AE07AC7AA20A795CAF5DD4176`，记录于 2026-10-11T20:33:49.8894719Z，绑定源码审查 snapshot `5eaf5604854d1cb0fc3d3d36592807d8874ecd15`；二十文件全 SHA256 如下；后续应用测试及 FileView 修复的新快照另记，不把此表当作移动中的最终源码。旧 `07-final-runtime-source-delta-fingerprints.json` 的十六文件范围保留为历史：十五份字节仍相同，唯 `managed_dependencies.rs` 原 `32A819D3...541D` 早于 `b5e366a → 40e0ce41` 的四次真实 LSP demand 修正，当前 `FC8EE8E1...4A4E` 与该已审查点一致。该已知测试差异已有 Hook 的 263.30 秒实际 Green 与两轴零问题，不当作新生产变更，也不将旧 hash 冒充当前输入。

| 文件 | 该时点清单完整 SHA256 |
| --- | --- |
| `crates/plugin-runtime/src/request_state.rs` | `E1EABB0FF023E7D23439732BB3772DE54C772D96434B1B5D0BB4BEA22EC70145` |
| `crates/plugin-runtime/src/manager/activation/driver.rs` | `4208065EBF39207BF274079BA68A4F21A3DD189AF03869E008F4A693EF54371F` |
| `crates/plugin-runtime/src/manager/plugin_services/demand_calls.rs` | `14A3FD8678422337C87780578EA647CBBE1923E6779F0DFA8C8527DF566A9064` |
| `crates/plugin-runtime/src/manager.rs` | `B8C5AE1050C04F483E54E657F193A3A0D170EE583A9A6DC6D379F0B5E7CA91C9` |
| `crates/plugin-runtime/tests/service_activation.rs` | `A260E14C1B131D49007CDDC80372C97A5F9031D29B9DDFA5EEB753F3D3DAF630` |
| `crates/plugin-runtime/tests/service_activation/nested.rs` | `265E9777199A7C59522F81E63AEBBE8A58B4E57D74070F80D083C76CA470824E` |
| `crates/plugin-runtime/tests/plugin_services.rs` | `A595C3675313B30E747BC166BE3211FAFEC344B32BDD8D8A83BDB62F48344A4A` |
| `crates/plugin-runtime/tests/support/demand.rs` | `4725CF7069CE36A97CA1BE956A6B5BD92EA3A395C416B02A95EDA4F5910C6DD8` |
| `crates/plugin-runtime/tests/language_configuration.rs` | `42D97B7EF00D98E789843517A58E9BAD66EDC0C45A4AA5513B5C62540F7D4F61` |
| `crates/plugin-runtime/tests/managed_dependencies.rs` | `FC8EE8E1C3A67447EC8341DE485F0E3520E18B3A2E2F2FDE39C9B95482404A4E` |
| `crates/plugin-runtime/tests/optional_language_dependencies.rs` | `D0D0903EF7E5167ABAFACCCCD41E662A33947CC3D1EEB0C35FB15CC88881265B` |
| `crates/plugin-runtime/tests/fixtures/demand-relay/src/lib.rs` | `99ABB7614A15E6DCEDDD52C3E1B739D3028CA899BB4A3BE766EEDA4736E2A3CE` |
| `crates/plugin-runtime/tests/fixtures/demand-relay/nanobug-plugin.json` | `DAC7E281A1F5761E1612C68C14673782B5D7D83E1C233267A48A32AF6431477D` |
| `crates/plugin-runtime/tests/fixtures/demand-relay/manifest.json` | `EB0E81297B3D8C7B74887B256913743671F02BFFFF007A1A7F1C2B41F54A819F` |
| `crates/plugin-runtime/tests/fixtures/demand-relay/Cargo.toml` | `ADABE38C18FF4B57F9890E6B6DB67C7B945BAD74029E200F8CAA87BF81935DCF` |
| `crates/plugin-runtime/tests/fixtures/demand-relay/Cargo.lock` | `492A0A37E58DFB237494EC0AA3ECDE73B672B84BD6384D103F64B2F62133B05D` |
| `crates/plugin-runtime/tests/sdk_lifecycle.rs` | `D92EF23E5F7EF952B8D7A2263E2AA1B75A3C5EA6E11470923E7C63C34ECA8D6A` |
| `crates/plugin-runtime/tests/support/lifecycle_scenarios.rs` | `FE8855F560401082C6C7D0E9E78D19D429BA5E4477F3D332C60933E67383B816` |
| `crates/editor-app/src/extensions/interaction_tests.rs` | `BB73104C91AFF709926B272832F2F3B17E42578C487BAAD2B50E5FBFEDD5481C` |
| `crates/editor-app/src/extensions/activation_tests.rs` | `08D05165A862E92514BEDF356C7243F96D01A1D57844DF3E32A5EE1BEEDC55A0` |

### 本轮规定门禁实际结果

| 实际命令 | 结果 / 总时长 | `../target01/` 日志与 JSON |
| --- | --- | --- |
| `cargo fmt --check` | exit 0 / 2.7946458 秒 | `07-final-runtime-fmt-workspace.log`、`07-final-runtime-fmt-workspace-result.json` |
| `cargo test --workspace --exclude editor-app` | `260/0/245 ignored`、75 binaries、exit 0 / 27.419173 秒 | `07-final-workspace-non-ui-default-retry3.log`、`07-final-workspace-non-ui-default-retry3-result.json` |
| `cargo check --workspace` | exit 0 / 5.436492 秒 | `07-final-runtime-check-workspace.log`、`07-final-runtime-check-workspace-result.json` |

格式和 Workspace check 在十九项完成后实际执行。非 UI 门禁为前述默认并发的完整 retry3；其后仅 ignored 测试夹具变化，未重复无受影响输入的全门禁。245 项 ignored 仍只表示未在默认门禁中执行，应用与物理验收不由三门禁推导。

本轮应用 `cargo check --offline -p editor-app` 首次因 CommandGuard 的 Result 错误嵌套匹配 exit 101、8.50 秒，`07-app-check-current.log` 与 `07-current-app-build-results.json`；仅修正匹配后 retry exit 0、6.61 秒，`07-app-check-current-retry.log` 与 `07-current-app-retry-build-results.json`。随后的 `cargo test --offline -p editor-app --no-run` 因三处测试编译错误 exit 101、37.88 秒，`07-app-no-run-current.log`；又一次 retry 因两处测试临时值借用错误 exit 101、14.62 秒，`07-app-no-run-current-retry.log` 与 `07-current-app-final-build-results.json`。两次都是编译失败，不计作行为 Red。

后续修正后的真实编译结果保存于 `../target01/07-current-app-complete-build-results.json`，三项均 exit 0：

| 实际命令 | 总时长 | `../target01/` 日志 |
| --- | --- | --- |
| `cargo test --offline -p editor-app --no-run` | 90.14 秒 | `07-app-no-run-current-final.log` |
| `cargo build --offline -p editor-app` | 49.44 秒 | `07-host-build-current.log` |
| `cargo build --offline --manifest-path ../native-qa-07/setup/Cargo.toml` | 32.38 秒 | `07-native-helper-build.log` |

前两项分别编译应用测试与宿主；第三项仅编译仓库外、使用公开 Manager 的隔离 QA 辅助程序。它们都不包含行为测试或物理验收。`07-app-before-nested-fingerprint.json` 保存嵌套等待修复前应用测试 EXE `editor_app-bf8b72a9198c679b.exe` 的 SHA256 `9118E4AEB25930A802FB31DFB6CFD86453DDE70E6933ADB24BE35F3D2F3590F6`。

`../target01/07-current-native-binary-fingerprints.json` 的宿主 `debug/editor-app.exe` SHA256 为 `1AE9DDD39C18FD8639636AFE242880E6773DC18432BADA6118F5A7E7BD32DC27`，辅助程序 `debug/community-native-qa-lifecycle.exe` 为 `F160E0D9023A6E3BC0DB160FC2433E0480417A952BDC56188591B03466645F27`；记录当时实际文件再次读取哈希一致。这两份字节早于上文祖先取消/自然 deadline 门禁修复，仅保留阶段历史；之后的新构建与实际应用结果见下一节。

## 新宿主构建、SDK 字节核对与当前应用结果

`../target01/07-root-final-build-results.json` 保存随后三项实际构建，均 exit 0；文件名中的 final 不表示工单已完成验收：

| 实际命令 | 总时长 | `../target01/` 日志 |
| --- | --- | --- |
| `cargo test -p editor-app --no-run` | 102.22 秒 | `07-root-final-app-no-run.log` |
| `cargo build -p editor-app` | 46.34 秒 | `07-root-final-host.log` |
| `cargo build --offline --manifest-path ../native-qa-07/setup/Cargo.toml` | 21.60 秒 | `07-root-final-native-helper.log` |

`../target01/07-root-final-binary-fingerprints.json` 的实际输入：

| 文件 | SHA256 |
| --- | --- |
| `debug/deps/editor_app-bf8b72a9198c679b.exe` | `59602E9D06B2224D7D7ADCC10450658F355C5C064DE661A49DA5657DEA30CA25` |
| `debug/editor-app.exe` | `1D29DCFF353F81D581B5B1A0AD66A2AC6D271E11EC7C1840496371943C58E266` |
| `debug/community-native-qa-lifecycle.exe` | `2F5A43CEE1E9427808E5262A87B9E5B0C7E82D6BD2DE1F0119BAE09250D2BBDA` |

新宿主实际执行 `editor-app.exe --export-plugin-sdk ../target01/sdk07-final-1d29`，exit 0、0.24 秒；`07-sdk-final-host-export.log` 与 `07-sdk-final-host-export-result.json`。导出 50 文件、0 Markdown，与已冻结 SDK 50 的五十份文件逐字节 SHA 全部一致，内容摘要仍为 `488a6baccbb61c29dc6f5020f22bac1b813a0529dbf3a7768c9f760a9245f5fd`。接口输入未变化，继续复用此前真实独立构建包，不因宿主重链接重建无变更 WASM；字节核对不等于包行为或物理验收通过。

应用十二项名字 inventory 见 `../target01/07-root-final-app-test-plan.json`，每个 qualified 在 `--list` 中恰一项。首次使用同一 `59602E9D...` EXE，命令为记录中的完整 qualified 加 `--exact --ignored --nocapture --test-threads=1`；该轮只执行前三项，`07-root-app-current-green-results.json` 与对应日志保存真实结果：

| qualified | `passed / failed / ignored`、exit | 行为 / 总时长 | `../target01/` 日志 |
| --- | --- | --- | --- |
| `extensions::worker::worker_tests::cold_commands_are_discoverable_without_starting_the_guest` | `1/0/0`、0 | 27.39 / 27.45 秒 | `07-root-app-green-cold_commands_are_discoverable_without_starting_the_guest.log` |
| `extensions::worker::worker_tests::cold_command_first_birth_preserves_its_original_command_epoch` | `1/0/0`、0 | 54.78 / 54.81 秒 | `07-root-app-green-cold_command_first_birth_preserves_its_original_command_epoch.log` |
| `extensions::interaction_tests::native_plugin_menu_rejects_an_edit_while_its_original_command_waits` | `0/1/0`、101 | 55.19 / 55.23 秒 | `07-root-app-green-native_plugin_menu_rejects_an_edit_while_its_original_command_waits.log` |

第三项在 `interaction_tests.rs:90` 的“旧排队菜单命令不得到达访客业务代码”断言真实失败，原日志名称虽然含 green，不能记作通过。随后仅修正夹具，用真正的原生 Input 提交原文档文本修改，保留旧意图拒绝和新有效请求正控。`cargo test -p editor-app --no-run` exit 0、总 62.0432889 秒，`07-app-native-input-no-run.log` 与 `07-app-native-input-no-run-result.json`；新 EXE SHA256 `513D5DBF2611C81990D7FC4CE07FC94E79027A966707AFFCFCDA34756780B499`。从该 EXE 使用相同 qualified 与四个参数实际取得 `1/0/0`、exit 0，行为 54.23 秒、总 54.2974866 秒；证据 `07-app-native-input-green.log` 与 `07-app-native-input-green-result.json`。原 55.19 秒失败继续保留，前两项有效 Green 仍绑定原 `59602E9D...` 输入，不重复执行或写成新 EXE 通过。宿主 `1D29DCFF...` 与 helper `2F5A43CE...` 未因这次测试改动重建，当时仅为候选；后续最终生产 Host 与物理结果另记，不沿用该旧 Host 冒充本次输入。

应用第四项 `extensions::activation_tests::native_cold_menu_and_view_wait_keep_exact_owner_without_eager_birth` 随后实际执行。直接从上述 `513D5DBF...B499` EXE 使用该完整 qualified 与四个参数得到 Red `0/1/0`、exit 101，行为 27.26 秒、总 27.3005703 秒；`../target01/07-app-remaining-native_cold_menu_and_view_wait_keep_exact_owner_without_eager_birth.log` 与摘要 `07-root-app-remaining-nine-results.json`。断言 `cold static menu must render` 失败；夹具调用了只有 comparison 时才可打开的菜单，此上下文没有 comparison，正常关闭不能用来证明 cold 菜单已绘制。仅修正 `activation_tests` 的菜单入口，使用正式 Explorer 的实际 specific row，保留全部 cold、clip、paint 和 Opened 断言。

应用第四项修正后的 `cargo test -p editor-app --no-run` 实际 exit 0、总 41.4880622 秒，`07-app-cold-explorer-no-run.log` 与 `07-app-cold-explorer-no-run-result.json`。新 EXE SHA256 `E75F0519C838997639E322EA18787F530A70A4EC3C840D7A4E33147B1B028481`，从它使用同一完整 qualified 与四个参数实际得到 Green `1/0/0`、exit 0，行为 79.98 秒、总 80.0163422 秒；`07-app-cold-explorer-fixture-current.log` 与 `07-app-cold-explorer-fixture-current-result.json`。原 27.26 秒 Red 保留，不把新 EXE 的第四项通过冒充前三项重复执行。该轮完成时已有前四项真实 Green，其后第五项实际 Red 见下一段；此时完整应用矩阵尚未通过，随后最终 `12/0/0` 见下文。

应用第五项 `extensions::settings_hot_tests::native_hot_settings_apply_without_owner_restart_and_keep_only_secret_keys` 从同一 `E75F0519...8481` EXE 使用完整 qualified 与四个参数实际得到 Red `0/1/0`、exit 101，行为 128.06 秒、总 128.1102663 秒；`../target01/07-app-last-eight-native_hot_settings_apply_without_owner_restart_and_keep_only_secret_keys.log` 与 `07-root-app-current-last-eight-results.json`。记录时 `settings_hot_tests.rs:218` 的 apply unwrap 返回 `Native declaration validation rejected the draft`。当时队列在首败停止，计四项 Green、一项失败、七项未运行。随后确认是测试的空字符串 simulateInput 未删除选区，原 129 字节草稿仍被表单正常拒绝；只改 cfg(test) 为真实 Ctrl+A、Backspace 后输入并核对 Work::SetSetting.value，生产授权和清理没有变化。修正后的完整 Green 另记于下文，原 Red 不删除。

### 原十二项已执行结果与失败后复验

原十二项的全部实际行为已执行，最终实际 `12/0/0`、0 项未执行；逐项 JSON/log 的正式清单为 `../target01/07-final-app-twelve-actual-manifest.json`，SHA256 `AABEF5EEB33A384537E29BEB5B941C96D89614E44BE3C33EEA168A4D6D9B6A13`，最终源码点 `2515a09a0a4f2f7640d8b11d71b994571dcf379f`。第 5/10/11/12 项的四次原失败和第十二项追加诊断 Red 均保留；不把多个 EXE 的针对性验证说成一次全文件运行。下表是各项最终有效行为结果，均使用对应实际 EXE、完整 qualified 与 `--exact --ignored --nocapture --test-threads=1`。

| case | 完整 qualified | passed / failed / ignored、exit | 行为 / 总时长 | `../target01/` 日志 |
| --- | --- | --- | --- | --- |
| 1 | `extensions::worker::worker_tests::cold_commands_are_discoverable_without_starting_the_guest` | `1/0/0`、0 | 27.39 / 27.4504867 秒 | `07-root-app-green-cold_commands_are_discoverable_without_starting_the_guest.log` |
| 2 | `extensions::worker::worker_tests::cold_command_first_birth_preserves_its_original_command_epoch` | `1/0/0`、0 | 54.78 / 54.8139026 秒 | `07-root-app-green-cold_command_first_birth_preserves_its_original_command_epoch.log` |
| 3 | `extensions::interaction_tests::native_plugin_menu_rejects_an_edit_while_its_original_command_waits` | `1/0/0`、0 | 54.23 / 54.2974866 秒 | `07-app-native-input-green.log` |
| 4 | `extensions::activation_tests::native_cold_menu_and_view_wait_keep_exact_owner_without_eager_birth` | `1/0/0`、0 | 79.98 / 80.0163422 秒 | `07-app-cold-explorer-fixture-current.log` |
| 5 | `extensions::settings_hot_tests::native_hot_settings_apply_without_owner_restart_and_keep_only_secret_keys` | `1/0/0`、0 | 154.39 / 154.4409512 秒 | `07-app-empty-input-current.log` |
| 6 | `extensions::activation_combination_tests::native_community_activation_combination_retains_source_undo_and_upgrade_recovery` | `1/0/0`、0 | 448.74 / 448.7855421 秒 | `07-app-remaining-seven-native_community_activation_combination_retains_source_undo_and_upgrade_recovery.log` |
| 7 | `extensions::community_language_tests::language_native::native_community_languages_share_reference_navigation_and_document_lifetime` | `1/0/0`、0 | 81.30 / 81.3366027 秒 | `07-app-remaining-seven-native_community_languages_share_reference_navigation_and_document_lifetime.log` |
| 8 | `extensions::community_language_tests::declarative::native_community_language_declarative_features_preview_undo_and_original_seal` | `1/0/0`、0 | 1.78 / 1.8123587 秒 | `07-app-remaining-seven-native_community_language_declarative_features_preview_undo_and_original_seal.log` |
| 9 | `extensions::language_tests::external_completion::native_completion_remains_available_outside_guest_workspace` | `1/0/0`、0 | 53.56 / 53.5970342 秒 | `07-app-remaining-seven-native_completion_remains_available_outside_guest_workspace.log` |
| 10 | `outline::model::tests::lifetime::native_structure_trap_clears_tree_without_another_document_event` | `1/0/0`、0 | 108.21 / 108.2442856 秒 | `07-app-final-three-native_structure_trap_clears_tree_without_another_document_event.log` |
| 11 | `extensions::shortcut_query_tests::lifecycle::shortcuts_real_plugin_lifecycle_epochs_and_restricted_workspace` | `1/0/0`、0 | 138.18 / 138.2177392 秒 | `07-app-final-three-shortcuts_real_plugin_lifecycle_epochs_and_restricted_workspace.log` |
| 12 | `extensions::ui_package_tests::delivered_ui_packages_follow_native_input_and_reclaim_layout` | `1/0/0`、0 | 51.81 / 51.8459783 秒 | `07-app-final-three-delivered_ui_packages_follow_native_input_and_reclaim_layout.log` |

case1/2 仍绑定 `59602E9D06B2224D7D7ADCC10450658F355C5C064DE661A49DA5657DEA30CA25`，case3 为 `513D5DBF2611C81990D7FC4CE07FC94E79027A966707AFFCFCDA34756780B499`，case4 与 case6–9 为 `E75F0519C838997639E322EA18787F530A70A4EC3C840D7A4E33147B1B028481`，case10–12 的修正版 Green 均为 `855EF3859267485F3956615601B619BD2F0E0801BC2D9073015635F116756377`；不冒充这些通过在后续 EXE 上重复执行。case6–10 的真实摘要为 `07-root-app-current-remaining-seven-results.json`，case11 为 `07-root-app-current-last-two-results.json`，case12 为 `07-root-app-current-last-single-results.json`。

正式清单对 case1/2/3 仅声明原指纹和日志保留，原 EXE 已被重编译覆盖，不能声称可执行字节仍在。case4/6–9 的 `E75F0519...8481` 字节保存在 `../target01/07-app-evidence/E75F0519/editor_app-bf8b72a9198c679b.exe`，case5 保留 00A4，case10–12 的 `855EF385...6377` 保存在 `../target01/07-app-evidence/855EF385/editor_app-bf8b72a9198c679b.exe`，`07-app-855-retained-result.json` 保存完整 SHA256；这些 retainedEXE 均再次核对字节 hash 匹配。后续仅四个受影响用例重跑，其余八项已验证输入与控制路径无变化，不重复编译或假称新 EXE 运行。

T35 的 case6 使用此前交付正式包进入 GPUI 的跨功能短组合，实际 `1/0/0`、448.74 秒行为/448.7855421 秒总时长。语言引用和声明式动作 preview/Undo 的 case7/8、工作区外 completion 的 case9 也实际通过。它们验证各自可观察原生控件与生命周期，不将 GPUI TestAppContext 的短组合等同于 Windows 物理桌面全部组合；C03 新增物理交互随后实际完成，范围另见[Windows 物理记录](native-07.md)，仍不扩大为全部八包物理组合。

case5 的修正版从 `00A4DFD98D49853EF000F8DAED13E66B35BDA3D774802B762B04ABEB5425824A` EXE 实际取得 `1/0/0`、exit 0，行为 154.39 秒、总 154.4409512 秒，完成于 2026-10-11T21:22:13.1886637Z；`07-app-empty-input-current.log` 与 `07-app-empty-input-current-result.json`。UTF-8 keyRef/空引用、Guest Validate、冷态不出生、同 owner 与资源边界均保留。原字节保存在 `../target01/07-app-evidence/00A4DFD9/editor_app-bf8b72a9198c679b.exe`，`07-app-00a4-retained-result.json` 保存实际 SHA256；这个文件也承载下面 FileView 诊断 Red。

case10 原失败在记录时 `tests/outline/lifetime.rs:95`，`trapped structure did not revoke its native outline`。旧测试最后一次绘制可能替完成回调清理，不足以证明原请求自己通知和清空；修正版增加真实 Window.observe 完成观察，要求同一 changed 文档、原 Arc inactive、target/pending 清空与树空同时成立，最终绘制后置，不放宽八秒期限。case11 原失败在记录时 `shortcut_query_tests/lifecycle.rs:355` 的 instance_id None unwrap；restart 合法地保持冷态，夹具改为独立公开 View 需求获得替换 owner，再交付原排队请求，旧 epoch/受限工作区断言保留。这两项修正版源码两轴已通过，随后从 `855EF3859267485F3956615601B619BD2F0E0801BC2D9073015635F116756377` EXE 实际取得 Green：case10 为 `108.21 / 108.2442856` 秒，case11 为 `138.18 / 138.2177392` 秒，均 `1/0/0`、exit 0。`07-app-final-three-actual-results.json` 与表中两个日志保存完整命令；case10 挂载到 model 的私有测试模块后，正式 qualified 为 `outline::model::tests::lifetime::native_structure_trap_clears_tree_without_another_document_event`，不要再用原 `tests::outline::...` 过滤词。P2 强观察行为现已实际闭合。

case12 原失败在记录时 `ui_package_tests.rs:83`，`editor-plugin-layout` 不存在。追加的诊断 exact 在 `00A4DFD9...824A` EXE 上仍实际 `0/1/0`、exit 101，行为 42.18 秒、总 42.2379266 秒；`07-app-file-view-cold-probe.log` 与 `07-app-file-view-cold-probe-result.json`，源码点 `cea81174cf8ce085e0967729ba08bfcb0dd59e32`。观察为公开 Manager live=None、所选 FileView 可见、目标及原 DocumentVersion 正确、render document=false、真实工作队列为空。它确认生产的冷文本回退没有发起首次 Demand，未通过手工 Manager 唤醒来绕过缺陷。通用所选冷 FileView 的回退绘制需求修复已冻结，源码两轴均零问题；修正版 actual Green 见下一段，原两次 Red 不覆盖。

从修复点 `2515a09a0a4f2f7640d8b11d71b994571dcf379f` 的新 `855EF3859267485F3956615601B619BD2F0E0801BC2D9073015635F116756377` EXE，case12 完整 exact 实际取得 `1/0/0`、exit 0，行为 51.81 秒、总 51.8459783 秒，完成于 2026-10-11T21:25:50.2306376Z；`07-app-final-three-delivered_ui_packages_follow_native_input_and_reclaim_layout.log` 与 `07-app-final-three-actual-results.json`。复用既有 masked paint probe 后，首次 View 需求、未保存源、canvas/error、恢复与 disable 均在原完整测试中通过，没有手工提前激活。`07-app-final-three-build-results.json` 另记录该点格式 exit 0 / 2.7259697 秒及 `cargo test -p editor-app --no-run` exit 0 / 56.8025442 秒，对应 `07-app-final-three-fmt.log` 与 `07-app-final-three-no-run.log`；两项编译/格式证据独立于行为 Green。之后 App10/11 已在同一新 EXE 顺序取得上述完整 Green，正式原十二项全部收口。

诊断构建另记录为编译证据：`07-app-diagnostic-build-results.json` 中格式 exit 0 / 2.7247082 秒，而 `cargo test -p editor-app --no-run` 因读取 Published 私有 instance_ids 的 E0616 exit 101 / 5.6518488 秒，`07-app-diagnostic-no-run.log`。删除这一私有缓存诊断，继续从公开 Manager 读取 owner 后，retry exit 0 / 58.1017622 秒，`07-app-diagnostic-no-run-retry.log` 与 `07-app-diagnostic-no-run-retry-result.json`；产物为上述 `00A4DFD9...824A`。编译错误不计行为 Red，no-run 不计修正版通过。
原生 QA helper 已编译，另执行其公开 `project` 子命令准备 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/native-qa-07/query-project`，exit 0、总 0.2147234 秒；`07-native-static-project.log` 与 `07-native-static-project-result.json`。helper 输入 SHA256 `2F5A43CEE1E9427808E5262A87B9E5B0C7E82D6BD2DE1F0119BAE09250D2BBDA`，query ZIP `1049771D8AFA45E603901CA7F23423CA37B601BB4FEB58112E8C1AD5FB9349B7`、`wasm_rebuilt=false`。该 project 子命令当时只准备开发项目，后续 translation 冷安装另记于下一段；当时未启动本单窗口或输入；早期 loopback 服务器观察请求数零后已停止。T35 实际包 GPUI 短组合已通过，C03 Windows 新增物理交互的随后真实结果见下一节；普通窗口库存不计作本单原生验收，不从 helper build、项目准备或 SDK export 推导原生 Green。

隔离 profile 随后通过同一公开 QA helper 实际 install 原 cloud-translation 0.1.2 ZIP，exit 0、总 25.4380091 秒；`../target01/07-native-install-translation.log` 与 `07-native-install-translation-result.json`。helper SHA256 `2F5A43CEE1E9427808E5262A87B9E5B0C7E82D6BD2DE1F0119BAE09250D2BBDA`，包 SHA256 `61273EF15C3750B933AD706FC92A5921B5173A98FE60A8D017E9B60CAA06B74B`。日志实际 cold=true；helper 在 install 与 shutdown 后均检查没有 live owner、resource_count=0，并正常退出，没有写入凭据。这个结果只证明隔离 profile 的静态冷安装及清理，不记作 Windows 物理交互或云请求通过。

## 最终规定门禁、生产宿主与 Windows 验收

最终输入是源码快照 `2515a09a0a4f2f7640d8b11d71b994571dcf379f`，tree `c0b72138cb5f78d217ce2216eaff8b01d7ab0533`，不是实际分支的普通交付提交。`../target01/07-pre-final-source-verification.json` 在 2026-10-11T21:31:03.5872983Z 对 `crates/` 与 `plugins/` 1236 个源码文件确认相对该点 delta 0；外部临时索引重试处理 Windows 参数长度限制，真实分支和索引不变。

`../target01/07-final-delivery-build-results.json` 保存以下真实命令，全部 exit 0：

| 实际命令 | 结果 / 总时长 | `../target01/` 日志 |
| --- | --- | --- |
| `cargo fmt --check` | exit 0 / 2.7153891 秒 | `07-final-delivery-fmt.log` |
| `cargo test --workspace --exclude editor-app` | `260/0/246 ignored`、75 binaries、exit 0 / 14.4667054 秒 | `07-final-delivery-non-ui.log` |
| `cargo check --workspace` | exit 0 / 2.6448394 秒 | `07-final-delivery-check.log` |
| `cargo build -p editor-app` | exit 0 / 26.1142877 秒 | `07-final-delivery-host.log` |

此前 245→246 是额外 Query header 窄探针的 ignored 登记；默认门禁仍未执行 ignored，不将 246 项算作通过。该 probe 已另行显式执行 `1/0/0`，十九项 Runtime 和十二项应用的实际执行也分别记录，不从 build、no-run 或 0 tests 推导行为通过。

生产 `../target01/debug/editor-app.exe` SHA256 `E81B1F9733784CCC41B436525DCAF260A5C6EE80451E9CE1DEC056E58B12DBC0`，实际再次读取匹配。它直接执行 `--export-plugin-sdk ../target01/sdk07-native-e81b`，exit 0、0.3399073 秒；`07-sdk-native-final-host-export.log` 与 `07-sdk-native-final-host-export-result.json`。50 个文件、0 Markdown，与旧 `sdk07-final-1d29` 五十份字节全部相同，hash_differences=0，digest `488a6baccbb61c29dc6f5020f22bac1b813a0529dbf3a7768c9f760a9245f5fd`。SDK 输入未变，复用已真实独立构建的新旧包，不把旧包重建成新 SDK 兼容证明，也不重复无变化 WASM 构建。

### C03 Windows 物理交互与明确范围

完整操作、公开启动命令、输入和结果由[Windows 原生验收记录](native-07.md)保存。本机生产窗口 `28576340` 使用上述 E81B Host 和任务自有 `native-qa-07/profile`、两个 workspace 与 query-project，不操作正常用户 profile。没有通过应用内部函数伪造输入；具体通过项如下。

| 实际物理场景 | 可观察结果 |
| --- | --- |
| 首次选择冷 SVG FileView | 原生 source 与 canvas 自动出现，预览缩放 149%；没有先手工 Manager Demand 或插件命令，源文件不写入。 |
| 公开 Settings Apply | User timeout=120000；Project poll_interval 从 1000 改 500。第二条原请求在 deadline 内仍 HTTP 200 / 17 bytes / Receiving，HTTP 请求计数不增加。 |
| 原生 Cancel 与迟到块 | 第三请求开始后 19 秒实际 Cancel，早于 120 秒 deadline；进度撤销，服务器收到 FIN，continue 后迟到第二块不应用。 |
| 两块 UTF-8 与缓存 | 第四请求实际 HTTP 200 / 31 bytes，中文和非 BMP 两块完整；公开 Read cache 显示两行正文，无新增 HTTP。 |
| 中文 IME 与 Guest 回显 | Microsoft Pinyin 实际候选“1你”，按 1 提交；Notes 原生 Input 和 Guest echo 最终均为 `nn你`。 |
| 主题、字体、滚动与键盘 | 浅色中文14px→深色中文16px，15/16 数值实际确认；Settings 滚动、Tab 焦点和 Return 令计数0→1。 |
| 主窗口无障碍与窗口尺寸 | UIA tree/button/edit/text 真实可读；最大化2560×1392→还原1283×821，source、preview、panels、Notes和查询结果保留。 |

第一和第二请求在后续人工动作前超过 120 秒 deadline，自然断开，不算手动 Cancel；第二请求的 HotApply 观察确实在原 deadline 内完成。最终 server18907 的完整 HTTP 请求计数为 `received=4 / completed=1 / disconnected=3 / authorization_headers=0 / rejected=0`，不是 TCP accept 计数；server18906 五项均 0，只说明本轮未发 translation HTTP，不替代双消费者 T32。没有输出认证头内容或写入凭据。

物理界面只使用简体中文；English 与两 palette 的新增配置表单组合由 App5 GPUI 实际验收覆盖，不编造 English 物理通过。T35 的八消费者完整短组合已由 448.74 秒实际包 GPUI 用例完成，不将本轮桌面写成八包全量物理组合。03/04 已记录的 Settings UIA 枚举崩溃限制仍保留，本轮 Settings 只用截图，不枚举；主窗口 UIA 成功不等于修复 Settings 问题。历史 `2026-10-12 05:33:03` disabled 错误继续保留，与本轮后续实际正常流程分开记录。

### 正常停止与磁盘清理

开发控制器公开 stop exit 0，任务 server stop exit 0，没有强杀；Sky 任务窗口消失。2026-10-11T22:00:26.9721095Z 的 `../target01/07-native-final-cleanup-result.json` 显示 exact Host/QA 关联进程 0、18907/18906 监听 0。与 `07-native-workspace-before-final.json` 的四个任务文件比较，byte/hash 全部相同；以下结果由最终 JSON 实际读取。

| 相对 QA 根的文件 | bytes | 前后相同 SHA256 |
| --- | --- | --- |
| `workspace-a/review.nbexpr` | 56 | `AE546B394B5E9BA60FBED16E1754C00FBE1916423E9FDEA5C68D28627EF3479C` |
| `workspace-a/review.nbkv` | 49 | `9F2F8814B1909D15F9D2F6489C2EC54F70421BF9C19C0C3850B70BDEA3A18639` |
| `workspace-b/scope.txt` | 76 | `718AEC54EEC1B10A5F61E2050D5F66A25A56AC871051AC41D7CE916D6FD95947` |
| `workspace-a/cold-preview.svg` | 324 | `E61619522934D2D90E4C3E6242477EB3B270A39172B4F65854E6493AF83418DC` |

私有 `last-query.txt` 位于隔离 profile 的受管 workspace 数据目录，31 bytes，SHA256 `2BCF8DA5E999AD6B137CB09A6F18C18708B909A4680445C4AFAB7DE7D890F0C0`，matchesBothUtf8Chunks=true，严格等于两块 UTF-8 原文；没有写进项目目录。清理和缓存结果不扩大为 Linux/macOS、屏幕阅读器专项或 Settings UIA 的未执行验收。

## 开发候选与既有正式包复用

上述宿主通过公开独立构建入口实际执行：

```powershell
& ../target01/debug/editor-app.exe --plugin-build plugins/example --release
```

exit 0、总 17.86 秒，`../target01/07-development-candidate-build.log` 与 `07-development-candidate-build-result.json`。日志确认 example 0.4.0 使用宿主管理的 `488a6baccbb61c29dc6f5020f22bac1b813a0529dbf3a7768c9f760a9245f5fd` SDK 缓存，而非访客依赖仓库协议路径或附带 SDK 副本。实际 `OK` 行输出的 candidate 原样为：

```text
\\?\C:\Users\Tmiracle\.codex\worktrees\plugin-api-community\Editor\plugins\example\target\nanobug-development\19586d298de67b2a01c5c3c18ca375f81376aea947138200c3a16be87d82ac6d
```

结果 JSON 已保留这个 verbatim 路径；开发目录两条测试实际使用该目录作为 `NANOBUG_DEVELOPMENT_CANDIDATE`，不把 ZIP、日志或移除前缀的猜测路径当作候选。构建本身只证明候选准备；随后矩阵 case17/18 已分别实际取得 `1/0/0`，行为 60.41/30.13 秒，覆盖原 reload 失败恢复、私有数据及新增权限确认边界。

前 04 已交付的六个正式包由 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-language/Editor/target/` 复制至当前 community 工作树对应 `target/` 目录。`../target01/07-reused-delivered-04-packages.json` 保存每个完整 source/destination 路径；两端实际 SHA256 均再次核对等于记录值：

| 相对于各自 `Editor/target/` 的包 | source = destination SHA256 |
| --- | --- |
| `community-language/config-check-0.1.0.zip` | `4E1706A1F5B9FF6BDE2F17AC70F5E3E795A1E452EA5436502C19D0A79BC34F08` |
| `community-language/expression-language-0.1.0.zip` | `6DA3E320240AD32EDCD554962A1283FD63C91DA8C487C75E30C586CA58221024` |
| `community-language/keyvalue-language-0.1.0.zip` | `D4C94C7DB6DF4CDBF76A7ABE07D8BA83E772FAACC188EE867578E26A756E1536` |
| `community-language/spell-check-0.1.0.zip` | `4A341014D25CB220A0CA4D9B2F4C283825CE054BD6E8BEDDDCC816C6DB0F6568` |
| `community-transactions/workspace-converter-0.1.1.zip` | `300C1291BA3830F00803EC226AC39EC291819208E1A03C7F3A4FD564AB4B117F` |
| `community-transactions/workspace-scaffold-0.1.1.zip` | `6CD6CF38A06519003B7215F9E04492CEFAD59328281AF5961B5E977446C5DAED` |

六包均 `wasm_rebuilt=false`，是既有交付字节在当前宿主下的兼容及组合输入，不是新 SDK 50 的重新构建产物。复制和哈希相同本身不证明生命周期行为；随后 T35 实包 GPUI 短组合及语言 case7/8 的实际 Green 已独立记录，不扩大为全部物理桌面验收。

## 独立审查与未决证据

两轴均从 `90ceab915d63c2e6b8cb260c6f28acfc45adc7fc` 独立审查，不将作者自查代替独立轴。最初临时索引保存的 snapshot `87665bbfac71774c61af25253d9e52ee81819c4c` / tree `c0e46e27b46b4e5f3594f0ce16ed324aa8e31eea`，随后审查固定 snapshot `aacb39db23b12b2580f680b2511ae4fad1d03d5e` / tree `bf4f6453556cdf24bcd65ddd99c5b8e15ea2159c`；记录分别为 `../target01/07-review-snapshot.json` 与 `07-review-snapshot-final.json`。这些是可复核的源码快照，分支 HEAD 当时仍为固定基线，未冒充本单普通提交或最终通过点。

Standards 全量查看 147 文件，初期指出 native 错误消息未走 i18n、跨功能测试放在 `extensions/` 两处边界。已补中英八个 keys 并实际搬入 `tests/activation_combination`，上述两项在 `aacb39d` 定点复查无剩余；完整核对 12 类 smell 基线，无值得单独报告的坏味道或剩余硬性违例。此阶段意见不代替后续源码定点复查；最终源码点 2515 的两轴已另行清零，完整最终文档双轴仍待执行。

Spec 在 `aacb39d` 确认此前几项已修正：每个排队 service call 保留原配置 witness，而 open reference 可供后续新调用复用；Hot native plan 以实际方案决定是否保留原 Arc/authority；选中语言失败不执行业务且原 Err 可观察；丢弃任务与 shutdown 撤销原等待。对应已执行行为证据见上文，尚未执行的新用例仍不计通过。

该次完整 Spec 列三项：P1 内层 service demand 未绑定原外层取消/source/绝对 deadline；P1 warm Context 只检查逻辑 live 而未检查确切 owner，可能在同一次 poll 借用已替换资源；P2 `development_projects` 两条旧测试仍假设 eager install/reopen。三项生产/测试源码已修复并进入随后增量审查；服务外层取消的真实 Red 保留，后续矩阵 case8/10/11 和开发 case17/18 已给出实际 Green。审查意见、源码修复与行为结果分别记录。

随后不可变增量保存为 snapshot `3442026890ab91bfeeba25a0cb86dee30af7baab` / tree `9e3dd04773ecccb4c4f0bcd7cbbf9bd74d3a2709`，相对前一 `aacb39d`，记录 `../target01/07-review-snapshot-postfix.json`；真实分支 HEAD 仍是基线。Standards 增量审查八文件通过。该次 Spec 保留 P1：`context.native_waits` 未观测 demand 祖先的取消及自然 deadline，嵌套原生需求可能在祖先意图失效后继续。随后已完成上文独立 `demand-relay` Guest 的真实 Red→Green，不再保留“嵌套 Red/Green 待执行”的旧状态；没有借阶段编译或单层取消测试代替该边界。

嵌套守卫复核输入保存为 snapshot `9050d39e91548b443a9a98f10d5f75db345070a2` / tree `e9b8288b2416c8a5f4f9154dfe71845472f534d3`，相对 `3442026` 十三文件增量，记录 `../target01/07-review-snapshot-nested.json`；不是实际分支的新交付 commit。随后 scope/TOML 夹具点保存为 `b9ed2e77f7f5ed6e763ca6e5a9882f70a17ff2ae` / tree `c3f58fd40dfa209ef912618dca119b27e8521a66`，记录 `07-review-snapshot-gate-fixtures.json`。这两个固定点的 Standards/Spec 已完成并均无剩余问题；Standards 按十二类 smell 核对，无值得报告的坏味道。

四个测试/support 文件的增量固定为 `b9ed2e7` → snapshot `b5e366a89e91a05bafb18da7f83c4b32276c72eb` / tree `d3d1f1617b86cd2510c09763de42f46d7b0f2d8f`，`../target01/07-review-snapshot-demand-adapters.json`。范围为 `language_configuration.rs`、`managed_dependencies.rs`、`optional_language_dependencies.rs` 和共享 `support/demand.rs`。该点 Standards 无问题，Spec 指出 P2：managed 的 ignored Hook 用例仍未消费 Demand；不得用其它八项通过覆盖这一项。

四次真实 LSP demand 修正后的增量 `b5e366a` → snapshot `40e0ce41f3ade837f0f98c50710130163670a301` / tree `aedff010af878bc8b90bd5fea16a3127e76403f0`，记录 `../target01/07-review-snapshot-ignored-hook.json`。两轴源码审查均无剩余问题；Standards 完整十二类 smell 无值得报告项。Hook 后续实际 Green 已记录于十九项矩阵节，源码审查与实际运行分别保留。

应用原生 Input 夹具增量 `40e0ce4` → snapshot `fc5092d4da7321c649235e7d0332e2c07310ba47` / tree `e206488ba36316a7b681a79daee965c0724f45d7`，记录 `../target01/07-review-snapshot-app-input.json`；随后公开 TimedOut 判据修正增量 `fc5092d` → snapshot `13f89e34073e679e53c9fdebd04f3d5d93f5bfa5` / tree `c30944b5a41e3a22938194a2816eeb6900769653`，记录 `07-review-snapshot-sdk-predicate.json`。这两个点的 Standards/Spec 均无剩余问题；应用第三项已实际 Green，T32 在第二点的完整运行仍失败，不能以两轴源码通过替代行为通过。

T32 等待阶段诊断夹具保存 snapshot `12897863a55f691db086162c2b1a2dd24eebe720` / tree `04e76eb1385439d8c3ed73bef7b7e740439b1637`，相对前一审查点 `13f89e3`，`../target01/07-review-snapshot-sdk-phase.json`；仅涉及测试失败观察，两轴增量审查已完成且无剩余问题。该点完整 T32 的真实第三次 Red 和确切失败阶段仍保留于上文。

应用第四项 cold 菜单夹具修正固定为 snapshot `c810b6efe71e4884e8b2d8815bf603b4f5e3c4c0` / tree `3b88e22bf586d2639a3b7535a470744e1d64bbc1`，相对 `1289786`，记录 `../target01/07-review-snapshot-app-cold-menu.json`；两轴无剩余源码问题。随后该项已取得 `79.98 秒 / 1/0/0` 的真实 Green，原审查与行为记录分别保留。

SDK 请求领域输入及额外 Query 窄用例固定为 snapshot `95bfa8a0f3638cb33260200d7fedd1c595362a9e` / tree `f5b5179e60a69ac332fc944bd6bc66f43e09d40c`，相对 `c810b6e`，`../target01/07-review-snapshot-sdk-domain-input.json`。Spec 无剩余问题，Standards 无硬性违例，完整十二类 smell 无值得报告项。审查员查看源码时 probe 行为尚待；随后取得的实际 `51.25 秒 / 1/0/0` 独立记录于上文，既不修改审查原报告，也不把它替代完整 T32。真实分支仍为 `90ceab915d63c2e6b8cb260c6f28acfc45adc7fc`，尚未普通提交本单；后续行为及原生实际终态已另记；最终文档双轴及普通交付仍待执行。

SDK5 迁移阶段 cfg(test) 单文件修正保存为 snapshot `5eaf5604854d1cb0fc3d3d36592807d8874ecd15` / tree `522aaf12103a9aa744c6351889a21df248574e54`，相对 `95bfa8a`，`../target01/07-review-snapshot-sdk-migration.json`。该增量两轴已返回：Standards 硬性违例 0、完整十二类 smell 无值得报告项；Spec 遗漏/范围蔓延/错误实现 `0/0/0`。两轴实际记录见[独立双轴记录](07-activation-and-compatibility-review.md)，未由审查员自行运行测试；SDK5 的实际 Green 绑定上述 `F8FD99D7...F510` EXE 与 160.76 秒行为结果。最终原生结果已另记，最终文档双轴及普通交付仍待执行。

应用失败后的增量仍以无引用源码快照记录，不移动真实分支：

| 固定 snapshot / tree | 对应边界及独立复查状态 |
| --- | --- |
| `923971fc4c09bbae9ff9e1c0ce98191fce82dc2f` / `75c8a63b6f8789afc16a1c8929be910a2704c28c` | App5 仅真实删除选区的 cfg(test) 输入修正；Standards 硬性违例 0/十二类 smell 无新增项，Spec `0/0/0`；随后 actual 154.39 秒 Green。 |
| `317ef3d4cb970488880d21f132c818c599ffed8d` / `df0b90a77e7ce6849d8a34589fe24178de21e2cb` | App10 旧绘制式观察仍不足：Standards 0，Spec 保留 P2 错误实现 1，后续修正见 ee802982。 |
| `3a58404b6e36bd8c3b9183f8b748a3473e4f7acd` / `675ab1f69f60e5ca6c53ec7a1717d09152d51065` | App11 restart 后独立公开 View 需求及 App12 失败观察；两轴源码零问题，随后 App11 完整 actual 138.18 秒 Green，App12 原诊断 Red 保留。 |
| `cea81174cf8ce085e0967729ba08bfcb0dd59e32` / `f4297f62d9a6df007f68e9b6c4bf56179ab6d224` | 移除私有缓存字段诊断，继续读取公开 owner；两轴零问题，真实 FileView 诊断 Red 保留。 |
| `ee8029827a9fd32bb4145bab6513e28217148c9d` / `916a16a15d6ec1c9ddc66476b3c4d828b95e5f71` | App10 对真实完成通知与 model target/pending 的强观察；两轴零问题，原 P2 在源码层闭合，随后 model 私有挂载 qualified 实际 108.21 秒 Green，P2 行为闭合。 |
| `2515a09a0a4f2f7640d8b11d71b994571dcf379f` / `c0b72138cb5f78d217ce2216eaff8b01d7ab0533` | 通用所选冷 FileView 的回退绘制需求，复用现有 masked paint/可见性/generation/取消/source 机制；Spec `0/0/0`、Standards 硬性违例 0/十二类 smell 无新增项；随后 App12 actual 51.81 秒 Green。 |

相应 JSON 为 `../target01/07-review-snapshot-app-empty-input.json`、`07-review-snapshot-outline-transport.json`、`07-review-snapshot-app-restart-probe.json`、`07-review-snapshot-app-probe-private.json`、`07-review-snapshot-app-outline-notification.json`、`07-review-snapshot-file-view-cold.json`。详细独立报告仍分轴保存于[独立双轴记录](07-activation-and-compatibility-review.md)。App10 不扩展生产可见性，跨功能 GPUI 测试文件仍放 `src/tests/outline/lifetime.rs`，由 model 的测试模块挂载；完成观察保留真实 Subscription，不新增宿主测试 API。

行为验收及规定门禁已完成：Runtime `19/0/0`、额外 Query probe 独立 `1/0/0`、应用原十二项 `12/0/0`；最终格式和 Workspace check 均 exit 0，非 UI `260/0/246 ignored`。T35 是实际包 GPUI 短组合，C03 是本机 Windows 新增物理交互，两者分别有真实证据；历史失败、旧 SDK 输入及平台限制全部保留。最终 Host/SDK/源码和停止清理已核对，见[Windows 物理记录](native-07.md)。最后独立双轴基线 `90ceab9` → 冻结 `b3bf9fc5342dc0421947d83fdd3727b5a5bc511f`，tree `ff7d7a2d9649250c62e66be44f8457a2f742e435`；165文件，相对已审源码点 `2515a09` 源码delta0、仅七份文档收口，真实索引与分支未动。Standards硬规范0/十二类smell无可行动项，Spec遗漏/范围蔓延/错误实现 `0/0/0`；两轴实际读回最终日志、保留EXE、SDK及原生清理证据，并确认outline原P2的源码与108.21秒行为闭合。详细结论见[独立双轴记录](07-activation-and-compatibility-review.md)。普通源码提交、推送与#99关闭均已实际完成，详见下一节。

## 普通交付及全批次议题读回

普通提交 `8ee28952014a6533253fc83e17c3410d88a76e48` 包含固定审查范围165文件；`crates/`、`plugins/` 相对最后受测源码快照2515差异0。暂存前检查范围及产物排除，暂存后`git diff --cached --check`通过，工作树当时pending0，原用户工作区未暂存或修改。随后普通推送到`origin/codex/plugin-api-community`；2026-10-11T22:19:13.1020363Z `git ls-remote --heads`读回同一完整SHA。外部实际结果为`../target01/07-delivery-commit-result.json`、`07-delivery-push-result.json`。

GitHub connector更新#99返回403 `Resource not accessible by integration`。按既有工单授权改用本仓库现有Git认证/GitHub REST，凭据仅内存使用；更新完整正文、勾选八项验收、按用户新约定同步C04源码注释/零Markdown SDK位置规则，并设closed/completed。2026-10-11T22:20:41Z完成关闭，另一次GET于22:20:43.0730345Z读回相同正文及状态；`07-issue-closed-readback.json`保存非敏感议题结果。没有修改标签、指派、九条依赖或父议题。

2026-10-11T22:21:08Z再次通过连接器读取#92–#99：#93–#99全部closed/completed；#92仍OPEN，正文与执行前baseline逐字相同。此后仅同步本地工单、验收索引与发布映射的真实交付状态；源码/SDK/包输入未改变，不重复无关Rust构建或测试。
