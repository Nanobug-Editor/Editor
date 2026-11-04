# 连续安装插件的进度窗口与失败重试（2026-11-03）

## 问题与复现边界

- 用户报告第一项插件可以安装，第二项出现 `timeout: connect`，没有安装弹窗、按钮持续转圈；补充称下载前的权限确认也不出现，使用当前 GitHub Release。随后确认第一项已显示“插件安装完成”，关闭窗口后才安装第二项。
- 本机核对 Release `v0.1.0-internal.20261103` 的源码为 `5f6f58851db14815ef5cb585c6d335953318c0e6`。任务起点 `0618b7bf` 与该提交之间只有文档变更。
- 使用与发行产物相同的 exe（SHA-256 `814e7036359ba7b52d709ba52ced1e2305fadbf7a7c8d86d58894a3e524e724e`），显式传入独立 `--profile`，在 Windows 原生窗口依次安装 Markdown 0.18.8、TOML 0.2.5：两次权限确认均出现；第一次有下载进度和完成窗口，第二次确认后只有安装按钮转圈，没有进度窗口，随后成功安装。
- 因此已复现的是**第二次安装的进度反馈缺失**。本次在线操作未复现连接超时，也未复现权限确认缺失，不能声称已确定这两项现象的独立触发条件。当前网络探测成功不代表用户报错时网络正常。

## 根因与修复

1. 管理面板用单个 `installation_dialog_open` 布尔值记住进度窗口；只有本地导入入口重置它，市场安装直接调用 worker，第二次和重试均沿用已打开状态。
2. 市场下载失败时清空 `installation`，已经打开的进度窗口会回退成泛化的结束提示；足够快的失败甚至没有可供界面打开窗口的报告。
3. 安装报告新增递增的尝试编号，本地导入与市场安装共用。面板按尝试打开一次窗口；窗口只观察、取消对应尝试，保留的完成窗口在后续尝试开始后退出。
4. 下载失败保留包含具体错误的终态报告，清除操作忙碌状态和取消句柄，允许用户重新确认后重试。继续沿用现有 i18n 文本、目录授权、摘要校验和安装事务，不改变网络超时或首次权限确认规则。

## 验证

- 修复前先运行实际 ZIP、真实 HTTP 回环服务、后台安装 actor 和 GPUI 指针交互的回归：第二次窗口断言、下载失败保留错误断言均失败；第二次权限确认断言通过。
- 初次修复后，两项原始回归均通过。进一步覆盖完成窗口保留时的后续安装、取消第二次安装且保留第一项、再次确认并重试成功。
- `cargo test -p editor-app --bin editor-app -j4 extensions::worker::marketplace::tests:: -- --test-threads=1`：14 通过，8 个需要独立包或在线环境的测试仍 ignored，未计作通过。
- 同一 Cargo 测试产物中的 `second_local_import_shows_visible_consent`、`install_confirmation_global_option_is_unchecked_and_accepts_opt_in` 均通过；`queued_installation_progress_follows_the_selected_ui_locale` 使用 `--ignored --test-threads=1` 显式执行并通过。
- `cargo fmt --check`、`git diff --check`、`cargo build -p editor-app --bin editor-app -j4` 通过。
- 修复版 Windows 原生验收：使用全新的独立 profile，以同样顺序安装 Markdown 0.18.8、TOML 0.2.5，两次均出现权限确认、可取消的下载进度和“插件安装完成”，关闭后恢复可用的管理界面。测试 exe 为 dev 构建，SHA-256 `bd96b895821271d4d3ee11e9850d4f75afb6b768a83c709b74541857a074d0a1`；未改写已发布 Release。
- `cargo test --workspace --exclude editor-app -j4`：888 通过、0 失败、447 ignored；未执行的 ignored 测试不计入通过数量。`cargo check --workspace -j4` 通过；已有未使用代码、私有接口及链接警告保持原样。

安装交互验收后关闭修复版主窗口时，自动化窗口枚举与关闭相邻执行，stderr 出现 `accesskit_windows 0.34.0/src/util.rs:380`、`HRESULT(0x80070578)` 无效窗口句柄异常。位置和症状与[现有发行限制](../../distribution/verification/release-all-2026-11-03.md#未解决的无障碍并发关闭异常)相同，保留 `candidate.stderr.log`，本次不将自动化退出计为通过，也不据此扩大修改无障碍依赖。

本机证据集中在 `C:/Users/Tmiracle/.codex/task-artifacts/plugin-install-retry-20261103/`：`red-progress.log`、`green-progress.log`、`marketplace-tests-final.log`、`release-second-permission.png`、`release-second-missing-progress.png`、`release-second-installed.png`、`candidate-second-permission.png`、`candidate-second-progress.png`、`candidate-second-complete.png`。复现使用隔离 profile，不写入用户日常插件目录。
