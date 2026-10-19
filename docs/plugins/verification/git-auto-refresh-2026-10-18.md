# Git 周期检查打断审查回归

用户反馈：暂存与 Git 面板每隔几秒自动刷新，导致无法正常操作。本记录独立于已关闭的 #119/#120 和此前加载/工作窗口超时修复；已有验收记录与用户测试项目不改写。

后续用户明确要求允许自动刷新，但降低频率且改为静默刷新。独立 0.1.9 的新规则、真实变更时仍禁用的第二条缺陷及最终验证见 [低频静默刷新记录](git-silent-refresh-2026-10-18.md)；本页保留 0.1.8 当时的实际证据，不将旧结果冒充新规则验收。

## 重现与定位

通过真实独立 0.1.7 ZIP、公开 Manager、原生进程与 Timer，在私有仓库展开暂存与本地分支页。仓库没有任何变化，4.56 秒后暂存树 epoch 从 4 变为 5，已加载页面被撤销，回归 RED，日志 `target/git-refresh-stability-red.log`。

第一阶段仅让状态/操作变化刷新暂存与冲突树，让分支名/当前分支变化刷新分支树；提交详情文件保持自己的来源。树回归 GREEN（53.82 秒，包含 WASM 冷启动），但加入控件场景连续性断言后，4.16 秒仍出现 revision 41 → 42，RED，日志 `target/git-refresh-stability-scene-red.log`。因此无变化的读进程忙碌/完成也在打断界面。

原生同样确认第二条因果链：阶段一包在无输入闲置时，`manager.poll` 使 process 0 → 1、scene 59 → 60，14 个 disabled 字段变化，实例及树 handles 保持一致。草稿、原焦点、a-review 选择、src/Local 展开、11 px 滚动位置仍在，但当前提交按钮/暂存树/Git 操作准入全部为 false。原生回归 RED（58.86 秒），见 `target/git-refresh-native-scene-red-2026-10-18.log` 和同名目录下的 `dark-zh-CN-100/evidence.json`、`native.log`、`failure-workbench.png`；退出时 HWND 与 GPUI 窗口均为零。

首次原生支架只检查绘制状态，阶段一包通过（73.09 秒）；其 disabled 元数据为 null，不能据此证明控件可用。补入公开场景连续性及准入断言后才捕获上述 RED。旧日志原样保留在 `target/git-refresh-native-red-2026-10-18`，不计为失败或最终验收。

第二阶段标记可选的周期读取：无变化时不发布内部忙碌/探测场景；真实状态、分支、HEAD、完整历史或操作身份变化，以及错误，仍恢复正常发布。当前原生输入先取消其拥有的只读子进程和未完成查询，再执行该事件；不排队重放旧输入，不放宽 revision、index 或文档版本检查。只有尚未失败的操作探测可恢复上次完整观察的 readiness；已发布的来源保持有效。业务写进程不使用该取消入口。

增量审查发现并修正两个中断边界：部分 status/probe 失败即退出静默，保留完整错误观察，不因抢占解封旧建议；周期观察转入原生文档恢复读取时，也退出静默，保留该请求自己的版本、回执和忙碌状态。公开包回归用真实 Git 冲突和与宿主契约一致的 `ReadDocument → Completed(Err LimitExceeded)` 回执重现第二项：旧候选实际 Timer 已重读但界面仍停在 enabled scene 37，RED（51.05 秒）；最终包正常发布忙碌权威、错误后恢复可用，GREEN（73.27 秒），磁盘结果与 index 字节不变，退役资源为零。日志位于 `target/git-refresh-failure-runner/observation-reread-{red,green}.log`。同步探测错误不会被初始化标志覆盖；已退役进程通知也不重发场景，后者为防御性收敛，宿主本身已撤销旧 roots，未认定为已证实根因。

产品变更仅在独立 `C:/Projects/RustProjects/Git-for-Nanobug`；宿主新增公开包/管理器与原生回归测试，没有新增 Git 专属宿主 API。候选版本提升为 0.1.8，协议 7、SDK 0.2.0、能力与权限声明保持原基线。

## 最终验证

- 最终实际 ZIP 的 `workbench::refresh_stability::git_package_unchanged_timer_preserves_loaded_staging_and_branch_pages` 和 `workbench::scene_activity::git_package_history_activity_defers_automatic_refresh_until_quiet`，2/2 GREEN（83.18 秒），见 `target/git-auto-refresh-manager-final.log`。确认定时查询实际执行、无变化保持树页与场景、单次当前输入抢占可选读取、迟到输出不替换对话框；真实外部状态/分支变更仍使相应页失效，新提交自动显示，旧事件仍被拒绝，显式焦点刷新及时。
- 最终 `workbench::observation_failures::git_package_automatic_native_reread_publishes_busy_authority`，GREEN（73.27 秒）；上述原生恢复错误并未被静默隐藏。
- 最终原生 `extensions::worker::native_acceptance::idle_retention::git_native_idle_checks_preserve_review_state`，1/1 GREEN（91.99 秒），见 `target/git-auto-refresh-native-green-2026-10-18.log`。暗色、简体中文、100%：草稿闲置 8002 ms／268 帧，scene 保持 59；单次 D 替换原选区成功，菜单闲置 8017 ms／271 帧，scene 保持 61。原焦点、树展开/选择、11 px 滚动锚点、菜单焦点与准入保持；单次 Escape 正常关闭菜单。四张 D3D11 实际截图均已逐张查看；所有 Actor、GPUI 窗口与 HWND 正常退役为零，无 TLS 错误。正式回执 `target/git-auto-refresh-native-green-2026-10-18/dark-zh-CN-100/evidence.json` 的 SHA-256 为 `31c32e7a150bfc9ef0ddaaf06d9367c42b7197b4b031aef3111c9a88cc15b0d2`，明确标记 `idle-refresh-state-only`，不冒充完整工作台流程。
- 访客测试 101/101 通过，见 `target/git-auto-refresh-guest-tests.log`。访客 `cargo fmt --check` 及任务源码快照格式检查通过，后者日志 `target/git-auto-refresh-canonical-fmt.log`。
- 任务源码快照 `cargo test --workspace --exclude editor-app`：77 组、278 passed、0 failed、302 ignored，见 `target/git-auto-refresh-nonui-workspace.log`；三个上述真实包测试与原生回归均已单独显式执行，未把 ignored 当作通过。
- `cargo check --workspace` exit 0，见 `target/git-auto-refresh-workspace-check.log`。根工作区 `cargo fmt --check` 仍仅在既有 `extensions/preview/toolbar.rs:65`、`extensions/surface.rs:86` 两处换行失败，见 `target/git-refresh-root-fmt.log`；未改动这两处其他任务的代码。
- 本次源增量审查的两个 P2 已解决，无新增未解决项。部分失败与输入精确交错没有单独强制端到端 RED；该边界依据源审查及既有 lifecycle 覆盖，不虚报独立实测。

## 交付身份与重现

本地独立插件包：`C:/Projects/RustProjects/Editor/target/git-auto-refresh-fix-2026-10-18/git-0.1.8.zip`，803,903 bytes，SHA-256 `5c6c483fd60b9fc02277aadfe800408fbc324eab1b99ffac7e1ddf0acb9b3292`。插件自身 `dist/0.1.8/` 保留相同 ZIP、53 文件源码归档与逐文件账本；源码归档 SHA-256 为 `cd06d05f86da11473b66b8cae2acd8f96d016d97f5d245ff75d39da786c41188`。53 个当前文件与归档均逐项核验匹配；Cargo.lock 仅更新插件自身版本，依赖版本没有变化。

包只有 `git.wasm`（2,459,465 bytes，SHA-256 `d8251ed6af80460219edcc024416da1ef28a3403c8c20233e97b14d71d18c87a`）、`manifest.json` 和原生 rebase helper。清单相较 0.1.7 仅包版本改变；helper 仍是 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。无 SDK 或上游项目副本。

构建仍直接调用前一轮宿主 `target/git-load-fix-release-2026-10-18/Nanobug.exe --plugin-cargo` 和 `--plugin-package`。安装说明和结构化结果见新包同目录的 `使用说明.txt`、`verification.json`；通过插件管理器安装 ZIP，使用已交付的通用树分页修复宿主。

```powershell
$env:NANOBUG_GIT_CANDIDATE = 'C:/Projects/RustProjects/Editor/target/git-auto-refresh-fix-2026-10-18/git-0.1.8.zip'
& ./target/git-refresh-failure-runner/git_plugin.exe --ignored --nocapture --test-threads=2 workbench::refresh_stability workbench::scene_activity
& ./target/git-refresh-failure-runner/git_plugin.exe --ignored --exact workbench::observation_failures::git_package_automatic_native_reread_publishes_busy_authority --nocapture
$env:NANOBUG_GIT_NATIVE_IDLE_RETENTION = '1'
$env:NANOBUG_GIT_NATIVE_OUTPUT = 'C:/Projects/RustProjects/Editor/target/git-auto-refresh-native-recheck'
& ./target/git-refresh-native-final-runner/editor_app.exe --ignored --exact extensions::worker::native_acceptance::idle_retention::git_native_idle_checks_preserve_review_state --nocapture
```

运行原生用例前清除其他 `NANOBUG_GIT_NATIVE_HISTORY_*`、`NANOBUG_GIT_NATIVE_SCENE_PROBE` 等诊断开关；保留父用例管理独立 child 及关闭流程。冻结的管理器 EXE SHA-256 为 `4d4f4151397ad3d8638025bcd0c41d72086f8eaab5a4c36f660dc5db1a582af4`，原生 EXE 为 `67e5350997ac7085bc5fd029af97528038b28621d77c59bc148212fdcf37b1dc`，均保留匹配 PDB。

## 限制与现场

未重跑既有八种主题/语言/缩放组合，也未验证系统 IME 候选弹窗。本次验证覆盖定时检查打断交互及已完成错误回执，不等于修复所有卡死：审查另见既存非 `write-check` 原生请求 `Cancelled` 清理缺口（独立插件 `src/events.rs:164`，宿主 `crates/plugin-runtime/src/request_state.rs` 的请求 deadline 会发 `Cancelled(TimedOut)`）；该独立超时路径未纳入此次修复或宣称通过。

所有重现使用私有配置和仓库；未修改用户 `Nanobug-Git-Lab` 的分支、暂存区、合并或变基现场，也未替换运行中的宿主或静默安装候选。

返回[插件文档目录](../README.md)。
