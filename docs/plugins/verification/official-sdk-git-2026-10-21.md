# Git ResourceGroup 接入与验收

日期：2026-10-21。状态：A1 的实现、Windows 行为、生命周期及热更新验收通过；Git 独立插件从 0.1.23 提升至 0.1.24。未提交、推送、发布或安装到用户日常宿主。

依据：[SDK 接入方案](../specs/official-plugin-sdk-adoption-2026-10-21.md)、[开发工具契约](../specs/plugin-developer-tooling.md)。本会话仅修改 `C:/Projects/RustProjects/Git-for-Nanobug`、必要的 Git 宿主测试与本记录；不修改共享 SDK、TypeScript、Markdown 或文档总索引。

## 来源与产物

Git 目录开工及结束时均没有 `.git`，没有可报告的 HEAD。未在该插件目录执行 `git init`；比较基线是 `target/sdk-adoption-git-2026-10-21/baseline/` 的开工前副本。独立原生验收仓库的初始化仅发生在该任务的 `target/.../native-workspace/`，不涉及插件目录或用户仓库。

证据目录均相对主仓：`target/sdk-adoption-git-2026-10-21/`。

| 输入 | SHA-256 / 身份 |
| --- | --- |
| 固定宿主 `target/plugin-developer-tooling/debug/editor-app.exe` | `c82565746b49011761884604da17e1bcbd30a45cb972b42a2a6ba20fda940856` |
| 固定宿主内嵌 SDK | `8e2f359fedf819d8e8b6b3da1012722629e12d9731c02c38e5b01ccab8bd02e4` |
| 冻结旧 ZIP `target/branch-tree-gestures-2026-10-21/packages/git-0.1.23.zip` | `7d0ea99e484afafe343902a1cc30baaebbb7a618ae1c4ecee4144579727fe5bd`；未覆盖 |
| 新 ZIP `packages/git-0.1.24.zip` | `2afa18799a00a1862937af54475908a61df15252ebdec6e10c0d1cd513afb946` |
| Git 最终源码逐文件账本 | `guest-source-final.json`；路径与逐文件 SHA-256 按序拼接后的摘要 `bfa93ba514cf9e5ad5cb4f452fdd7425ab2ccfee08a9c982af0498e551e56349` |
| 最终集成测试程序 `frozen-tests/git-plugin-final.exe` | `e2327a3ecc3245af33227aff86c2ca9d3cf6374bd7ab7a1ca45df08730aaf3b1`；与固定宿主分开记录 |
| 原生测试程序 `host-target/debug/deps/editor_app-646c88d348da6726.exe` | `c164a7969e91fd3433ef4871882a2e274cac7c79a29658de1eb2925172058d78`；共享源码测试构建，未替换固定宿主 |
| 故障衍生 ZIP `fault-packages/git-0.1.24-trap.zip` | `0468ebe85d5ed0c250ebda74ba0777c8efa492e27093db5c9112f5c4c4e5dfbc`；不可作为发行包 |

独立消费者通过固定宿主 `--plugin-cargo` / `--plugin-package` 构建，实际解析到宿主受管 SDK 缓存；没有仓库 `crates/` 路径依赖或 SDK 副本。工具链为 rustc 1.98.1、cargo 1.98.1，目标为已安装的 wasm32-wasip2 / x86_64-pc-windows-msvc，没有安装工具链。Cargo 包、清单、锁文件及双语说明同步为 0.1.24；`nanobug-plugin.json` 的 `version: 1` 仍是项目描述格式版本，不是包版本。原生 rebase helper 未改动，保持其独立 0.1.0。包协议仍为 7，能力要求和权限不变。`package-inventory.json` 核对包内共 9 项，仅清单、WASM、6 个 SVG 和原生 helper。

访客测试输出为插件的 `target/sdk-adoption-git-2026-10-21-tests/`。主仓集成测试/检查输出为本证据目录的 `host-target/`，`host-metadata.json` 已核对 workspace_root 和 target_directory。主仓仍含大量并行改动；集成测试来自共享源码，不能冒充固定 EXE/SDK 的构建。`host-source-observation.json` 记录共享源码观察时的 SDK 摘要及主仓 HEAD，明确不充当构建时冻结快照。固定宿主证据来自真实 CLI/原生会话和静态检查，分别列出。

## 实现

- `src/lifecycle.rs` 在现有 State 上管理 ResourceGroup。timer 与 documents subscription 由 group 唯一拥有，额外保存的完整句柄仅供通知匹配。
- 30 秒 timer 成功而订阅创建失败时，显式关闭此前资源，返回原始 Failure；清理失败附带独立说明，失败资源仍归 group。显式重试每次最多一次清理，不在通知中自动重订阅或无限重试。
- `Prepare` 在覆盖 State 前检查清理结果；失败则保留旧 State/owner。重复 `Activate` 复用已有观察资源、数据根和树，不重复分配。
- 每个事件先 `observe`；外来 timer/document/subscription 通知在抢占、输入计时、查询推进之前被过滤。匹配的 `SubscriptionFailed` 撤销身份并显示故障，不自动重新订阅。
- 隐藏面板继续保留文档订阅，关闭比较源时释放其引用。Git 进程、写队列、审查确认、树和临时文档继续由原有模块管理，不放进 ResourceGroup。30 秒间隔、用户活动退让、保存/手动刷新优先保持。
- 初始化错误向 Manager 返回失败，避免将错误界面提交为激活成功；普通业务错误继续使用现有界面反馈。没有 Deactivate 回调，Drop 不调用宿主，最终异常回收由实例退役处理。

## 已执行检查

| 检查 | 实际结果 / 证据 |
| --- | --- |
| 插件 `cargo fmt --check` | 通过；`guest-fmt.log` |
| 固定宿主 `--plugin-cargo Cargo.toml test --lib --offline` | 136 通过、0 ignored；`guest-tests-final.log` |
| 固定宿主 `--plugin-cargo Cargo.toml check --target wasm32-wasip2 --offline` | 通过；`guest-check-final.log` |
| 固定宿主 `--plugin-package . --output .../packages` | 通过；`package-final.log` |
| 固定宿主 `--plugin-check .../git-0.1.24.zip --format json` | 通过，`executed=false`；`plugin-check-final.json`。仅准入结论 |
| 新增真实 WASM / Manager 回归 | 最终包 `sdk-resources-delivery.log` 6 通过、0 ignored，436.79 秒；初始化失败、双资源部分清理、重置/重试、重复激活/显隐、trap、更新及两类失败候选全部执行 |
| 30 秒真实 timer / 当前审查输入 | 最终 `quiet-delivery.log` 1 通过、0 ignored；65 秒持续边界活动保持场景，静默后观察到外部提交并拒绝 stale key，显式 Focus 及时刷新；整个进程 169.61 秒 |
| 主仓 `cargo check --workspace --offline --jobs 2` | 通过，存在现有警告；`workspace-check.log` |
| 主仓 `cargo fmt --check` | 最终通过，`workspace-fmt-final.log`。首轮的并行 `analysis_guard.rs` 缺失及范围外格式差异保留在 `workspace-fmt.log`；本切片未批量改写他人文件 |
| 主仓 `cargo test --workspace --exclude editor-app --offline --jobs 2` | 最终通过：454 passed、0 failed、350 ignored，`workspace-test-final.log` 与 `workspace-test-summary.json`。ignored 不计通过；本切片的 6 项包回归及 65 秒活动测试已另行显式执行。首轮的并行 ResourceHandle.scope 缺失记录在 `workspace-test.log` |
| 新增 GPUI 原生自动回归 | 最终 1 passed、0 ignored，118.07 秒，`native-test-verification.log`。此前三次编译受并行模块错误阻断，日志 `native-test-build.log`、`native-test-final.log`、`native-test-delivery.log` 保留；对应位置由其会话修正，本切片没有修改它们 |

真实 WASM 失败注入测试通过独立 WIT host 边界执行包中原组件，不模拟 ResourceGroup。它观察实际接受/关闭的 timer，断言主错误码、清理失败信息、重置失败不丢 owner、清理未完成不接受第二个 timer，以及之后显式重试恢复分配。另一用例先成功接受 timer 和订阅，再拒绝数据根初始化；订阅释放失败仍释放 timer，之后重置只重试该失败订阅，重复重置不分配资源。普通场景经公开 Package/Manager 验证实际资源快照、重复激活、显隐、升级、失败候选和受管数据。错误恢复覆盖可静态准入且可加载、但没有协商 `editor.documents` 的候选，不能用损坏 WASM 的准备失败替代 Activate 失败验证。

trap 验收使用单独的 `fault-project/` 和 `fault-packages/git-0.1.24-trap.zip`。这是从本轮独立源码复制后，仅在 Focus 通知注入 panic 的测试衍生包，不是交付 ZIP。由固定宿主及同一受管 SDK 构建，输出根另记于 `fault-metadata.json`，没有向正式插件添加故障命令、向宿主添加专用 API。断言检查 trap 后、显式 disable 前的旧 incarnation 零资源，然后再验证 disable 幂等。

## 固定宿主 Windows 原生验收

使用专用 `native-profile/`、`clean-profile/` 和 `native-workspace/`。用户日常 Release PID `45612` 未被操作或重启。交互验收的隔离 GUI PID `48856`，启动时间 `2026-10-21 22:54:36 +08:00`，窗口 ID `15469644`；控制器 PID `65784`。

在 `acceptance.txt` 原生编辑区输入 `未保存的 Git SDK 验收 · A1`，文档标签显示未保存标记。Git 暂存/历史面板显示实际临时仓库。一次资源包摘要变化的热重载、一次故意协议 999 候选拒绝、以及最终源码的成功重载期间，GUI PID、文档和未保存文本保持；真实 Ctrl+Z / Ctrl+Y 仍撤销及恢复该输入。验收结束后才显式 Ctrl+S 保存此临时文档，再发送 stdin `stop`。所有临时协议/图标标记已还原，六个 SVG 与开工基线逐字节一致。

`development-report-with-rejection.json` 的三次已接受 generation 为 0/1/2；故意失败的候选保留 `admission=failed`，最终 `status=failed`、退出码 1 是正确的负例结果。其 `activation=passed`、`cleanup=passed`、`termination=normal`；没有将失败会话改写为成功。目录候选 digest 与 ZIP 字节 SHA-256 本就不同，按报告中的身份区分。

另起正常独立会话，`clean-development-report.json` 中 admission/startup/activation/cleanup/status 全为 passed，正常 stdin stop，退出码 0。补充双资源失败清理后，最终源码另在 `final-profile/` 执行一遍：`final-development-report.json` 同样全部 passed、正常退出 0，目录包 digest 为 `3128d8099551f6a718105c8cb6b2e563f6b49034ad4b9a5881948b43129abfef`。`business=not_exercised` 是这些报告的固定业务覆盖标记；这里只据此报告生命周期通过。原生编辑、面板和 Undo/Redo 的人工交互证据在本节独立陈述，不把报告改称全面业务认证。该手动热重载场景发生在追加数据/树失败清理之前的本轮候选，最终 ZIP 的生命周期与错误路径另由上述报告及真实包回归验证。

最终 ZIP 另经 GPUI 自动回归进入公开 Manager 和原生 EditorApp 发布入口：先保留未保存中文输入，再安装冻结旧包、更新最终新包、拒绝损坏组件候选及停用。每阶段均断言进程 PID、EditorState entity 和未保存文本保持，原生 Ctrl+Z 改变原事务、Ctrl+Y 恢复原文本，磁盘文件不被隐式保存；前三阶段还检查 Git 提交表单实际渲染。该用例为 `extensions::developer_tooling_tests::git::git_sdk_hot_update_preserves_native_document_and_undo_redo`，实际显式 `--ignored` 执行通过；不是只检查文本副本或编译结果。

从主仓复现该用例（路径中的旧 ZIP 与最终 ZIP 须先满足上表摘要）：

```powershell
$env:CARGO_TARGET_DIR = 'C:/Projects/RustProjects/Editor/target/sdk-adoption-git-2026-10-21/host-target'
$env:NANOBUG_GIT_BASELINE = 'C:/Projects/RustProjects/Editor/target/branch-tree-gestures-2026-10-21/packages/git-0.1.23.zip'
$env:NANOBUG_GIT_CANDIDATE = 'C:/Projects/RustProjects/Editor/target/sdk-adoption-git-2026-10-21/packages/git-0.1.24.zip'
cargo test -p editor-app git_sdk_hot_update_preserves_native_document_and_undo_redo --offline --jobs 2 -- --ignored --nocapture --test-threads=1
```

## 审查与限制

使用 code-review 的 Standards/Spec 两轴，对开工快照与本次实际文件分别只读审查。Standards 初审及增量复核均为 0 硬违规、0 判断性问题。Spec 初审发现“初始化错误被成功回执掩盖”和“无关通知在过滤前取消后台查询”两项；已修正并补回归。复核提出 trap 断言应先于 disable，亦已调整。审查不包含提交历史，未执行提交、推送或关闭 issue。

最终 Spec 静态复核为 0 项剩余发现；其结论不将仍在执行的测试视为通过。`trap-final.log` 单独重跑了停用前资源清零的断言，1 通过、0 ignored。

Standards 最后复核提出两项 P3 注释失真：WIT host 新增接受订阅分支后仍写“拒绝订阅”，活动测试仍残留 End 滚动说明；已就地更正，没有改变行为。

计时回归原来交替 Home/End，End 会选择另一个提交并合法启动其预览查询，却被零进程断言误认为自动刷新。冻结旧 ZIP 与新包均在同一断言失败，分别见 `quiet-baseline-control.log` 和 `quiet-native-final.log`。现在持续发送边界 Home，保留 65 秒真实活动、两个 30 秒 tick、无 Git 查询及场景稳定断言；外部新提交插入后先完成一次 Home 的合法预览，再用第二次边界 Home 验证 Focus 立即刷新。该修正通过规格轴复核，不修改插件业务或删除计时断言。

本轮只验证 Windows，未验证 macOS/Linux。主仓格式、编译、非 editor-app workspace 测试和本切片 GPUI 用例均已通过；workspace 的 350 个 ignored 仍未作为全套业务通过。主仓源码持续有并行改动，测试程序与固定宿主身份分开记录，不把共享构建称作冻结发行产物。本记录只关闭 A1 验收范围，不代表 TypeScript、Markdown、市场或完整插件平台的工作完成。
