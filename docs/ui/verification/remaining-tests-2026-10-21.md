# 剩余 14 项宿主测试排查与修复

日期：2026-10-21。范围：接续 [editor-ui 抽取验证](editor-ui-extraction-2026-10-21.md)中已在迁移前后复现的 14 项失败。保留 GPUI Kit 正式入口，不 fork、不升级依赖，不提交或推送。

## 复现与判断依据

先使用上一阶段冻结的 `target/component-lock-validation-2026-10-21/editor-ui-host-tests.exe`，对 14 个测试逐个执行 `<完整名称> --exact --test-threads=1`；全部复现原断言失败，单项耗时 0.04–0.34 秒。原始失败记录在 `target/remaining-tests-baseline-2026-10-21.log`。

判断以[插件平台契约](../../plugins/specs/plugin-api-platform.md)、[按需激活规则](../../plugins/specs/plugin-api-community-foundation.md#9-状态配置与生命周期)、[插件管理器操作规则](../../plugins/specs/plugin-manager-shared-layout.md#2026-10-14-管理呈现补充)及[宿主消息规格](../specs/host-messages.md)为准，没有把当前实现自动当作需求。

| 失败范围 | 数量 | 分类与修复 |
| --- | --- | --- |
| `declared_language_service` 的调用方：状态弹窗、旧 Rust 回调、三项定义跳转 | 5 | 夹具沿用“安装即准备语言服务”的旧前提。普通回调夹具先验证安装保持冷状态，再通过公开 Manager demand 取得真实计划；状态弹窗测试从真实识别文档产生请求，复用既有 worker 测试传输取得回执，在同一发布帧绑定懒适配器，保留握手完成与旧回调拒绝断言。 |
| `status_indicators_open_plugin_lists` | 1 | 测试注入了没有文档需求的虚构语言状态，下一帧会正常撤销。该测试的目标是通用 loading 弹窗，改用待恢复插件状态；原有点击、外部关闭及锚定布局断言继续执行。 |
| 插件日志隔离的两项消息测试及 Run/Build/Debug 拒绝消息测试 | 3 | 过时的默认展开前提。现行规格要求默认关闭，先断言关闭，再通过真实消息按钮打开并检查历史；继续验证插件日志归属、红点、清空及单次拒绝不重复记录。 |
| 旧布局与重新打开后的会话显隐 | 2 | Windows 测试夹具使用了另一份会话身份。Workspace 将目录规范化为扩展路径，SessionState 按该路径计算文件名；原测试用临时目录原始路径读写。统一使用 Workspace 返回的根目录，保留隐藏、旧布局同侧面板、宽度与重新打开后的持久化断言。 |
| `native_input_and_canvas_route_text_to_their_own_nodes_and_hide_reclaims_targets` | 1 | 过时的 `native_ui.is_none()` 内部结构断言。隐藏需要保留键控状态并撤销输入，更新为检查原实体保留、场景失活，以及隐藏后实际中文输入和按键不再向插件发布文本或键事件。 |
| `incompatible_plugin_details_keep_preferences_and_update_uninstall_actions` | 1 | 管理器已按后续规格隐藏不可用操作，旧测试仍寻找并点击禁用控件。改为断言启停入口隐藏；偏好不变、无启停工作项、更新与卸载入口和实际更新请求继续验证。 |
| `operation_errors_retain_plugin_ownership_through_worker_publication` | 1 | 旧英文错误文案断言。改为比较公开 `StaleRevision` Failure 与现行翻译，继续验证错误归属、级别、来源、日志正文，以及包检查错误不伪造插件归属。 |

这些失败的原因位于测试前提、夹具或断言；本轮没有据此改写产品的按需激活、面板显隐、输入授权或插件管理行为。隐藏保留状态的依据还包括[插件 UI 状态契约](../../plugins/specs/plugin-ui-decoupling.md)与既有原生表单、树和手势回归。

语言状态测试的首次调整还暴露了测试传输的帧边界问题：先发布计划再注入适配器，会正常启动另一适配器并尝试对测试程序执行 LSP 握手。定向探针记录到 `language server response omitted Content-Length`、适配器失活及归属错误日志。修复在既有 `extensions/lsp_tests.rs` 内增加通用的测试发布回调，真实需求及计划仍由 Manager 受理；不公开 worker 私有字段，不增加生产测试接口。临时探针在完成定位后移除。

## 验证

本轮验证使用当前工作区；metadata 的 workspace 为 `C:\Projects\RustProjects\Editor`，target 为其 `target/`。共享工作区已有改动和期间其他任务的改动不归为本轮成果。

| 检查 | 实际结果 | 本地日志 |
| --- | --- | --- |
| 原 14 项逐项 `--exact --test-threads=1` | 14 passed、0 failed，保留原测试名称 | `target/remaining-tests-targeted-final-2026-10-21.log` |
| `cargo fmt --check` | exit0 | `target/remaining-tests-format-2026-10-21.log` |
| `cargo test --offline --locked --workspace --exclude editor-app --jobs 2` | 84 组，421 passed、0 failed、335 ignored；其中 editor-ui 为 85 passed、0 failed、10 ignored | `target/remaining-tests-workspace-2026-10-21.log` |
| `cargo check --offline --locked --workspace` | exit0 | `target/remaining-tests-check-2026-10-21.log` |
| 宿主完整程序 `--test-threads=1` | 660 passed、0 failed、301 ignored，耗时 224.49 秒；包括原 14 项及依赖/锁文件/统一入口守卫 | `target/remaining-tests-app-full-final-2026-10-21.log` |
| Windows 原生输入及退出 | 显式运行 ignored 的 `native_initialized_windows_exit_without_tls_failure`，1 passed；内部完成 KitRoot 与 NativeEditor 两个真实 Windows/D3D11 子进程 | `target/remaining-tests-native-input-exit-final-2026-10-21.log` |
| 锁文件、依赖树与第三方版本 | 全功能、全目标、normal/build/dev 树中没有组件及宏库；锁文件相对 HEAD 没有新增第三方包或版本 | `target/remaining-tests-dependency-tree-2026-10-21.log`、`Cargo.lock` |
| `git diff --check` | exit0 | `target/remaining-tests-diff-check-2026-10-21.log` |

`cargo test --offline --locked -p editor-app --bin editor-app --no-run` 已成功。宿主全量、最终逐项复测及原生切片使用 `target/remaining-tests-2026-10-21/editor-app-tests-final.exe` 与匹配 PDB，SHA-256 为 `51ad5c9ac0c3bdae7e66d298f342fc2bb9f78424e944e007229d71e1e518bc62`。构建与测试均来自同一工作区；只复制产物以避开 Windows 正在运行文件的重链接锁。较早的候选全量运行在取得最新构建后主动停止，没有计入通过结果。

首次构建遇到并发变更中的滚动路由、项目窗口和安装接口编译错误，随后重复同一入口已成功；未回退这些变更或扩大内部可见性绕过模块边界。共享输出目录还出现过构建锁等待，以及其他并发链接造成的 Windows `os error 1450` 资源不足；资源释放后重试成功。链接沿用本机已有 MSVC/Windows SDK，仅在验证进程中设置其 `LIB` 路径，没有安装工具或修改全局环境。保留既有 unused、private_interfaces 和 Wasmtime 链接警告。

未执行的 ignored 用例不计通过。本轮没有重建无关 WASM 包，没有交叉验证 macOS/Linux，也未手工检查 Windows 输入法候选窗口。没有提交、推送或发布。

返回 [UI 文档目录](../README.md)。
