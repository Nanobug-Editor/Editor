# Git 插件实施验证（2026-10-15）

状态：2026-10-18 最终独立 0.1.7 已完成行为验收，等待普通提交、推送与 #120 关闭读回。#119 已读回 closed/completed；#120 当前仍 open。Git 保持外部独立项目、显式安装 ZIP 和公开 SDK，无随附或内置注册。本版业务覆盖按原全量有效 43 项加修正视口夹具的 1 项分别验证；原全量的一项夹具失败与真实退出码保留。#119 最终交付与历史结果保留，上一轮[暂停现场](git-plugin-paused-2026-10-16.md)不再表示当前执行状态。依据为[已确认规格](../specs/git-plugin.md)。

## 2026-10-18：0.1.7 最终独立候选（行为验收完成）

安装包为 `target/git-plugin-final-120-scene-quiet/git-0.1.7.zip`，801674 字节，SHA-256 `8034109e64266b12e17d70fcb7c513de81d1b2130b690baa117ae44450135d71`；WASM `0a784718f207f4f9fa51faa8f57aaaa4534adac3655cd7aee530e507ac477799`。协议 7、公开 SDK 0.2.0 和 helper 0.1.0／`73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a` 保持。对应 52 个真实构建源文件归档为 `git-0.1.7-source.zip`，SHA-256 `cd6973de71c069459ac73eee7369772d23ef8b56f2fca90ab45b4fc8b71a45c7`；逐文件摘要账本自身为 `acb473c2f4820a6f46ca2b929c72794eedea48ada546d0ea247451a4afb5eadd`。独立交付目录 `C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.7/` 同时保留安装 ZIP、源码 ZIP、文件清单和校验值；源码 ZIP 不含 dist／target／SDK／.git。Root 再次逐项核对 52 个源码归档条目、活跃构建源及真实 helper 摘要（`target/git120-final-017-dist-verified-2026-10-18.json`）。

本版仅在独立访客修复发布策略：隐藏／忙碌／冷却 Timer 及成功可见性回执不替换无变化场景；周期本地检查等待最近明确交互和业务完成后的完整空闲间隔，忙碌期间的保存／激活／手动显式请求合并保留并及时恢复。未放宽 revision／source／patch／资源校验，未重放导航或业务输入，未加入 Git 专属宿主分支。六项静置回归由 4 RED／2 控制通过到 6/6 GREEN（`target/git120-scene-receipts-2026-10-18/quiet-{red,green}.log`）。

最终访客 101/101、0 ignored、5.17 秒通过，公开 CLI 打包 7.62 秒，格式检查通过；Root 在私有 global／system／template／hooks 配置下再次验证 101/101、0 ignored、6.07 秒（`target/git120-final-017-guest-private-tests-2026-10-18.log`）。完整 Guest 固定范围 `6b8433ed8cc04d9179c9a74204943facb10de883...44a2eaefc2b0a14eef06b49e9e76f22f913ccb80`／tree `c32c32430e4e321acc6df25465f7e65aeb06c2ef`，12 个 QA 提交、31 文件，3989 增加／378 删除；Standards 硬性 0／判断性 0、全部 12 baseline，Spec a0／b0／c0。该私有 QA 固定点只用于审查，独立项目没有新增 Git 元数据或远程仓库。

宿主格式、workspace check 通过，后者 3.98 秒、80 条既有警告；非 UI workspace 77 组／278 passed／299 ignored／0 failed（`target/git120-qa14-{fmt,workspace-check,workspace-tests}-2026-10-18.log`）。最终源码复核后再次执行规定三项检查均通过：格式 3.77 秒、check 1.58 秒／80 条既有警告、非 UI 77 组／278 passed／299 ignored／0 failed（`target/git120-final-017-host-{fmt,workspace-check,workspace-tests}-2026-10-18.log`）。第一次默认并发测试构建因 Windows mmap 报页面文件不足（OS 1455），随后有级联类型错误；没有修改无差异的导航源码或全局系统配置，以 `--jobs 1` 重跑已通过，失败原日志保留为 `target/git120-qa14-workspace-tests-oom-2026-10-18.log`。以上检查在只含本任务增量的 `target/git-ticket119-source/` 导出中执行，显式共享宿主 `target`；真实工作区的其他未提交改动不作为候选输入。QA14 之后的生产及测试源码均未变化，后续只有文档和验收资产。尚未提交推送或关闭 #120。

最终 0.1.7 五项实际包窗口回归已通过 5/5、0 ignored、343.07 秒（`target/git120-final-017-window-regressions-2026-10-18.log`），使用恢复后 clean-source runner `e6ae18b7190ad6c7aba9df0997bec115e3034f6458bd6336b741ba38ab50a852`。共享 UI 81 项按生产控件源码未变复用原通过记录；新增场景追踪只在测试环境显式启用，不替代最终 D3D 原生矩阵。

实际 Package／Manager 的连续活动与静置回归已在修正视口夹具后完成严格红绿：旧 no-op-only 中间候选 `27206737…` 于 51.51 秒红灯，活动第 4.604 秒 End 后的 113 被后台推进为 114，process 1；最终 `8034109e…` 同一公开 900×240 Resize 夹具 1/1、58.84 秒通过，覆盖跨两次真实 Timer 的 7 秒连续按键、停止输入后 15 秒内实际画出外部新提交、真实更新拒绝旧事件、近期输入后 Focus 显式刷新和退役零资源（`target/git120-scene-activity-viewport-{noop-red,final-green}-2026-10-18.log`；runner `848e2a5816b94afbb475e6b50f2ba1a63e993a230a92a9b3a86e1579b7e09f31`）。第一次夹具没有发送原生视口测量，默认高度 0 加 End 滚动使新增首行不在 Paint 内，67.77 秒失败保留为 `target/git120-scene-activity-final-green-2026-10-18.log`；只补真实公开 Resize，没有放宽 Paint／15 秒／旧 revision 断言或重放输入。原 `6db674…` 全量中的同一旧夹具也在 67.56 秒失败：先通过 7014 毫秒／128 个连续按键，再因默认零高度下的 Paint 可见范围失败；下节分别记录原全量和修正夹具结果。

### 最终实际包边界验证

最终包同一 `8034109e…35d71` 的原全量已自然执行完毕，真实 libtest 为 **43 passed／1 failed／0 ignored／2 filtered out，2756.97 秒**；runtime exit 101，外层 PowerShell exit 1，失败原样透传。唯一失败是上述旧视口夹具，其他 43 项均有恰好一个私有测试子进程通过及正常退出收据；原始日志 `target/git120-final-017-package-full-2026-10-18.log` 未改写。2 个默认环境用例被 filtered，不计入这 44 项；计数采用 libtest 和子进程收据，未采用不兼容 nocapture 交错输出的辅助流式解析器计数。

当前源码的受影响场景已在修正 runner `848e2a58…e09f31` 定向通过 1/1、58.84 秒。因此本候选的 **44 场景覆盖为 43 项未变用例通过＋1 项修正夹具通过**，不是同一次全量调用 44 项全绿。[逐名覆盖与来源回执](assets/git-plugin-2026-10-18/package-coverage.json)记录各用例、runner、包身份、真实结果及复用依据。未重复执行未变的 43 项。

原全量冻结 runner 为 `6db674e062c8fea67d46cc420be4612f5541b8a8fb295ac019e0d20542cfb4be`；固定 QA13→QA14 的全部 Rust 差异只有 `workbench.rs` 增加新模块声明及新增 `scene_activity.rs`，另外 43 项所在源码文件在两个固定点逐字节一致。旧 v1 测试完整源码没有单独冻结，不声称重建了该次全部编译输入；复用依据是冻结原 runner 的实际结果、固定差异及仅补公开 Resize 的修改记录。原测试和观察进程均自然结束，测试私有进程无遗留。

实际执行入口为冻结 `target/git120-scene-activity-runner-2026-10-18/git_plugin-bedae8640c39723e.exe --ignored --nocapture --test-threads=1`；定向复验使用 `target/git120-scene-activity-viewport-runner-2026-10-18/git_plugin-bedae8640c39723e.exe --exact workbench::scene_activity::git_package_history_activity_defers_automatic_refresh_until_quiet --ignored --nocapture --test-threads=1`。两者加载同一最终 ZIP，均由各私有测试子进程进入公开 Package／Manager，按既有隔离配置处理 Git、模板及 hooks。

宿主完整固定点至 QA14 `23bf796c7ffc1f02209127116e9284af309c697c`／tree `4a902173008467231fcd4399dad294bd4b2fdfe6` 的双轴复审均为 0（14 个 QA 提交、71 文件、11470 增加／29 删除，Standards 硬性 0／判断性 0、全部 12 baseline，Spec a0／b0／c0）。新增活动场景和规格空闲策略已纳入；此后只增补验收文档与正式截图资产，最终交付前仍核对增量固定点。QA 快照不表示真实分支已经提交。 QA14→QA16 `7fc0daf86048d90e57ccca497bc9324aef2fc973` 的 79 项文档／资产增量已完成 Standards 硬性 0／判断性 0 与 Spec a0／b0／c0；72 个原始文件的固定 Git 字节、大小及 SHA-256 全部匹配，57 个新增本地链接有效。资产目录专用属性保留捕获字节及 Windows JSON 的 CRLF，不影响源码检查。

### 最终原生工作流及八组合

最终包 `8034109e…35d71` 与 clean-source runner `e6ae18b7…a852` 完成连续导航 1/1、139.63 秒：32 组、64 次独立精确 OID 打开、128 个循环控制键；连同两个前置键，raw／sink／Manager admission／执行结果均为 130，0 Stale、0 重发。完整 smoke 1/1、100.49 秒，完整八组合 1/1、882.43 秒且各 case 均正常退出；父进程 exit 0（`target/git120-native-final-017-{phase,smoke,matrix}-2026-10-18.log`）。[最终原生聚合回执](assets/git-plugin-2026-10-18/native-batch-receipt.json)保留 10 个子进程的包／宿主摘要、耗时及清理结果；全部 148 个历史键的四段链一一对应且 0 Stale。没有降低精确 OID、revision、单次输入、焦点、窗口激活、绘制或退出断言。

八组均执行同一完整流程：目录树审查 → 主比较真实预编辑／取消／提交／Undo → 块及整文件暂存 → index 中文提交 → 本地／远程分支树和仅筛选历史 → 键盘与滚轮 → 独立只读历史 → 独立三栏冲突原文档编辑／逐块提案 → 保存解决及真实双亲 Continue → 停用与窗口退役。每组都有 6 张 Windows D3D11 实际像素截图、同包摘要、操作观测和两份退出窗口清单，全部 `shutdown_complete=true`、剩余 GPUI／自有原生窗口 0、意外 Error 日志 0。缩放只发送到测试进程自己的窗口，未修改系统显示设置。

正式截图与 JSON 按原始字节归档，48 张 matrix 图及 smoke 6 图共 54 张均逐张视觉复核，无可行动布局／来源标签／中英显示／主题对比／遮挡问题。以下链接是最终包的正式资产；[摘要账本](assets/git-plugin-2026-10-18/asset-index.json)列明 72 个原始文件的大小与 SHA-256。中文 IME 的原生预编辑、取消、提交及 Undo 已执行；**系统 IME 候选弹窗仍未验证**。验收限 Windows x64 与下列画幅／缩放，不推广为其他平台或任意画幅均已通过。

| 主题／语言／缩放 | 工作台 | 历史独立窗口 | 三栏冲突 | 分支树 | IME 内容 |
| --- | --- | --- | --- | --- | --- |
| 深色／中文／100% | [图](assets/git-plugin-2026-10-18/dark-zh-CN-100/workbench.png) | [图](assets/git-plugin-2026-10-18/dark-zh-CN-100/history.png) | [图](assets/git-plugin-2026-10-18/dark-zh-CN-100/conflicts.png) | [本地](assets/git-plugin-2026-10-18/dark-zh-CN-100/branches-local.png)／[远程](assets/git-plugin-2026-10-18/dark-zh-CN-100/branches-remote.png) | [图](assets/git-plugin-2026-10-18/dark-zh-CN-100/workbench-ime.png) |
| 深色／中文／150% | [图](assets/git-plugin-2026-10-18/dark-zh-CN-150/workbench.png) | [图](assets/git-plugin-2026-10-18/dark-zh-CN-150/history.png) | [图](assets/git-plugin-2026-10-18/dark-zh-CN-150/conflicts.png) | [本地](assets/git-plugin-2026-10-18/dark-zh-CN-150/branches-local.png)／[远程](assets/git-plugin-2026-10-18/dark-zh-CN-150/branches-remote.png) | [图](assets/git-plugin-2026-10-18/dark-zh-CN-150/workbench-ime.png) |
| 深色／英文／100% | [图](assets/git-plugin-2026-10-18/dark-en-100/workbench.png) | [图](assets/git-plugin-2026-10-18/dark-en-100/history.png) | [图](assets/git-plugin-2026-10-18/dark-en-100/conflicts.png) | [本地](assets/git-plugin-2026-10-18/dark-en-100/branches-local.png)／[远程](assets/git-plugin-2026-10-18/dark-en-100/branches-remote.png) | [图](assets/git-plugin-2026-10-18/dark-en-100/workbench-ime.png) |
| 深色／英文／150% | [图](assets/git-plugin-2026-10-18/dark-en-150/workbench.png) | [图](assets/git-plugin-2026-10-18/dark-en-150/history.png) | [图](assets/git-plugin-2026-10-18/dark-en-150/conflicts.png) | [本地](assets/git-plugin-2026-10-18/dark-en-150/branches-local.png)／[远程](assets/git-plugin-2026-10-18/dark-en-150/branches-remote.png) | [图](assets/git-plugin-2026-10-18/dark-en-150/workbench-ime.png) |
| 浅色／中文／100% | [图](assets/git-plugin-2026-10-18/light-zh-CN-100/workbench.png) | [图](assets/git-plugin-2026-10-18/light-zh-CN-100/history.png) | [图](assets/git-plugin-2026-10-18/light-zh-CN-100/conflicts.png) | [本地](assets/git-plugin-2026-10-18/light-zh-CN-100/branches-local.png)／[远程](assets/git-plugin-2026-10-18/light-zh-CN-100/branches-remote.png) | [图](assets/git-plugin-2026-10-18/light-zh-CN-100/workbench-ime.png) |
| 浅色／中文／150% | [图](assets/git-plugin-2026-10-18/light-zh-CN-150/workbench.png) | [图](assets/git-plugin-2026-10-18/light-zh-CN-150/history.png) | [图](assets/git-plugin-2026-10-18/light-zh-CN-150/conflicts.png) | [本地](assets/git-plugin-2026-10-18/light-zh-CN-150/branches-local.png)／[远程](assets/git-plugin-2026-10-18/light-zh-CN-150/branches-remote.png) | [图](assets/git-plugin-2026-10-18/light-zh-CN-150/workbench-ime.png) |
| 浅色／英文／100% | [图](assets/git-plugin-2026-10-18/light-en-100/workbench.png) | [图](assets/git-plugin-2026-10-18/light-en-100/history.png) | [图](assets/git-plugin-2026-10-18/light-en-100/conflicts.png) | [本地](assets/git-plugin-2026-10-18/light-en-100/branches-local.png)／[远程](assets/git-plugin-2026-10-18/light-en-100/branches-remote.png) | [图](assets/git-plugin-2026-10-18/light-en-100/workbench-ime.png) |
| 浅色／英文／150% | [图](assets/git-plugin-2026-10-18/light-en-150/workbench.png) | [图](assets/git-plugin-2026-10-18/light-en-150/history.png) | [图](assets/git-plugin-2026-10-18/light-en-150/conflicts.png) | [本地](assets/git-plugin-2026-10-18/light-en-150/branches-local.png)／[远程](assets/git-plugin-2026-10-18/light-en-150/branches-remote.png) | [图](assets/git-plugin-2026-10-18/light-en-150/workbench-ime.png) |

## 2026-10-18 历史：发布策略与后台工作竞态

移除诊断额外激活后，冻结 QA12 runner `e5f3bd19aa8210025c2735cc9a5cd6e311adb0e31f1198c3aa6d330a80459b97` 在严格静默对照中仍于 103.77 秒失败：Home 的原生 Canvas／paint／Worker revision 均为 92，10 毫秒后实际 Manager 已为 93，原事件按规则拒绝。完整 smoke 和八组合没有启动（`target/git120-native-qa12-phase-2026-10-18/`），不能把窗口激活或辅助绘制调整当成最终修复。

新增只读 actor 场景追踪后，runner `c3f41b55e6de578988024a6de012692511a3cb061e7e6a9ca1ddeeb703d0cdb2` 在 161.53 秒正常首失败收尾，完成 16 组／32 次独立精确 OID 打开；67 个控制键均只发送一次，其中 1 次 Stale。失败键原生 revision 291，9 毫秒内转发；同期内部 poll 把 291 推到 292，process 0→1、resource 11→13，完整 history Document 有 14 处 disabled 变化。该笔属于真实后台工作启动，尚不能由外层追踪分辨内部 Timer 或 Request 回执。较早一笔 290→291 的完整 Document 除 revision 相同、resource 12→11，但它不在失败键的关键边界。所有窗口均退出；未执行 smoke／matrix（`target/git120-history-scene-source-phase-2026-10-18/`，`native.log` 1469–1475）。版本、来源、权限及原输入规则保持不变。

独立确定性回归经冻结 0.1.6 实际包和公开 Manager 截获真实 historyFiles 的 `SetPanelVisibility` 请求：成功回执释放一个资源，却把 scene 19 推至 20；原 19 的单次导航事件因 `StaleRevision` 被拒绝，44.91 秒明确红灯（`target/git120-visibility-ack-016-red.log`）。序列化 dispatcher 的隐藏／冷却 Timer 也已明确红灯。正在修复这些已证实的无变化发布，并审查周期本地检查如何让出连续用户交互；不能把它们扩写成最新原生竞态已经解决。

最小回执修复的临时 0.1.7 包（SHA-256 `27206737ab1f833085c17dd6f82bc73c3731ee7f215a40ae1d54d8a5cfd5d55f`）已通过访客 95/95 及同一真实 Manager 回归 1/1、46.14 秒：成功回执保留 N，原 N 导航只发送一次后定位精确 OID；失败回执仍发布可见错误（`target/git120-scene-receipts-2026-10-18/visibility-ack-noop-green.log`）。该包只是隔离 no-op 修复的中间输入，不是最终 0.1.7 身份。

为确认内部来源，临时探针只在 ignored canonical 导出中编译到独立 runner，随后精确撤回，根生产源码未修改；恢复前后的源码摘要及补丁保存在 `target/git120-history-internal-call-build-receipt-2026-10-18.json`。冻结旧 0.1.6 的来源诊断完成 32 组／64 次精确 OID 操作、128 个循环按键，149.23 秒通过，无重发、无 Stale，退出零窗口；这次通过不消除之前的失败。内部链明确显示 Files 完成后 279 毫秒，Timer sequence 15 使 history 254→255、process 0→1、14 处 disabled 改变；成功可见性回执另有完整 Document 相同却推进 revision 的独立记录（`target/git120-history-internal-call-phase-2026-10-18/`）。

静置策略的六项回归已得到 4 RED／2 通过控制：近期边界 Home、业务完成不足间隔、busy Timer 无变化发布、忙碌期间显式刷新丢失均明确失败；被动观察不推迟检查及空闲显式刷新及时两个控制原本通过（`target/git120-scene-receipts-2026-10-18/quiet-red.log`）。生产修复将周期检查绑定最近明确交互和业务完成后的完整空闲间隔，显式请求合并保留。最终包、实际 Manager 的连续跨 Timer／静置外部变化、原生全流程及八组合仍待验。

完整 QA12 `2b9e154…a513e92ce7782f50254820ac4093a2c0137583f1`／tree `25830ec19e08c101284527f273d36fedb9e870fa` 的双轴复审仍为 0（12 个 QA 提交、67 文件、10836 增加／29 删除）。同点格式检查和显式共享 target 的 workspace check 均通过，后者 3.00 秒、80 条既有警告（`target/git120-qa12-{fmt,workspace-check}-2026-10-18.log`）。误用新 target 的冷构建已主动停止，其记录不计通过；此后的 scene probe 与发布策略改动仍待复审和复验，#120 保持 open。

## 2026-10-18 历史：0.1.6 候选与原生键盘待验

最终候选为 `target/git-plugin-final-120-history-kinds/git-0.1.6.zip`，800814 字节，SHA-256 `01e26499650d81ba6b5d01ff4d67605f33cb66da9d1bab8a95951be55aa323a5`；WASM 摘要为 `651bfa444b6284ad991d4da38bc0a8469e6e3dbeb4ba6ae03d15734715b7fc70`。原生 helper 仍为 0.1.0／`73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。独立源码仍位于仓库外的 `C:/Projects/RustProjects/Git-for-Nanobug`，通过公开 `--plugin-cargo`／`--plugin-package` 入口构建，无宿主路径依赖、随附安装或 Git 专属宿主注册。对应 50 个真实构建源文件归档为同目录 `git-for-nanobug-0.1.6-source.zip`，摘要 `2fd8b630f30d4de24ac66f69c3cfe76e27af2e9f14f60bf45864ba8bb7d77288`；逐条核对归档字节与构建源摘要，不包含 target、SDK 或 .git。未创建远程插件仓库、Release 或市场登记。

文件 `foo` 改为 `foo/bar` 的目录／删除文件身份已经分别建模，目录选择不打开删除叶，合法重复名称可同时显示。实际包 0.1.4 在两项独立 Manager 场景红灯（`target/git120-tree-collision-014-final-red.log`）；0.1.5 暂存树通过，但历史场景暴露 Git tree object 被当成 blob 的另一缺陷（`target/git120-tree-collision-015-green-attempt.log`）。0.1.6 按完整 batch 记录区分 blob／tree／missing，树／缺失文件侧为空，异常或截断记录明确失败；原 blob 的 UTF-8／二进制／大小边界保留。相同严格断言最终 2/2、49.80 秒通过（`target/git120-tree-collision-016-green.log`），包含两侧原文、精确提交 OID、目录无业务副作用和叶选择。

0.1.6 访客测试 94/94、0 ignored、5.28 秒；公开打包成功，7.50 秒（候选目录 `library-tests.log`、`package-build.log`）。在私有 global／system／template／hooks 配置下再次通过 94/94、0 ignored、18.46 秒（`target/git120-final-016-guest-private-tests-2026-10-18.log`），独立源码的直接 `cargo fmt --check` 通过（`target/git120-final-016-guest-direct-fmt-2026-10-18.log`）。最终包五项实际窗口回归通过 5/5、0 ignored、380.55 秒（`target/git120-final-016-window-regressions-2026-10-18.log`）。同包最终实际 Manager／Git 完整回归已通过 42/42、0 failed、0 ignored、2 filtered、1486.91 秒（`target/git120-final-016-package-full-2026-10-18.log`）：逐名核对全部 42 项与清单一致，各私有子进程均恰好 1 项通过并正常退出，无遗留测试进程。默认环境用例的 2 filtered 不计通过。

独立候选交付副本保存于 `C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.6/`，包含安装包、源码归档、`SHA256SUMS` 和明确标记验收中状态的说明。再次解读包内清单，实际 WASM、native helper 字节和清单依赖摘要全部一致（`target/git120-final-dist-package-verified-2026-10-18.json`）。此副本不改变冻结的 50 项构建源码或宿主发行内容。

同包完整原生 smoke 通过 1/1、119.97 秒（子进程 119.85 秒），截图前同时要求当前元数据、所有已展开页和实际可见的选中文件（`target/git120-native-final-016-smoke-2026-10-18.log`）。随后八组合矩阵第一组深色中文 100% 在 124.11 秒失败：End 已选中并揭示最旧提交，但单次 Enter 未将详情切换到该精确 OID；窗口保持激活、原焦点保留，Panel／Worker revision 均为 179，正在核对真实 native Canvas 输入态与异步事件接收。失败日志和截图保留于 `target/git120-native-final-016-matrix-2026-10-18/`，不能将前一次 smoke 或同包五窗口扩写为八组合通过。原 HWND、GPUI 窗口与同 UI 线程原生窗口最终均为空，此次没有退出 TLS panic；系统 IME 候选弹窗仍未验证。

只读按键探针分别记录原生接收、原 sink 转发和真实 Manager 校验／执行，不修改 envelope、顺序或错误处理。缩短的历史诊断 1/1、68.86 秒通过；保留真实审查／中文提交前置的完整 smoke 随后 1/1、118.58 秒通过，各阶段 Enter revision 均为 168，实际 Manager 接纳后文件进程从 0 到 1，精确 oldest OID 和只读来源断言通过（`target/git120-history-key-full-probe-01-2026-10-18.log`）。两轮没有复现矩阵的原失败，不能单凭通过宣布修复。完整入口明确拒绝缩短流程标志，证据区分 `complete-workbench` 与诊断流程。

随后一次启动采样独立 Home／End → 单次 Enter，在第 0 组第二项操作即复现：原生 Canvas／paint／Panel／Worker 都为 68，enabled、原焦点、IME 和 gesture 条件全部满足；47 毫秒后的原 sink 仍转发事件 68，Worker／实际 Manager 已为 69，按原规则返回 `StaleRevision`。不存在旧 Canvas 早退或失败输入重试。原时序 RED 正常收尾于 110.50 秒，runner `a55f17fdcc8e0b0e8ace3812c56c37ec7eeadfcdc46fa25a16c0784df8f8588e` 与日志保留在 `target/git120-history-key-phase-probe-01-2026-10-18/`。独立只读审计确认，同一 Window update 内的同步 checkpoint 写盘会阻止先前 `cx.defer` 在闭包退出前交付；Kit 按键辅助自身也会同步 draw，因此不把全部 47 毫秒归因写盘。正在仅把 checkpoint 改为内存采样、原 deferred 回调排出后再落盘，对照原 32 组循环；产品、原事件、排序、版本校验及单次输入条件均保持。

I/O 单变量对照未消除主要交付窗口：raw → sink 仍为 44–46 毫秒；完成 7 次独立精确 OID 打开后，第 3 组 End 已被实际 Manager 131 接纳，但后续 Enter 的前置门禁 45 秒未满足，尚未发出 Enter，125.51 秒正常失败收尾。此轮没有足够失败门禁事实，不能把它当成同一个 stale 或写盘主因的证明。日志与 runner `621275be82715df619399b131e7499a5ae1b02e5412dfb3029b33a93c0a3a2a0` 冻结于 `target/git120-history-key-phase-io-01-2026-10-18/`。下一轮只补充失败门禁的内存事实，在初始 pointer 阶段出现未计划的 Space 事件及 window_active=false，尚未进入循环，105.91 秒失败；其来源未确定，不归因用户、产品或激活逻辑，也不算第 3 组失败复现（`target/git120-history-key-gate-probe-01-2026-10-18/`）。当前用同一 copy 和新私有 fixture 复测一次。Kit 辅助按键的同步绘制与诊断额外窗口激活动作仍需分别核对。

同一门禁探针的新私有 fixture 在进入额外 phase 激活前，初始 End 再次出现 raw／paint／Worker 58 → 46 毫秒后 Manager 59 拒绝事件 58；没有未计划键盘事件（`target/git120-history-key-gate-probe-02-2026-10-18/`）。下一单变量只让历史 Home／End／Enter 沿相同生产 `Window.dispatch_event` 发送一次 KeyDown／KeyUp，把 Kit 辅助的同步前后 draw 留给正常 `h.frame`；文本／IME 辅助不变。`4172a6e1c38bf3f4808b1fb7866e9081e86d47fc1ac2b789869e183b8fe2dbaf` 对照已发出的 10 个控制键全部被 Manager 接受，raw → sink 为 9–11 毫秒（均值 10），4 次独立精确 OID 打开通过；下一 Home 尚未发出即因 window_active=false 失败于 119.49 秒。原 Focus74 从未丢失，最终 Canvas／Panel／Worker204、enabled／来源／IME 均有效，大小保持 1273×896、100%。因此第二类持续阻塞明确是窗口未激活，而非 Canvas 重建或事件拒绝；不能把此轮写成 32 组通过（`target/git120-history-key-native-dispatch-phase-01-2026-10-18/`）。正在移除诊断循环额外的窗口激活动作，并只读记录前台是否属于测试进程；不读取其他窗口的内容或标题，不强制赋予 FocusHandle，不放宽 active 门禁。

最终宿主检查已通过：`cargo fmt --check`、`cargo check --workspace`，以及非 UI workspace 的 77 组／278 passed／297 ignored／0 failed，分别记录于 `target/git120-release-{fmt,workspace-check,workspace-tests}-2026-10-18.log`。ignored 不计为通过。固定宿主 QA9／QA10 双轴 0 结论保留；最新完整范围 `2b9e1548a91925493ac89f397ff1cd8c0c0380e0...2abfea26dc9623572f9d7964480eda1ac006bc7a`（QA11，11 个 QA 提交、67 文件、10798 增加／29 删除）完成复审，Standards 硬性 0／判断性 0、全部 12 项 baseline，Spec a0／b0／c0。独立访客范围 `6b8433ed8cc04d9179c9a74204943facb10de883...f86f1f3cfff7aca3690dcb4c56d55d49665a3502`（11 个 QA 提交、28 文件）的完整双轴 0 结论仍有效。QA 固定点不表示真实分支已经提交推送；此后的窗口观察与诊断增补仍需复验与增量复审，#120 保持 open。

## 2026-10-18 历史：0.1.4 完整原生流程与树身份边界

独立 0.1.4 候选 `target/git-plugin-final-120-conflict-tree/git-0.1.4.zip` 的 SHA-256 为 `4002a7a509f3da0feeef4c0597a2f510c917fc974d7509578fae0d4bff313c17`，原生 helper 仍为 0.1.0／`73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。这只属于本地候选，未随附、发布 Release 或登记市场。

完整生产原生 smoke 首次通过 1/1、0 ignored、96.37 秒，子进程 96.23 秒正常退出（`target/git120-native-conflict-tree-v014-smoke-2026-10-18.log`）。包括真实分支／目录树、主比较 IME／Undo、块与整文件暂存、index 中文提交、历史滚轮／键盘／独立只读窗口、冲突侧栏展开和选文件、三栏原文档编辑／提案、保存解决及真实双亲 Continue。原 HWND、全部 GPUI 窗口及同线程原生 GPUI 窗口最终均为空，Error 日志为空，没有 TLS panic。截图与逐步证据位于 `target/git120-native-conflict-tree-v014-2026-10-18/dark-zh-CN-100/`。这仅覆盖深色中文 100% 一种组合，系统 IME 候选弹窗未验证，不能据此标记完整八组合通过。

这一轮使用的宿主测试 runner 为 `target/git120-shutdown-fixed-18-runner/editor_app-b49bc269792187de.exe`，SHA-256 `9a55f708617a4fd1c92952f1787b6bf2af42642688897812b863ad85dcf58968`，匹配 PDB 保留。通用关闭／绘制回归从 0.19 秒红灯到 1/1、0.20 秒通过（`target/git120-shutdown-render-{red,green}-2026-10-18.log`）；五项 0.1.3 实际窗口、三项焦点、环境拒绝与新关闭回归合计 10/10、353.15 秒（`target/git120-isolated-host-window-form-gates-2026-10-18.log`），统一隔离后的真实 Worker／恶意配置哨兵 2/2、67.21 秒（`target/git120-isolated-worker-form-gates-2026-10-18.log`）。没有将这些旧包窗口结果扩写为 0.1.4／后续包已通过五项窗口。正常宿主已按该关闭修复重新 build，通过（`target/git120-final-shutdown-production-build-2026-10-18.log`）。

冲突树根因属于访客：同一路径同时存在于实时冲突和保留草稿，`Directory::insert` 的 Vec 生成重复叶。实际 Manager 旧 0.1.3 先验证打开草稿前唯一页正常，再绑定原生未保存草稿并刷新，唯一一次目录请求即因重复 ID 红灯 45.42 秒（`target/git120-conflict-tree-package-v013-red-2026-10-18.log`）；没有宿主重复请求或放宽唯一约束。目录路径幂等插入后，访客两项定向 2/2、0.00 秒及全库 87/87、0 ignored、5.29 秒通过，同一 0.1.4 的实际 Manager 回归 1/1、55.00 秒通过（`target/git-plugin-final-120-conflict-tree/{directory-green,library-tests}.log`、`target/git120-conflict-tree-package-v014-green-2026-10-18.log`）。resolved-only 草稿仍保留首次出现顺序与可导航性。

继续核对发现另一合法边界：文件 `foo` 改为 `foo/bar` 时，Git 状态与历史变更同时包含删除文件和新增目录；两种节点不能共用路径 hash，也不能让目录选择打开删除文件。独立私有 Git 实测及访客紧回归已确证，正在将目录／叶身份与 lookup kind 分开，并准备 0.1.5；最终候选全量、八组合及独立复审待完成。先前 0.1.3 的新隔离全量在 10 项通过后主动停止，只记录部分通过，不报告 39/39。

## 2026-10-18 历史接续：0.1.3 统一历史坐标与真实原生退出

当前候选为 `target/git-plugin-final-120-resize/git-0.1.3.zip`，SHA-256 `462b0fbffa0ca1cc25ae696aa8424f9f7b1936d163c07e8633a358aa1fb5fadb`；源码归档为同目录 `git-for-nanobug-0.1.3-source.zip`，SHA-256 `f2512d1a20d38a74824aa0ef30de64af4d93bdf306177a17dc8707a0cd33768f`，47 个实际构建源文件，不含 target、SDK 或 .git。exact SDK、协议 7 和原生 helper 0.1.0 摘要不变。访客 QA 为 `d4273b1eed17f6edc1b0fc866a66c46f180153f7`。

历史 Resize→Pointer 回归先在 0.00 秒红灯：画面 Review10／OID `000a`，旧点击却选择 Review18／OID `0012`（`target/git120-history-resize-pointer-red.log`）。尺寸变化时保存同一有效滚动位置后 1/1、0.00 秒通过（`target/git120-history-resize-pointer-green.log`）；Scroll/Resize 经公开序列化边界，正向 Pointer 调用同一实际 handler，以避免本机单元测试进入 WASM transport。全库 85/85、0 ignored、16.49 秒通过（`target/git120-guest-resize-all-2026-10-18.log`）；公开打包入口构建通过（`target/git120-guest-resize-package-2026-10-18.log`）。同一新包的 39 项实际 Git/Manager 已全部通过，0 ignored、1215.41 秒（`target/git120-resize-package-full-2026-10-18.log`），但其夹具隔离后续发现 P2，修复后仍需复验；上一 `64318c…a0a5` 候选的长测试已在发现选行缺陷后停止，只有部分通过，不报告完整通过。

同包 native smoke 在 104.07 秒失败（`target/git120-native-resize-smoke-2026-10-18.log`），业务全场景、refs/index/原字节、中文 IME/Undo、只读来源及真实双亲 Continue 全部通过，Error 日志为空；但退出时 TLS panic 仍使进程失败。真实符号栈指向 `accesskit_windows::StringBuffer::acquire → update_window_focus_state → GPUI window_procedure → CoUninitialize → LdrShutdownThread`，不能再将 `<unnamed>` 归因于后台 Worker。上游 `WindowsWindow::drop` 异步安排 `DestroyWindow`，编辑器实体释放并不代表 HWND 已销毁。

空窗及 Kit/原生编辑器的旧退出分层各 1/1、0.42 秒与 1.03 秒通过（`target/git120-native-empty-painted-exit.log`、`target/git120-native-initialized-exit-2026-10-18.log`），只证明实体／输入及进程的旧退出条件；新增退出前真实 `IsWindow` 断言在最小空窗 2.75 秒明确红灯（`target/git120-native-hwnd-retirement-red-2026-10-18.log`），记录实体已释放而 `HWND destroyed before quit = false`。正在仅验收支架中等待所有预先捕获的自有 HWND 真正销毁后再退出；不直接销毁窗口、禁用无障碍、故意泄漏或改上游。完整八组合矩阵仍未执行，不能关闭 #120。

新增 HWND 退出条件修复后，最小空窗 1/1、0.47 秒通过（`target/git120-native-hwnd-retirement-green-2026-10-18.log`），Kit／原生编辑器分层 1/1、0.98 秒通过（`target/git120-native-initialized-hwnd-green-2026-10-18.log`）。当前冻结宿主测试 runner 为 `target/git120-final-18-green/editor_app-b49bc269792187de.exe`，SHA-256 `5a1816a85eb099ee3fca69fb6616a0a793fc2ca5ee1bd047b7789e3bc6a1bc80`，保留同名匹配 PDB。共享 UI 已完成 81/81、0 ignored、201.96 秒（`target/git120-final-shared-ui-green-2026-10-18.log`），包括两项通用树来源交接回归。截图前等待当前树元数据与真实可见叶节点，期间不再次刷新或展开。

完整 smoke 在 113.44 秒仍失败（`target/git120-native-hwnd-final-smoke-2026-10-18.log`）：业务与项目／输入关闭全部通过，预先捕获的主窗和两个插件窗共 3 个 HWND 均已销毁，`remaining=[]`，但判定完成后仍出现相同 AccessKit／COM 线程退出 TLS panic。等待这三个 HWND 是必要条件，尚不足以修复完整场景；不能据此认定根因已解决。后续先检查当前私有 UI 线程的遗漏窗口及应用释放顺序，不扩大到用户窗口。完整八组合矩阵仍未执行。

单变量私有 UI 线程窗口探针在 101.96 秒再次失败并确证根因（`target/git120-native-window-probe-run-2026-10-18.log`）：关闭前的 `WindowId(1v1)/2v1/3v1` 三窗全部销毁后，`cx.windows()` 与同 PID／同线程只读枚举均显示全新的 `WindowId(2v3)/3v3` 两个可见 `Zed::Window`，harness 释放后仍存在，TLS panic 发生在同一 UI 线程。阶段 JSON 位于 `target/git120-native-window-probe-2026-10-18/dark-zh-CN-100/shutdown-windows-*.json`，匹配 PDB 与 runner SHA-256 为 `bf883b06798f5c6d893455ebd3b24c715658e7e5d81de9e61f3eff9f9a07124a`。源码对应 `prepare_project_shutdown` 清空窗口表但保留面板可见性，后续 shell 绘制再次调用未检查关闭状态的通用窗口同步，重新建窗；正在先补真实关闭／绘制回归，再修复通用生命周期门禁，不通过手工销毁遗漏窗口绕过问题。

已将此链收紧为不依赖 Git、WASM 或真实桌面的通用回归：两项普通窗口声明经现有记录型 Worker 发布，真实 shell 建窗，再调用真实 `prepare_project_shutdown` 并重画两轮。旧实现于 0.19 秒明确红灯，原 `2v1/3v1` 关闭后出现新 `2v3/3v3`（`target/git120-shutdown-render-red-2026-10-18.log`）。生产修复仅在通用 `sync_plugin_windows` 对关闭状态早退，保留已保存的面板可见性与原生关闭流程，GREEN 正在执行。原生支架同步要求所有 GPUI 窗口及同线程 GPUI HWND 均为空，捕获／枚举失败不算空集合；同时补充冲突窗口左侧树的真实展开与文件选择，避免截图绕过该交互。

同一 `462b…fadb` 候选的五项实际包窗口回归已通过 5/5、0 ignored、332.13 秒（`target/git120-resize-window-regressions-2026-10-18.log`），包括原文档共享、隐藏草稿、软换行滚动、比较刷新和冲突块审查。此轮尚未接入后续统一 Git 配置隔离，修复后再复验，不能与已证实失败的真实 Windows 退出混为同一门禁。

本轮格式检查通过（`target/git120-final-fmt-2026-10-18.log`）；非 UI workspace 为 77 组、277 passed、294 ignored、0 failed（`target/git120-final-workspace-tests-2026-10-18.log`），workspace check 和正常宿主 build 通过（`target/git120-final-workspace-check-2026-10-18.log`、`target/git120-final-production-build-2026-10-18.log`）。ignored 不计为通过，之后的夹具修复按影响补验证。

### 宿主完整复审与新增夹具隔离 P2

固定完整范围 `2b9e1548a91925493ac89f397ff1cd8c0c0380e0...80fd1e7d60832173f531f719deea4c4a978d02c8`，7 提交、57 文件、8710 增加／28 删除，双轴均已完成。Standards 硬性 1／判断性 0，Spec a0／b0／c1，最高 P2；两轴指向同一测试隔离问题，不重复计数。新增实际包及后台 Worker 夹具未从首次 init 到后续 Manager Git 一致隔离用户 global/system/template/hooks，可能执行用户 hooks 或 fsmonitor。此前 Native 子进程隔离修复保持关闭，本项为不同测试入口，正在修复；无其他新增生产源码发现。普通分支和真实 index 未因 QA 快照移动，#120 保持 open。

### Standards（0.1.3 最终访客）

独立审查完整固定范围 `6b8433ed8cc04d9179c9a74204943facb10de883...d4273b1eed17f6edc1b0fc866a66c46f180153f7`，10 提交、24 文件、2934 增加／216 删除，差异非空。硬性违规 0、判断性坏味道 0，最高无；完整覆盖 12 项 baseline。外部 WASM、无宿主路径依赖的公开 SDK、显式安装及原生依赖私有目录边界保持。

### Spec（0.1.3 最终访客）

同一完整范围分类 a/b/c 各 0，最高无，前轮 resize 后指针选错 OID 的 P2 已关闭。审查区分公开 dispatcher 的 Scroll/Resize 与直接实际 Pointer handler 的正向单元断言，不将后者扩写为 Manager 全链路证据；真实 Manager／原生门禁仍分别执行。静态结论不能代替未结束的实际包、原生退出或矩阵验收。

### 通用树刷新回归

真实截图可能捕获正常页加载瞬态，单凭占位不能证明卡死。通过现有冻结 `database-tree-0.1.1` 包、公开 Manager、真实 EditorState/PluginView 加入回归后，来源／禁用负对照 1 项通过，来源交接后自动恢复已展开页面 1 项在原实现明确红灯，2 项共 77.06 秒（`target/git120-tree-source-refresh-red-retry-2026-10-18.log`）。首次编译仅缺测试 `DocumentVersion` import，不计产品红灯。

最小宿主修复把当前已展开、启用、未加载且无错误的元数据恢复放到资源 revision 相等的提前返回前，保留原来源、节点与 deferred 权限检查，不重放选择或命令。正例支架进一步按真实异步交接分两次 Window update：旧来源撤销后先严格证明零请求，新场景发布后允许元数据立即完成或在后续 pulse 完成；所有原断言及负对照保留。两项均已在 81 项共享 UI 集合通过；旧 RED 对应此前单 turn 支架，不宣称新支架已重新跑过 RED。

## 2026-10-17 恢复：独立包与原生流程（历史）

### 当前 0.1.3 候选与未完成门禁

当前独立候选为 `target/git-plugin-final-120-columns/git-0.1.3.zip`，SHA-256 `64318c7f0e36282c3e79ed91c382a4a099e5e16d760ae411c7d0be248f73a0a5`，exact SDK 指纹不变。历史表头错位回归先红 1 项、0.00 秒，修复为原生 Row/Text 显式列宽后 1/1、0.05 秒绿（`target/git120-history-columns-{red,green}.log`）；同源码升级清单/Cargo/双语说明至 0.1.3，全库 84/84、0 ignored、21.15 秒通过（`target/git120-guest-header-v013-all.log`）。公开 `--plugin-package` 构建通过（`target/git120-guest-header-v013-package.log`），原生 helper 仍为原版本/摘要，没有扩大宿主 API。

新源码档案 `target/git-plugin-final-120-columns/git-for-nanobug-0.1.3-source.zip` 保存 47 个实际构建源文件，SHA-256 `75322d6fcedd52389bdbafa9d4b6a96ea06b90301914572fde5cccbb32e70665`；本地 QA HEAD 为 `6768d0a81f5d3a4719a200f38f13bacac115dc4b`，不包含 target、SDK 缓存或 .git。新包的 39 项实际 Git/Manager、5 项窗口以及原生 smoke 正在执行，未计为通过；其双轴完整复审也在进行。

### 0.1.2 候选与前轮门禁历史

当前独立候选为 `target/git-plugin-final-120-canvas/git-0.1.2.zip`，SHA-256 `7ef68b1ff0d38e950c3c8953165b98b4e154be589811f707c85d2e6e7cdd162e`，exact SDK 指纹不变。通过公开 `--plugin-package` 构建（`target/git120-guest-canvas-package-build.log`），访客 83/83、0 ignored、22.49 秒（`target/git120-guest-canvas-all-green.log`）。源码档案 `target/git-plugin-final-120-canvas/git-for-nanobug-0.1.2-source.zip` 包含 47 个审查过的源文件，逐文件与本地 QA HEAD `739629dbd1c86126662f24b62026de7588af007e` 的检出文件及实际构建输入比较，内容字节一致；SHA-256 `326d2da51d27d32161444d41c648ddea7d6dedc511989d0f9295e77dddf4c5f`，不含 target、SDK 缓存或 .git。另保留 Git 规范化换行的源码归档 `*-source-git.zip`（`66ba63a6c073d69fa7a00081d3c093f288ea407d4c628885a7b9348f5dba7a38`），不与逐字节构建输入档案混用。

最新包完整实际 Git/Manager 34/34、0 ignored、692.16 秒（`target/git120-canvas-package-full.log`），显式执行 ignored，含同 revision 的 Focus→Down、大历史滚轮、并行树页、安装/卸载/重开及全部高级 Git 操作。之后只有测试入口环境保护变化，没有修改访客、Manager 或生产 Git 路径；带保护的 runner 追加大历史、冲突恢复、独立包生命周期三项 3/3、134.76 秒（`target/git120-guarded-history-recovery-independence.log`）。非 UI workspace 最终为 77 组、277 passed、289 ignored、0 failed（`target/git120-final-guarded-workspace-tests.log`），新增 1 项仓库重定向子进程拒绝回归；ignored 未计为通过。

前一滚轮候选 `512555257a25482fe44fb17a05e4c7c56eaa78ee3b6271611444fd751cbec534` 的 82/82 访客通过记录保留（21.67 秒，`target/git120-guest-native-wheel-all-green.log`）。该包完整实际 Git/Manager 34/34、0 ignored、877.80 秒（`target/git120-wheel-package-full.log`），五项实际窗口 5/5、487.58 秒（`target/git120-wheel-package-windows.log`），均显式执行 ignored，不将前包结果标为当前包通过。

上一尺寸幂等候选 `73e1caf59d4f5361d666703620d36cd271e3e3841f12d4e0d1eb612d38ec54f9` 的全部实际 Git/Manager 测试 34/34、0 ignored、823.45 秒（`target/git120-passive-package-full.log`），显式执行 ignored，含重复尺寸观测后两树分页、目录选择及独立安装/卸载/重开；同包五项实际窗口 5/5、454.69 秒（`target/git120-passive-package-windows.log`）。冷启动命令正向回归 1/1、105.67 秒（`target/git120-resume-cold-command-positive.log`），真实 check-errors 命令执行后原始冷 epoch 仍有效。紧凑布局候选 `3af43657eaac5d631e2ba7fb5fd3785214963f51ef7df7017654248bba3194f6` 的 34/34、907.96 秒（`target/git120-compact-package-full.log`）亦保留为独立历史结果，不与最新候选混合计数。

宿主通用 UI 集合 72/72、121.37 秒，包含原 71 项及新增本地多行输入回归（`target/git120-compact-shared-ui.log`，显式 include-ignored）；原生同步、可见性、文档 IME 与侧边标签合计 12/12、0.78 秒（`target/git120-shared-lifecycle-and-input.log`）。非 UI workspace 为 77 组、276 passed、289 ignored、0 failed（`target/git120-resume-workspace-tests.log`），ignored 未计为通过。后续表单修复仍需更新受影响门禁。

本地 Textarea 缺少 Base 语义外框的回归先失败 1 项、0.02 秒（`target/git120-textarea-semantic-red.log`），补齐原有 TextareaState 的 InputBase 框后 1/1、0.03 秒通过（`target/git120-textarea-semantic-green.log`）。真实点击焦点、中文/emoji、换行和同一原生状态均有断言，没有复制编辑器或增加可变文本源。紧凑历史底栏把常驻仓库策略移至信息对话框，截图已显示默认停靠区三个完整提交行；重复 Canvas Resize 由访客在真实尺寸不变时保持当前场景，真实变化仍发布。

最新原生 smoke（`target/git120-passive-native-smoke.log`）127.26 秒失败：保存原文并刷新后，差异块暂存和整文件暂存均已执行；真实输入框显示完整中文提交说明，但插件未确认其值，尚未进入 Commit/历史/冲突步骤。诊断定位通用 Input/Textarea 变更在场景更新期间可能被拒绝，而原生 last_value 已提前记作送达；正在按真实回调接缝补回归与修复，不能只延长等待或放宽值断言。证据在 `target/git120-native-passive-2026-10-17/dark-zh-CN-100/`。此前原生失败的暂存块问题是夹具未保存 IME/Undo 后的原文，已补真实 Ctrl+S、等待干净文档并执行公开 refresh-hunks；没有绕过脏缓冲检查。额外冷启动 Refresh 命令已移除，改为面板按需启动后点击可见 Refresh，保留命令自身来源门禁；最新原生日志没有旧启动命令 Error。

表单最小回归 `ui::plugin::form_delivery_tests::native_form_draft_crosses_scene_handover_without_losing_its_change` 在真实 Base 点击、焦点及中文原生输入后，以 0.03 秒复现新场景收到 Change 数为 0、期望 1（`target/git120-form-handover-real-red.log`）。更早的同名首次运行只失败于单行控件快照未声明 focused，属于夹具元数据错误（`target/git120-form-handover-red.log`），不作为业务红灯；修正为读取原 Base 状态的 focus/value，保留真实 snapshot 定位点击和 Textarea 元数据断言，未直接调用 focus 或写状态。后续修复使用私有投递回执，所有新投递仍经过当前精确 revision 校验；显式 value_revision 覆盖、退役与移除不能复用旧草稿，接收成功不依赖插件回声，防止循环发布。

### 0.1.2 访客 Standards

固定点 `6b8433ed8cc04d9179c9a74204943facb10de883...739629dbd1c86126662f24b62026de7588af007e`，完整 8 提交、24 文件差异非空。两路独立正式审查已完成：Standards 硬性违规 0、判断性坏味道 0，最高严重级别无；覆盖完整 12 项 smell baseline。前一 `03b29f0` 的审查结论保留为其对应历史身份。

### 0.1.2 访客 Spec

同一完整范围分类 a/b/c 各 0，最高严重级别无；独立 WASM、公开 SDK 和通用宿主能力边界保持。静态审查不代替尚未通过的原生流程与最终候选门禁。

### 表单修复、原生滚轮与后续窗口退休

通用表单四项回归 4/4、0.21 秒（`target/git120-form-delivery-green-retry.log`）；首次完整编译仅因私有方法可见范围小于实际 surface 调用方而失败，已限定在 `crate::extensions` 内修正。真实后台 Worker 回执 1/1、58.56 秒（`target/git120-form-actual-manager.log`），实际候选经公开 Manager 证明 Stale 不送入 guest、当前 Change 得到 Delivered、旧实例得到 Retired、Click 不能借 Change 通道重试。通用 UI 76/76、120.51 秒（`target/git120-form-shared-ui.log`），包含旧单行输入回声/显式清空回归。仅待下一 runner 纳入既有 i18n 日志文案；协议与 SDK 未变。

随后原生 smoke 在 137.01 秒失败（`target/git120-form-native-smoke.log`）：真实本地/远程分组与目录、筛选 incoming 不切换 HEAD、All refs 恢复、比较 IME、块/整文件 index 和中文提交均已完成。提交后新增 HEAD 的父提交、唯一变更路径、index 清空及 28 个未跟踪文件保留都有真实 Git 断言。失败步骤为历史原生滚轮，退出另报告 ExtensionPanel 句柄；证据在 `target/git120-native-form-receipts-2026-10-17/dark-zh-CN-100/`，仍不作为完整原生通过。

滚轮缺陷在独立访客：宿主按公开约定发送像素 `Wheel`，访客只处理绝对 `Scroll`。公开序列化回归在 0.00 秒复现 -160 像素后 offset 仍 0、期望 160（`target/git120-guest-native-wheel-real-red.log`）；首次误写模型字段名的编译失败 `target/git120-guest-native-wheel-red.log` 不算行为红灯。修复复用历史边界计算，仅真实移动才发布场景。旧 73e1 包两项实际 Manager 回归也在 63.45 秒明确失败（`target/git120-guest-wheel-manager-red.log`）：385 提交滚轮 offset 为 0、期望 7280，以及边界轮动把已申请两树页的 revision 从 16 推至 17。最新包将其纳入全量 34 项复验。

原生手势夹具同步区分“单击提交打开独立窗口”和后续滚动：等待该选择及新窗口真实完成，再普通激活主窗，确认原 Canvas 焦点仍保留后滚轮；没有改变已确认的单击规则或直接投递 Canvas 事件。窗口退出泄漏在独立复制的旧测试程序中再次复现，129.85 秒（`target/git120-native-leak-probe.log`），`LEAK_BACKTRACE=1` 未提供有效分配栈，因此不把该日志作为引用来源的直接证明。依据窗口所有权检查，将原生关闭回调对 ExtensionPanel 的强引用改为弱引用：活动窗口根继续拥有面板，Win32 异步销毁期间保留的回调不再延长其生命周期。原有输入退休、指针释放、可见性记录及文档保留顺序不变。新 runner 已构建，正以最新独立包复验原生 smoke 和五项窗口回归；没有移除泄漏检测或增加固定睡眠。

弱引用修复后的原生运行在 119.56 秒失败于历史选择等待（`target/git120-wheel-owner-native-smoke.log`）。该次 `timeout-wait.png/json` 已明确显示新提交首行选中、详情面板 visible，场景从 145 更新到 262，不能将其归为点击未生效；等待同时要求详情激活时主窗仍有焦点，正在将严格焦点检查放回普通激活主窗之后并补充分项诊断。项目关闭和输入退休均成功，退出未再报告 Entity38；这仍不是完整原生通过。最终 workspace check 已通过（`target/git120-final-workspace-check.log`，9.74 秒），正常宿主已从隔离源码构建（`target/git120-final-host-build.log`，1 分 52 秒）。

独立访客对无业务 Canvas 观察的公开序列化回归先在 0.00 秒失败：Focus 导致无变化视图发布（`target/git120-guest-canvas-observation-red.log`）。统一历史 Canvas handler 返回实际变化后，无业务 Focus/Move/Up/Text、无效或带修改键导航保持场景，真实选择/滚动/Enter 继续执行。首次正向测试误让本机单元夹具进入 WASM 宿主 I/O，触发 SDK unreachable（`target/git120-guest-canvas-observation-green.log`），属于夹具限制，不计产品失败或通过；修正为直接验证真实 Down 选择及 Files 队列，公开 dispatcher 仍验证全部无业务观察，1/1 通过（`target/git120-guest-canvas-observation-fixture-green.log`）。实际 Manager 的同场景 Focus→Down 正向验证已加入已有大历史用例，等待新包运行。该风险与上一段已点击成功但焦点等待超时分开记录。

### 宿主首次完整审查与修复中

宿主冻结点为本地 QA commit `acb88d8f36a66ab954c3e1552491ff38e7a22ceb`，固定基线 `2b9e1548a91925493ac89f397ff1cd8c0c0380e0`，完整 47 文件差异非空；实际分支 HEAD 和真实 index 均未移动。

Standards：硬性违规 1、判断性坏味道 0，最高 P1。新原生冲突夹具及大历史 fast-import、恢复 merge 共三处直接 Git 启动遗漏 `GIT_INDEX_FILE/GIT_DIR/GIT_WORK_TREE` 清理，已按其余同类启动点补齐环境隔离和原因注释，等待复验、复审。

Spec：(a) 缺失 0、(b) 范围扩展 0、(c) 实现错误 1，最高 P2。旧表单 flight 回执尚未处理时，连续输入的最新 draft 可能未入 Worker 队列，当前 Click/Submit 可先执行并消费旧值。真实 Base 连续输入后立即点击/Enter 的两项回归已在 0.06 秒明确失败（`target/git120-form-ordering-real-red.log`），不是编译或夹具错误。修复只对尚未入队当前场景的最新 draft 门控业务激活，保留原始 revision 和后续显式动作，不缓存或重放旧业务事件；最新同场景 Change 已在 flight 时仍按 Worker FIFO 放行。最终审查与门禁未完成，工单不关闭。

第二次宿主完整冻结范围为 `2b9e154...e17be144b27b12a5065987cf0d49598d62f89f20`（2 提交、48 文件）。Spec a/b/c 均 0，P2 源码修复确认；表单 7/7、0.44 秒（`target/git120-form-ordering-green.log`），覆盖真实按钮/Enter、Input/Textarea 连续输入、场景交接、Dismiss、未重放旧动作与旧四项生命周期。首次新断言使用了不存在的 Base `is_disabled`，编译失败记录 `target/git120-form-ordering-green-build.log`，已改为公开 `is_editable` 并保留可编辑性断言；其不计产品红灯。

Standards 对完整 12 项 smell 复查后仍为硬性 1、判断性 0、最高 P1：清理前三变量未涵盖 `GIT_COMMON_DIR/GIT_OBJECT_DIRECTORY` 等。将补全 Git `rev-parse --local-env-vars` 的仓库环境前置拒绝，并拒绝 namespace/askpass，诊断只显示变量名。旧原生子程序已在父进程自有 `target/git120-native-environment-real-red/` 和缺失候选下证明拒绝过晚：child exit 101，但 `case/workspace` 已建立（`target/git120-native-environment-real-red.log`）；该输入必在 Package/GPUI 之前失败，不曾启动桌面或使用用户仓库。新父子回归将检查在任何 fixture 目录/Git 写入之前拒绝并保持 sentinel。

隔离保护已在宿主测试和实际 Git 集成测试的共享入口落地，覆盖 Git 报告的 15 个 local-env 名称与 namespace、两种 askpass，共 18 项；只拒绝继承值，不修改进程全局环境。宿主私有 cfg(test) helper 同时供原生、后台 Worker 和既有窗口夹具使用，不是生产宿主 API。原生负向子进程回归 1/1、0.04 秒（`target/git120-final-native-environment-green.log`），在任何工作区/profile/output 创建前拒绝，哨兵字节及目录项保留。Runtime 的同类回归进入上述 277 passed。最终 workspace check 3.42 秒通过（`target/git120-final-guarded-workspace-check.log`）。

通用 UI 已更新为 79/79、0 ignored、97.20 秒（`target/git120-ordering-shared-ui.log`），使用复制的同源码测试程序防止 Cargo 重链正在运行的文件；后续变化仅为 Git 夹具保护和原生验收等待，UI 生产代码及这 79 项输入不变。最新包后台 Worker 回执 1/1、46.25 秒（`target/git120-canvas-final-form-actor.log`），显式执行 ignored。

原生树等待修正后再次运行 122.26 秒，失败于 index-only Commit 等待（`target/git120-canvas-ordering-native-smoke.log`）；HEAD 未变、无 Git 业务 Error，不能算通过。新增表单门控可能使本地按钮短暂禁用，但夹具只检查 guest Node；已核对 Base 公开观测行为：enabled 未聚焦按钮保留 focus handle，元数据为 Some(false)，disabled 为 None。正在把“当前 guest-host、已绘制几何、真实原生焦点能力”与一次点击合并在同一 Window 更新，保存点击前后事实，不重发 Commit、不降低 HEAD/index 断言。完整原生矩阵尚未通过。

最新独立包五项窗口回归 5/5、0 ignored、342.72 秒（`target/git120-canvas-final-windows.log`），显式执行 ignored；正常宿主从同一隔离生产源码构建通过，1 分 06 秒（`target/git120-final-production-host-build.log`）。第三轮宿主完整范围为 `2b9e154...cb1d25ce6f77b01153325240ac98115510104f98`（3 提交、51 文件）：Standards 硬性 0、判断性 1（P3，operations 夹具重复维护部分 Git 环境名单），Spec a/b/c 各 0。该 P3 已改用共享的 18 项前置拒绝入口，等待最终编译和复审。

原生按钮等待修正后的 smoke 在 127.85 秒失败（`target/git120-native-ready-smoke.log`）：中文 Commit 已完成，真实 HEAD、父提交、提交说明、仅提交 index 和未跟踪文件保留全部断言通过；提交首行点击后新 revision、所选标题、精确 OID 及独立详情窗口均已确认。失败为普通激活主窗后 Canvas 焦点未恢复，捕获的 FocusId 74 被 FocusId 38 替换。两路源码调查定位通用宿主 `SetPanelVisibility → ExtensionPanel::show` 把独立面板的 focus 写入父窗口；停靠栏未切换 tab、Canvas 实体未重建。正在补最小焦点归属回归并修复，不通过第二次点击或强制聚焦掩盖失败。项目关闭与原生输入退休成功，退出未报告实体泄漏；完整八组合矩阵仍未执行。

新增最小焦点回归在 0.48 秒明确红灯 2 项、普通 dock 正对照通过（`target/git120-window-focus-real-red.log`）。修复让独立面板不把句柄写入父窗，在 `WorkWindow` 根挂载同一 focus，并只在目标新窗创建时初始化；后续发布保留子控件焦点。三项 3/3、0.49 秒通过（`target/git120-window-focus-green.log`），格式通过（`target/git120-window-focus-final-fmt.log`）。P3 共享 guard 后的完整非 UI workspace 再验为 77 组、277 passed、289 ignored、0 failed（`target/git120-shared-environment-workspace-final.log`）；当前开始复验同包原生 smoke、五窗口与正常宿主构建，矩阵尚未通过。

第四轮完整宿主正式审查为 `2b9e154...12dbe2fa0bd16f1d689971960d7c7e54dd405b6f`（4 提交、52 文件）：Standards 硬性 0、判断性 0，完整覆盖 12 项 baseline；Spec a/b/c 各 0，最高级别均无。配套正常宿主构建通过，54.84 秒（`target/git120-window-focus-production-build.log`），workspace check 通过（`target/git120-window-focus-workspace-check.log`）。共享可见性 3/3、0.31 秒，旧代码组同步 4/4、0.30 秒，冷观察 1/1、0.13 秒，环境负向 1/1、0.05 秒（`target/git120-window-focus-shared-*.log`）。

同一焦点修复后的 native smoke 在 102.34 秒失败（`target/git120-native-focus-smoke.log`）：主窗 Canvas 74 焦点保留、真实滚轮、End 选中最旧行、Enter 打开该提交 OID、首提交空树对比及两个只读来源均已通过。失败在“点击只读列 → 同次更新输入 → 立即读 snapshot”的组合焦点/值断言，尚未区分哪一项；原生指针投影会通过 `window.defer` 等待 listener 表释放，因此正在补点击即刻／真实帧后／输入后三阶段事实与同一手势的真实获焦等待。不重发点击、不赋予焦点、不放宽只读文本断言，也未修改产品代码；证据在 `target/git120-native-focus-2026-10-17/dark-zh-CN-100/`。项目关闭和输入退休均成功，但该次退出另出现 TLS destruction panic，仍计失败，不能算最终原生通过。

最终焦点生产源码的五窗口已全部通过，5/5、0 ignored、406.06 秒（`target/git120-window-focus-final-windows.log`）。随后分阶段 native 运行在 114.39 秒失败（`target/git120-native-readonly-smoke.log`），已明确只读列仅为夹具采样过早：即刻 active=true/focused=false，真实帧后 focused=true，输入后仍为 Document 且原值保留。进入真实冲突后，从基准恢复、应用非冲突更改、接受左块三次真实操作及原文结果均通过；下一处相同的“点击结果列后立即断言原编辑器焦点”失败。现将主比较及冲突两处都改为等待同一物理点击自然完成后的真实帧和原实体焦点，不分配焦点、不重发手势。退出再次有 TLS destruction panic，已独立调查，下次开启原生回溯；不把成功的业务片段算全流程通过。

证据盘点还确认三个成功路径缺口，正补实际包测试：interactive squash（现有 reorder/reword/fixup/drop 正向保留）、插件触发 Skip/Abort（现有 Continue/重开正向保留）、普通 stash pop/drop 成功（现有冲突、selector shift、拒绝与 apply 保留记录继续有效）。这些只补测试，不扩展产品范围或修改生产宿主 API。

成功路径补测现已完整 5/5、0 ignored、254.61 秒（`target/git120-positive-completions-first.log`）：除上述三组外，补齐了 cherry-pick 首步冲突后同安装重开、Skip 并仅执行第二步，以及通过插件创建分支、明确切换、选中文件丢弃的预览/取消/确认。全部使用同一 `7ef68b…162e` 包；refs/HEAD/index/字节/原分支和资源终态有独立 Git 断言。既有 operations 的隔离工厂及 commit helper 原体、参数、路径和四项配置纯移父模块私有共享，不改变原 34 项输入。完整非 UI workspace 再验 77 组、277 passed、294 ignored、0 failed（`target/git120-positive-final-workspace-tests.log`）；新 ignored 只在上述显式命令计通过，默认运行未执行它们。格式通过（`target/git120-positive-final-fmt.log`）。

第五轮冻结范围 `2b9e154...5571ff496aabde17f3d40d7dc2de10acd1f31589`（5 提交、53 文件）Standards 硬性 0、判断性 0；新的独立 Spec 复核 a/b 为 0、c 为 1（P2）：原生夹具会继承用户全局 Git hooks/template，可能执行夹具外程序。私有 global/template/hooks 哨兵通过旧 runner 在 0.33 秒稳定复现，首个 commit 失败且 GUI 未启动；`target/git120-native-global-config-red-2026-10-17/` 保存 red-evidence 和 child log。正在仅测试子进程隔离 global/system 配置、空模板及 hooks；生产插件继续遵循正常用户 Git 配置。

原生后续两次失败仍保留：`target/git120-native-pointer-frame-smoke.log` 为 139.22 秒，滚轮已更新但 End 未进入最旧行；该等待只读取 worker offset，未等到原生 Canvas 当前且 enabled，补同一 Window 内 current node/原焦点/active 后单次发键，无重放。新 `target/git120-native-current-canvas-smoke.log` 在 147.71 秒失败于非冲突更改结果，历史 End/Enter/只读输入已过；点击时 host/worker 同 revision、Base 按钮可用，但尚未核对上一原生编辑的 source revision 已由 guest 读取确认。正在补现有公开 DocumentVersion 的一致性等待和分项源版本/结果事实，不更改业务断言。两轮带回溯的退出未再出现 TLS panic，尚不能宣称该退出问题已有确定修复。

全局配置隔离回归现已 1/1、0 ignored、1.33 秒通过（`target/git120-native-git-config-green-final.log`）：完整 Git 历史和真实冲突阶段已建立，自有 hook/fsmonitor 未执行、模板未复制、配置及哨兵原字节保留。此前第一次编译因测试诊断对 `Option<Arc<Document>>` 的借用写法失败，修正后一次运行仅在 Windows 分隔符字符串比较失败（1.10 秒）；将断言改为路径比较后通过，未降低 hooks/template 或 Git 结果断言。最新 source-version runner SHA-256 为 `1de49852d6a8fb2d6b474173ee972fd28a828fbf2ce04641afc7600959136bfe`，正在复验同一独立包的原生 smoke；生产源码和插件包均未因本次夹具修正改变。

第六轮宿主固定范围 `2b9e154...7f5d95a23b7eec499561b59834a6328c57599c0e`（6 提交、54 文件）两轴均复审为 0：Standards 硬性/判断性各 0、覆盖 12 项 baseline；Spec a/b/c 各 0，上一轮夹具隔离 P2 已关闭。格式从隔离源码工作目录执行 `cargo fmt --check` 通过（`target/git120-native-configuration-final-fmt.log`）；从根目录传相对 manifest 的 fmt 命令曾报 `Failed to find targets`，不计通过。

最新同包 smoke 107.06 秒仍计失败（`target/git120-native-source-version-smoke.log`）：业务场景首次完整通过，包括真实块/文件暂存、中文提交、历史点击/滚轮/End/Enter/只读、三项合并提案、同一结果编辑器的 IME/Undo、Save and resolve 以及真实双亲 Continue；所有 repository 字节/refs/index 断言通过，Error 日志为空。项目关闭、输入退休及主窗实体释放均已报告成功，`evidence.json` 的场景与 shutdown receipt 为 true，但随后未命名线程 TLS 析构 panic 使进程退出 101，不能以 receipt 替代完整退出。证据为 `target/git120-native-source-version-2026-10-17/dark-zh-CN-100/`。正在建立真实空窗口的有界最小退出回归，不使用 `process::exit`、故意泄漏或关闭实体检测规避异常；完整八组合矩阵仍未执行。

同一真实截图复核发现历史表头使用空格 Text，未与按实际 Canvas 宽度计算的数据列对齐。新增公开场景回归在原 0.1.2 源码明确红灯 1 项、0.00 秒（`target/git120-history-columns-red.log`），覆盖中英文、宽度、字号与滚动；正在仅独立插件中改用显式原生列宽。最终候选身份将随修复更新，不能直接把旧包结果记为新包通过。

### 目录候选与前轮诊断历史

宿主 HEAD 仍为 `2b9e1548a91925493ac89f397ff1cd8c0c0380e0`，没有新提交或推送。当前独立包为 `target/git-plugin-final-120-directory/git-0.1.2.zip`，SHA-256 `763884848945a02906c7b3c484ede11e803ea7f9e7bc2a4cbfae6c9a79e6ee63`，exact SDK 指纹仍为 `d97c7f684cb7a4ab0d4eb643f0ed45d8a521483a592a239f0f7cb3277ff8f60f`；日志 `target/git120-resume-candidate-build.log`。访客 `79/79`、0 ignored、18.87 秒（`target/git120-resume-guest-all-green.log`）。未创建远程插件仓库、Release 或市场登记。

目录选择真实 Manager 回归先以 B20 旧包在 44.93 秒复现 `StaleRevision: UI document was replaced`（`target/git120-resume-directory-package-red.log`）。最小访客修复由树处理自身返回是否发生业务变更，成功纯数据页及目录/未知/忙碌选择不重发无变化视图，真实文件/ref/错误继续发布；不降低 Manager 校验。新包的两项树回归和独立包生命周期共 3/3、89.43 秒（`target/git120-resume-directory-and-independence-clean.log`）。生命周期验证同一宿主空配置无 Git，显式安装 ZIP 后按需启动，卸载撤销资源，保留数据并重开仍不会恢复已卸载插件。

冷环境/焦点回归 1/1、0.14 秒（`target/git120-resume-cold-final-green.log`）：epoch0 不缓存/发送观察，启动及替换实例各发送相同当前主题一次，旧焦点订阅不得借用新实例。首轮重跑仍失败（`target/git120-resume-cold-green.log`），已查明夹具新窗未激活导致 GPUI 不派发焦点事件，且缓存断言跨过真实重画；修正公开 activate_window 和同一次更新内采样后通过，没有放宽通知断言。

新原生 smoke 53.42 秒失败在 `native input commit-message not painted`（`target/git120-resume-native-smoke.log`）；真实截图显示表单可见，但本地 Textarea 缺少 Base 语义外框。目录展开、可编辑比较、中文预编辑/取消/提交/Undo均实际通过对应步骤，正常项目关闭和原生输入退休成功，无 Entity20 泄漏。证据 `target/git120-native-directory-2026-10-17/dark-zh-CN-100/`；这些部分结果不计为完整流程通过。截图还显示默认历史画布被常驻说明挤压，日志有启动命令 StaleRevision Error；两项均待解决，完整八组合矩阵未执行。

一次实际包测试继承了主线程用于隔离暂存的 `GIT_INDEX_FILE`，其夹具 Git 写到了临时 index，导致该轮失败（`target/git120-resume-directory-and-independence-green.log`），不计通过。已停止该轮、从已存 tree `dbbfcddf2ff7a7a9f72727ae6d4559c10822c607` 重建，并重新应用精确任务增量；新活动 index 为 `target/git-ticket120-active.index`，其 `.backup` 为最近有效备份。执行测试前清除临时 index 环境，夹具也显式移除外来 Git 目录/index 选择；原工作区与真实 index 未改。后续只使用新活动 index，不复用已污染的 `target/git-ticket119.index`。当前源码门禁 fmt/check 通过（`target/git120-resume-host-fmt.log`、`target/git120-resume-workspace-check.log`），最终门禁仍需随后续修改更新。

### Standards

访客固定点 `6b8433ed8cc04d9179c9a74204943facb10de883...158ba6cf9cc4f825f67647e1d32c7939ca0e465a`，完整 4 提交、23 文件差异非空。硬性违规 0、判断类问题 0，最高严重级别无。

### Spec

同一访客完整范围分类 a/b/c 各 0，最高严重级别无；独立插件交付边界保持。两轴仅静态源码结论，不代替未完成的生产界面和最终候选门禁。

## 工单 02：工作台增量验证（2026-10-16，已暂停）

11:57 暂停快照：B20 的其余 28 项已结束 27 项且全部通过；reset 模式用例仍在运行时由用户暂停请求中断（`target/git120-final-package-remaining.log`），没有完整测试摘要，不报告 28/28。新增 `tree_request_tests::directory_and_ignored_selection_dispatch_preserve_the_admitted_scene` 实际 1 项红灯，0.00 秒（`target/git120-directory-selection-guest-red.log`）；首次少模块前缀导致 0 项测试的命令不计验证。宿主冷观察回归 1 项红灯，0.14 秒（`target/git120-cold-observation-red.log`）；冷主题被缓存、Theme/Focus 发送 epoch0，真正启动和替换后未重发主题。代理在暂停前已写入最小宿主修复，尚未导出隔离候选、编译或跑绿；目录选择生产修复尚未开始。新增 Manager 子目录场景已落盘，尚未执行。以下运行中措辞属于暂停前迭代记录，恢复时以最新暂停现场为准。

当前冻结候选为 `target/git-plugin-final-120-tree/git-0.1.2.zip`，SHA-256 `b20aba4ff3408a39a5b2c466e63eec8a394843156fae156f52ef66717f0962d0`。同一 exact SDK 指纹 `d97c7f684cb7a4ab0d4eb643f0ed45d8a521483a592a239f0f7cb3277ff8f60f` 下访客 78/78、0 ignored、16.08 秒（`target/git120-guest-tree-final-sdk-tests.log`），构建记录 `target/git120-candidate-tree-final-build.log`。源码档案 `target/git-plugin-final-120-tree/git-for-nanobug-0.1.2-source.zip` 的 SHA-256 为 `c1f79ed9c81c20cef668da4448d3e89c4cab8192f6221707d273750a01ab156d`，排除所有 target、SDK 缓存及 .git；仅为本地候选交付。

同一 B20 包新增四项实际 Manager 回归 4/4、122.76 秒（`target/git120-tree-and-boundaries-green.log`）：两棵树同场景并发分页、HTTP 认证失败、停滞 Fetch/Pull 的真实连接取消与资源退出、非 UTF-8／大于 256 KiB 多文件冲突及重开/外部中止/旧回调拒绝。Windows 取消的 `ConnectionReset` 与 EOF 同为连接确已关闭的证据，超时不算通过。其余 28 项同包测试正在执行，不能提前计为通过。

审查固定点 `6b8433ed8cc04d9179c9a74204943facb10de883...4ffc8cbdc57ceda96b92cd3a516a46573efa744a` 的完整三个提交差异非空：Standards 0 项硬性违规、0 项判断类问题（最高无）；Spec a/b/c 各 0 项（最高无）。期间修复确认逻辑重复的 P3，并统一待确认入口；实际包暴露的纯 TreeRequest 回复重发视图缺陷已先红后绿。此静态结论与四项通过均不代替最终原生门禁。

生产原生 B20 smoke 于 96.88 秒失败（`target/git120-native-b20/dark-zh-CN-100/native.log`）：真实展开 `src` 后等待 `a-review.txt` 超时，树快照保持 expanded 而子页 loading，刷新持续推进场景，仍需排查选择/分页事件顺序。场景失败在退出前独立保存；生产项目关闭、两帧原生输入退休及主窗关闭均成功，本轮未出现泄漏。但本轮未到 IME，不能据此证明编辑后的退出问题已修。真实截图及 evidence 均在同目录，失败截图不计 AC5 通过。另有冷启动 Theme/Focus 观察被发送至未启动实例的通用宿主问题正在按测试先行修复。

以下保留初始 0.1.2 与后续迭代历史；不能将不同包摘要的结果合成最终全通过。

#119 的已执行结果保留。#120 新访客候选为 `target/git-plugin-final/git-0.1.2.zip`，SHA-256 `a8751d9eb948806b795d657c5d806788d8c7ba45452c3b7bbafe884f4a57acb1`，协议 7、SDK 0.2.0、`ui.code_editor >=1.1,<2`；原生辅助程序仍为 0.1.0，包内声明摘要 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。这是本地候选，未发布 Release 或登记市场。

配套宿主从隔离源码 `target/git-ticket119-source/` 重建，内嵌 SDK 缓存指纹为 `d97c7f684cb7a4ab0d4eb643f0ed45d8a521483a592a239f0f7cb3277ff8f60f`。实际命令为 `target/debug/editor-app.exe --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib --target-dir C:/Projects/RustProjects/Git-for-Nanobug/target/final-sdk-tests -- --nocapture --test-threads=1`，74/74、0 ignored，15.73 秒（`target/git120-guest-exact-sdk-tests.log`）。`--plugin-package` 同时构建 WASM 和声明的 native helper，日志 `target/git120-candidate-build.log`；没有借用含其他任务未提交契约的 SDK。

历史回归先在旧 0.1.1 包复现：滚到已加载旧页后，刷新缩短列表而保留越界 offset，导致公开 Manager 拒绝整个历史场景，错误为 `Invalid canvas typography or scroll range`（`target/git120-package-history-red.log`）。修复按已加载数量逐页重采样并原子发布，去重后有限补齐；持续变化或失败保留完整旧结果，真缩减/空结果夹紧滚动，独立历史详情保留不可变提交身份。自动刷新从完成时起冷却，显式焦点/保存仍可立即刷新。

脏缓冲回归先在旧包复现旧批准直接执行和等待阶段不能停止（`target/git120-package-guards-red.log`）。修复将被阻挡命令保留为重新审查请求：普通操作重采样 Git 状态和完整影响，仍需新的明确确认；其他流程撤销批准并从原入口重选。取消撤销尚未启动的计划和意图；旧 `Completed`/`Cancelled` 回执只负责原请求清理，不能借用新请求。等待原生检查时停止按钮保持可用；相关 busy 控件不再由后续 `.disabled(...)` 覆盖。

新同包 Manager 回归首次 4/4、115.86 秒（`target/git120-package-workbench-green.log`）：385 提交分页及真实 Timer/Focus/DidSave/新增提交；首提交、普通提交及三父提交只读来源；保存后重建影响与第二次确认；取消后旧回执不得批准新操作。此后继续加强两个终态的资源清理，并追加远程、linked worktree、高级操作和恢复场景，最终结果尚待汇总，不能累计为全量已通过。

Windows 原生验收已建立真实 GPUI/D3D11 子进程、私有配置/仓库和生产 Worker。首版 readiness 错把存在但禁用的 Refresh 视为可用，旧包与 0.1.2 均明确失败，日志 `target/git120-native-build-smoke.log`、`target/git120-native-v012-smoke.log`。夹具已改为同时检查公开节点、真实 Base 按钮和宿主场景 revision；完整主界面/历史/冲突场景及八种主题/语言/DPI 组合正在验证。此阶段没有合格生产截图，不宣称 AC5 完成。

访客源码无 `.git` 或远程。为正式审查建立仅本地 QA 快照 `target/git120-guest-review-source/`：固定点 `6b8433ed8cc04d9179c9a74204943facb10de883`，本轮初始提交 `f4a95082f229de99c03c6eae98812fdff37c352d`。这不等于插件源码已推送，最终源码档案、双轴审查和宿主交付仍在进行。

## 工单 01 最终验收与审查（2026-10-16）

交付提交 `09510cc58a8b434b815dfa5f9a7a3207f1221a55` 已普通推送到 `origin/codex/git-plugin-workbench` 并核对远端 OID。GitHub 连接器写议题返回 403，按既有授权使用同一仓库的 Git 凭据调用 REST 关闭 #119，再通过连接器读回 `closed / completed`；凭据只在进程内存使用，没有写入请求文件、日志或议题。#120 的唯一原生前置 #119 已完成，没有改动父设计议题。

源码审查为 `git diff 1379f58df76622bdf4d6a7c4167469f124d17a0b...8205c1e557198ea38c2640b09e50672cd1f2a316`，四个普通提交、119 文件的差异非空；验证代码 tree 为 `93f30dd44d7a988d7708ebe7e83818ff95a5875a`。最终同一 0.1.1 包五窗口 5/5，390.65 秒，`target/git-ticket119-tree-windows.log`，显式执行 ignored。包 SHA-256 仍为 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`，没有拿先前 runner 的窗口结果替代本次验收。

### Standards

0 项硬性违规、0 项判断类问题。Base 公开行为、原文档唯一状态、来源校验及资源清理保留；手势凭据集中管理，未发现新增职责分叉或重复抽象。

### Spec

分类 a/b/c 均为 0，最高严重级别无。此前 Tree 遗漏已关闭：撤销立即同步到 Tree，延迟分页在申请资源前检查原始手势凭据，行、键盘和菜单沿用同一门禁；图片 Up 匹配原 Down，框外延迟清理不能撤销新按压。两轴仅静态复审，真实行为证据另见本节与下表。

最终分别为 Standards 0 项（最高无）、Spec 0 项（最高无）。补白 50 项与通用 UI 71 项的覆盖合成方法见下表；侧边标签的选择、重排、左右停靠宽度调整、边框及刷新保留重命名另行 4/4 通过，0.05 秒（`target/git-ticket119-shared-side-tabs.log`）。所有本单已知失败已修复；生产外观矩阵和完整 Git 工作流由 #120 承接，不以本单完成替代整项功能交付。

## 2026-10-16 第三轮复审与 Tree／图片补齐

范围仍为 `1379f58df76622bdf4d6a7c4167469f124d17a0b...a5b3794d1dce931ce8c519457e26106d0c4deb7d`，三个普通提交的差异非空。两轴分别复审，读取已提交源码，不借用工作区其他主题变更。

### Standards

0 项硬性规范违规、0 项判断类问题。来源手势与焦点资源集中管理；Base 公开行为、原文档和既有权限保留，没有插件专属分支或 SDK 扩展。

### Spec

1 项 P2，实现缺陷（分类 c）：#119 AC4 的旧按压问题仍遗漏合法的 EditorAction Tree 子控件。其稳定 disclosure 身份能够将旧按压接到新 revision，资源 epoch 本身不能撤销 Base 按压。按钮、Canvas 和 SideTabs 已修路径未发现新问题；分类 a/b 均为 0。两轴分别为 Standards 0 项（最高无）、Spec 1 项（最高 P2）。

新增原生 Tree 回归通过冻结 `database-tree-0.1.1` 包与公开 Manager 准备资源及根页面，真实按下 disclosure，替换同来源 UI 场景，重绘后释放。修复前在 42.76 秒复现新场景展开及申请数据页（`target/git-ticket119-tree-red.log`）。第一次夹具没有准备根页面、找不到 main 行（`target/git-ticket119-tree-image-red.log`），属于夹具失败，不作为 Tree 行为红灯；同一日志中图片 Up 越场景已独立复现。

实现／测试 tree `93f30dd44d7a988d7708ebe7e83818ff95a5875a` 只增加以下修复：Tree 真实行和根元素的来源手势身份、键盘／菜单捕获及延迟请求凭据；状态继续使用同一个 TreeState，滚动和焦点保留。图片的持按 Move／Up 必须匹配真实 Down，框外释放延迟清理按独立序号撤销，不能清掉同场景的新按压。来源变化及无隐藏帧的撤回立即撤销；普通树和图片保持原公开行为。

六项真实输入回归 6/6 通过，47.92 秒（显式执行 ignored，`target/git-ticket119-tree-image-green.log`）；包含 Tree 展开／折叠、撤回再恢复，图片旧拖动／释放拒绝及新点击可用。后续宽过滤器使用 `--skip native_code_alignment_source_tree_cannot_expand_with_a_replaced_press`，复用同一 runner 刚通过的这一项，避免重复执行固定 WASM 夹具；不是跳过未验证项。

| Tree／图片修复候选验证 | 实际结果 | 日志（宿主 target/） |
| --- | --- | --- |
| 六项来源真实输入回归 | 6/6，47.92 秒；显式执行 ignored | `git-ticket119-tree-image-green.log` |
| 补白集合 | 49/49，14.72 秒，另复用上行同候选 Tree 1/1；50 项全部覆盖 | `git-ticket119-tree-alignment.log` |
| 通用插件 UI 集合 | 70/70，59.96 秒，另复用同候选 Tree 1/1；71 项全部覆盖 | `git-ticket119-tree-ui.log` |
| 格式检查 | `cargo fmt --check` 通过 | `git-ticket119-tree-image-fmt.log` |
| 非 UI workspace / workspace check | 77 组、276 通过、272 ignored；check 7.44 秒，未执行 ignored 不计通过 | `git-ticket119-tree-workspace.log`、`git-ticket119-tree-check.log` |
| 旧同步／可见性／DocumentInfo IME | 4/4（0.29 秒）、3/3（0.26 秒）、1/1（0.16 秒） | `git-ticket119-tree-legacy.log`、`git-ticket119-tree-visibility.log`、`git-ticket119-tree-document.log` |
| 实际包五窗口 | 5/5，390.65 秒；显式执行 ignored | `git-ticket119-tree-windows.log` |

正式复审已完成，结果见最上方最终验收节；准备推送并核对关闭。

## 2026-10-16 第二轮正式复审与手势修复

审查范围为固定点 `1379f58df76622bdf4d6a7c4167469f124d17a0b` 至 HEAD `85c9bccbabd2cfc440f06760b571aba0a714e84e`；`git diff <固定点>...HEAD` 和对应提交列表非空。两个轴分别由子代理完成。

### Standards

0 项硬性违规、0 项判断类问题。未发现新增公开能力绕过、上游复制或文档状态分叉；此静态结论不能代替真实输入回归。

### Spec

1 项 P2，实现缺陷（分类 c）：来源动作按钮的稳定 Base 元素身份保留了上一帧的按压，而回调读取新的 UI revision；旧按压可能激活新的场景，违反 #119 AC4。初步三栏测试因布局重建附带撤销按压而掩盖完整边界，补充同来源只换 UI 场景、旧版无补白代码组、来源 Canvas 和来源 SideTabs 四个真实鼠标回归，全部失败（`target/git-ticket119-action-widgets-red.log`，1.62 秒）。另外，同一次 update 内撤回再恢复也必须结束旧按压，不能依赖隐藏帧存在。

两轴合计分别为 Standards 0 项（无最高级别）、Spec 1 项（最高 P2）。修复只为来源动作建立绑定 UI revision、完整来源 token 及撤回世代的本地手势身份，再经过既有 IME、原生文本身份、精确来源和拥有者检查。输入、滚动和焦点保留；普通 Canvas 的跨帧拖动不变，没有更改协议、插件或包资产，也没有复制上游控件。

四个回归修复后 4/4 通过，2.57 秒，`target/git-ticket119-action-widgets-green.log`；第一次隔离编译缺少 `FluentBuilder` 导入，已补齐后重建。稳定的公开 debug selector 用于定位真实控件，不将测试绑到需要变化的私有 Base 元素 ID。实现与测试 tree 为 `a314887c742e328b862bfae45963c52b2a0cb5a9`（不含本段文档刷新），仅导出本任务增量；没有借入其他任务的未提交源码。

| 手势修复候选验证 | 实际结果 | 日志（宿主 target/） |
| --- | --- | --- |
| 四项来源动作真实鼠标回归 | 4/4，2.57 秒；显式执行 ignored | `git-ticket119-action-widgets-green.log` |
| `align --include-ignored --test-threads=1` | 48/48，13.40 秒 | `git-ticket119-gesture-alignment.log` |
| `ui::plugin:: --include-ignored --test-threads=1` | 69/69，57.41 秒；含普通 Canvas 跨帧拖动 | `git-ticket119-gesture-ui.log` |
| 旧语义同步 / 可见性 / 原生 DocumentInfo IME | 4/4、3/3、1/1 | `git-ticket119-gesture-legacy.log`、`git-ticket119-gesture-visibility.log`、`git-ticket119-gesture-document.log` |
| `cargo fmt --check` | 通过 | `git-ticket119-gesture-fmt.log` |
| 非 UI workspace / workspace check | 77 组、276 通过、272 ignored；check 通过，6.85 秒。未执行 ignored 不计通过 | `git-ticket119-gesture-workspace.log`、`git-ticket119-gesture-check.log` |
| 同摘要 0.1.1 实际包五窗口 | 5/5，391.06 秒；显式执行 ignored | `git-ticket119-gesture-windows.log` |

本轮仅修改宿主私有手势适配与真实回归，未改变插件候选摘要。正式复审、窗口复验和推送关闭尚未完成。

## 2026-10-16 恢复：审查缺陷修复

当前实现候选 tree 为 `9df3a9a611c18a0c88f9fc9542d228b9743fe0c7`，由同一临时 index 导出后验证。实际包仍为下文同摘要 0.1.1 ZIP；本轮未改访客、协议或包资产。Cargo 仅显式使用上游已经锁定的 `ropey =2.0.0-beta.1`，没有升级依赖。

- IME：`NativeTextIdentity` 只保存不可变 COW 根，通过上游公开 O(1) instance 比较检测原生文本变化，不设按大小/长度放行的权限捷径。TextTab 的 notify-only 文本变化推进插件能力版本，不改变 dirty/Undo；普通 Change 保持原职责。完整来源 token 改变时才采样动作身份，同版本重发不能重新授权。所有边缘控件的真实事件检查当前 Window 的 marked range、原生身份和现有精确来源权威；Tree 先验权再申请分页，测量回执仍表示实际发送。
- 三栏：同一个 pivot 间隙内，两侧新增行使用现有 PairRows 匹配并归入共享比较预算。没有复制 LCS、增加新预算或修改源文本；两栏不增加比较。新的真实三栏回归检查共同新增行、后续锚点、EOF、原文和选择，并在窗口重测后复验。
- 宿主 DocumentInfo 回归先在 0.13 秒复现预编辑未推进版本，修复后 0.16 秒通过。新按钮回归在输入发生的同一次 update 内直接检查事件出口，覆盖等长候选重发、仅 unmark 后旧来源拒绝和新版本恢复真实点击。首次后续断言因在旧锚点前插入 UTF-8 使旧偏移落在字符内部而没有可绘制矩形；夹具改在仍可见的 row03 输入，保留 row02 原生禁用按钮、宽度和真实事件断言，没有删除权限检查。

| 隔离候选验证 | 实际结果 | 日志（宿主 target/） |
| --- | --- | --- |
| 当前有效场景 → IME → 同版本重发 → unmark → 新版本点击 | 1/1，0.77 秒；显式执行 ignored | `git-ticket119-preedit-authority.log` |
| `align --include-ignored --test-threads=1` | 44/44，9.31 秒；包含新增两项原生回归 | `git-ticket119-alignment-review-final.log` |
| 原生 DocumentInfo 的预编辑/等长替换/取消/提交/Undo/非文本通知 | 1/1，0.16 秒 | `git-ticket119-native-capability-final.log` |
| `native_code_sync --test-threads=1` | 4/4，0.29 秒 | `git-ticket119-legacy-review-final.log` |
| `panel_visibility --test-threads=1` | 3/3，0.27 秒 | `git-ticket119-visibility-review-final.log` |
| `ui::plugin:: --include-ignored --test-threads=1` | 65/65，49.62 秒；通用控件、Canvas、输入和集合交互 | `git-ticket119-plugin-ui-review-final.log` |
| 实际包 5 项原生窗口 | 同摘要 0.1.1 包 5/5，373.79 秒；显式执行 ignored | `git-ticket119-review-windows.log` |
| fmt / 非 UI workspace / workspace check | 全通过；非 UI 为 77 组、276 通过、272 ignored，未执行的 ignored 不计通过；check 12.80 秒 | `git-ticket119-review-fmt.log`、`git-ticket119-review-workspace.log`、`git-ticket119-review-check.log` |

使用本轮构建输出的 `target/debug/deps/editor_app-34d8e9e5735919f6.exe`，不是此前 runner；先执行 `cargo test -p editor-app --bin editor-app --no-run --target-dir .. -j1`。红灯、第一次补充断言失败和后续绿灯分别保留；此节只列当前实际结果，正式复审和推送关闭尚未完成。

## 正式审查与再次暂停（2026-10-15 历史）

审查范围为固定点 `1379f58df76622bdf4d6a7c4167469f124d17a0b` 至本地 HEAD `81d6221a7f6daff290b95bff992ecc039b119c5d`，提交列表和 diff 均非空。

### Standards

0 项硬性违规，0 项判断类问题。静态检查确认通用公开能力、原文档与权限边界、上游公开接口组合、注释及中英文资源；此轴不代替原生行为验证。

### Spec

2 项 P2，均为实现缺陷（分类 c），无范围扩张（分类 b）：

- IME 预编辑仅 notify、不推进能力 revision，旧来源动作可能继续发出。新增 `native_code_alignment_current_actions_reject_preedit_and_unmarked_old_content` 已失败，0.24 秒；`target/git-ticket119-preedit-authority-red.log`。
- 同一 pivot 间隙内两侧共同新增行未进一步匹配，三栏显示行错位。新增 `common_peer_insertions_share_display_rows_without_pivot_anchors` 已失败；`target/git-ticket119-peer-insertion-red.log`。

合计 2 项，最高 P2；Standards 最高严重级别无，Spec 最高 P2。生产修复尚未开始。测试候选 tree 为 `8466f18870f667dbfd6af99dcf91cc4692218ca1`，相对 HEAD 仅增加三处测试文件、125 行。以下 39/39 与 5/5 等结果属于提交候选的既有测试集合，不能据此宣称追加回归后的完整候选已通过。

## 工单 01 当前候选门禁

实现 tree：`0f9ac5d2b88434191c7e82faaf286216c3afe9ef`（未包含下面文档记录的刷新）；比较点 `1379f58df76622bdf4d6a7c4167469f124d17a0b`。源码由 Git 临时 index 导出到 `target/git-ticket119-source/` 后验证，未借用其他任务的未提交文件。实际插件包仍为 0.1.1，SHA-256 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`。

| 导出目录中的命令 | 实际结果 | 日志（相对宿主仓库根） |
| --- | --- | --- |
| `cargo test -p editor-app aligned_editor_disjoint_native_viewports_use_independent_native_editing --target-dir .. -j1 -- --test-threads=1 --nocapture` | 1/1，0.26 秒；独立原生输入/Undo/真实滚轮和重新排列恢复通过 | `target/git-ticket119-disjoint-green.log` |
| `cargo test -p editor-app align --target-dir .. -j1 -- --include-ignored --test-threads=1` | 39/39，8.48 秒；13 项 ignored 已显式包含，未跳过 | `target/git-ticket119-alignment-final-gate.log` |
| `cargo test -p editor-app native_code_sync --target-dir .. -j1 -- --test-threads=1` | 4/4，0.28 秒 | `target/git-ticket119-legacy-final-gate.log` |
| `cargo test -p editor-app panel_visibility --target-dir .. -j1 -- --test-threads=1` | 3/3，0.27 秒 | `target/git-ticket119-visibility-final-gate.log` |
| `cargo test -p editor-app editor::comparison::diff --target-dir .. -j1 -- --test-threads=1` | 2/2，0.08 秒 | `target/git-ticket119-comparison-final-gate.log` |
| `cargo test -p editor-app extensions::windows_tests --target-dir .. -j1 -- --ignored --test-threads=1 --nocapture` | 同包 5/5，395.07 秒；显式执行 ignored | `target/git-ticket119-windows-final-gate.log` |
| `cargo fmt --check` | 通过 | `target/git-ticket119-final-fmt.log` |
| `cargo check --workspace --target-dir .. -j1` | 通过，3.26 秒 | `target/git-ticket119-final-check.log` |
| `cargo test --workspace --exclude editor-app --target-dir .. -j1` | 276 通过、272 ignored；复用同一非 UI 内容的隔离结果，此后只修改 UI 及文档 | `target/git-ticket119-isolated-workspace.log` |

定向 Cargo 命令也可从宿主根添加 `--manifest-path target/git-ticket119-source/Cargo.toml` 并改用 `--target-dir target`。该二进制完成一次构建后，重复过滤词直接运行 `target/debug/deps/editor_app-d4e1859b30e4cb6a.exe`；上表给出等价 Cargo 入口，日志不伪装为每项都重新编译。此阶段的自动行为与门禁通过，已形成普通本地提交；随后正式审查发现上述两个 P2，修复、复审、推送和 issue 关闭尚未完成。

## 恢复实施与新工单（历史过程）

以下保留每轮失败、修复和通过记录；当前状态以上表为准，不能把中间候选或旧包的通过数累计为最终结果。

用户已批准[剩余工作场景拆分](../tickets/git-plugin-remaining/README.md)。已发布 [#119](https://github.com/Nanobug-Editor/Editor/issues/119) 和 [#120](https://github.com/Nanobug-Editor/Editor/issues/120)，读回 #120 被 #119 阻塞，标签均为 `ready-for-agent`。连接器创建议题返回 403、未创建议题；随后通过仓库已有 Git 凭据调用 REST 完成发布和原生依赖核对，凭据未写入文件或输出。

0.1.1 候选（SHA-256 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`）的 55 项访客与同包 17 项真实 Git 测试保留通过证据，见暂停现场和 `target/git-alignment-guest-tests.log`、`target/git-alignment-package-tests.log`。本轮未改访客；不借用旧 0.1.0 的窗口结果证明 0.1.1 已通过。

首次恢复修复将绘制期的父实体权威查询改为已准备的来源元数据；事件分发仍精确校验身份和 revision。补白 34/34、旧同步 3/3 复验通过，日志 `target/git-ticket119-alignment.log`、`target/git-ticket119-legacy-sync.log`。新的 5 窗口套件为 2 通过、3 失败（312.29 秒，`target/git-ticket119-work-windows.log`）；初始化借用 panic 已消失，但出现真实窄列与字形对齐问题，未宣称窗口验收通过。

随后确认：测试需要推进每个窗口的实际下一帧回调；软换行公共行在窄列中占两行，原锚点出屏，改为可见公共行后暴露真实 8px 内边距差异。生产形状的快速原生夹具在关闭行号时可点击，启用真实行号后 0.48 秒稳定复现首块按钮缺失（`target/git-ticket119-gutter-red.log`）。分栏未采用声明的 220px 初始文件区，结果列被双操作栏和行号栏挤空；各列字形起点不一致还造成物理行偏差。初始尺寸和字形起点已修复，并由下面后续回归验证。

采用声明的初始分栏尺寸并统一实际字形起点后，当时工作区的补白原生回归 37/37（6.02 秒，`target/git-ticket119-alignment-final.log`）、旧同步 3/3、面板可见性 3/3 通过。真实包 5 窗口复验中，三项主比较通过，两项冲突窗口持续工作未返回，已终止该自有测试进程；没有把未结束项计为通过。有限绘制诊断显示三栏原点、布局、滚动和 epoch 已稳定，但窗口仍反复绘制；90 次绘制断点在 53.37 秒内捕捉到 `App::flush_effects` 的脏窗口刷新循环（`target/git-ticket119-isolated-merge-backtrace.log`）。跨窗口通知转发已由独立双窗口原生回归修复。

本轮非 UI workspace 测试已通过（`target/git-ticket119-workspace-tests.log`）。为隔离其他任务已有变更，另以临时 index 导出 Git 必要实现闭包；补齐可见性依赖后该隔离候选的 workspace check 通过（`target/git-ticket119-isolated-check.log`）。此为中间候选，最终分栏/对齐与测试定稿后还须刷新、复验和审查，未提交或推送。

### 工单 01：隔离候选的后续回归

固定比较点为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`。使用独立临时 index `target/git-ticket119.index` 导出 `target/git-ticket119-source/`，只含 Git 功能所需的公开机制、消费者、测试与文档；真实 index 及其他主题的工作区改动保留。下列 Cargo 命令均在导出目录执行，目标目录为 `..`，不能以含其他未提交实现的工作区通过结果代替候选验证。

- 跨窗口读取同一 `EditorState` 的原生回归在修复前 8.97 秒内达到 257 次绘制上限，修复后 2.67 秒通过。局部 authority paint fence 只抑制同一轮已绘制几何的额外观察者转发；原生通知和后续输入、IME、私有状态通知仍保留。日志：`target/git-ticket119-cross-window-red.log`、`target/git-ticket119-cross-window-green.log`。
- 该隔离候选补白相关 38/38（8.70 秒）、旧同步 3/3（0.16 秒）、可见性 3/3（0.29 秒）通过，日志为 `target/git-ticket119-isolated-align.log`、`target/git-ticket119-isolated-native_code_sync.log`、`target/git-ticket119-isolated-panel_visibility.log`。之后追加旧版父实体借用与零可视交集回归，最终套件尚待重新汇总。
- `cargo fmt --check`、`cargo test --workspace --exclude editor-app --target-dir .. -j1`、`cargo check --workspace --target-dir .. -j1` 通过，分别见 `target/git-ticket119-isolated-fmt.log`、`target/git-ticket119-isolated-workspace.log`、`target/git-ticket119-isolated-check-final.log`。非 UI 测试共 276 通过、272 ignored；ignored 未执行，不能计入通过。后续只修改 UI 路径，仍须复查最终格式与编译。
- 新的旧版同步回归在 0.05 秒复现绘制期读取正在更新的父实体。几何测量和动画请求保留在绘制期，权威校验与滚动写入延后到外层更新结束；校验包含场景、成员代次和精确来源版本。最终 4/4 通过（0.33 秒，`target/git-ticket119-legacy-owner-final.log`）。两次中间实现触发绘制阶段断言，均已修正，未作为通过证据。
- 实际包窗口中间复验为 3 通过、2 项在原“连续八个静帧”条件下超时（`target/git-ticket119-isolated-windows.log`）。超时诊断中原生文档入口为空、场景稳定，仍有本地 Git 查询运行。夹具改为等待公开刷新按钮恢复可用，再要求两轮场景与文档入口稳定；未关闭定时器、放宽版本门禁或延长 30 秒等待上限。冲突逐块操作定向通过（100.74 秒，`target/git-ticket119-merge-ready-final.log`），随后同包全部 5 项通过，见当前门禁表。
- 零可视交集原生回归在 0.12 秒复现错误的 `Ready`，使两栏源码片段均被裁空（`target/git-ticket119-disjoint-red.log`）。改为独立原生降级后，真实滚轮暴露覆盖层 auto inset 落在正常流编辑器下方，引起错误坐标及反复重测；新增原生视口断言在 0.26 秒复现（`target/git-ticket119-disjoint-diagnostic.log`）。两处 full-pane canvas 固定左上角后，输入/Undo、独立滚轮、静止收敛和重新排列恢复全部通过（`target/git-ticket119-disjoint-green.log`）。不完整测量仍保持 Measuring，仅全部有效但没有共同高度的布局降级。
- 新等待器首次完整窗口回归在插件禁用后错误等待已撤回的刷新控件，场景为空、实例和进程均已退休。该自有测试在定位后终止，日志保留为 `target/git-ticket119-windows-withdrawal-fixture-failure.log`，未计通过；等待器现将完全退役视为静止，同时对活动实例继续检查公开 readiness、稳定场景、空原生入口和进程结束。

## 工作范围与现场

- 宿主：`C:/Projects/RustProjects/Editor`，开始时已有大量其他任务未提交改动。只将下面列出的 Git 插件及通用接缝计入本任务，不把整个工作区 diff 视为本次成果。
- 插件：`C:/Projects/RustProjects/Git-for-Nanobug`，独立源码目录，当前没有 `.git` 或远程。宿主提交不等于插件源码已推送；本批候选交付不创建远程插件仓库或发布版本。
- 补白候选：`target/git-plugin-final/git-0.1.1.zip`，SHA-256 `7336d5d39fecc18f2faa8972916fa739978f7e96da96116bb6b34fe5d0363e48`。清单与访客版本同步升至 0.1.1，要求 `ui.code_editor >=1.1,<2`。通过重建宿主的 `--plugin-package` 直接构建，实际包验证进行中；包含摘要校验的 Windows 原生变基辅助程序，SDK 由宿主缓存注入。
- 旧基线：`target/git-plugin-final/git-0.1.0.zip`，SHA-256 `bae3594de6cd2d167a74144e3bd5d912b0e5ded7377979f009af767e1dee35f5`。先前的 5 项原生窗口结果属于此基线。此前 `git-plugin-candidate` / `git-plugin-next` / `git-plugin-delivery` 为迭代快照，分开记录结果。候选包需要配套宿主；不声称兼容未包含新能力的旧宿主。
- 审查固定点为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`，交付前审查 `git diff 1379f58df76622bdf4d6a7c4167469f124d17a0b...HEAD` 及对应提交列表。此前针对未提交现场的调查只属预检查，不算正式 Standards/Spec 审查通过。

## 实现与验证接缝

宿主新增的是面向全部插件的能力，不按 Git 插件 ID 分支：

| 能力 | 契约和实现 |
| --- | --- |
| Pipe EOF | `process` 1.8 的 `CloseStdin`；先排空已接受字节，再关闭输入管道，不终止子进程；PTY 拒绝 |
| 独立窗口 | `ui.windows` 1.0；声明 window 面板；借用唯一原生文档，关闭窗口保留草稿和 Undo，禁用回收窗口资源 |
| 持续比较 | `editor.virtual` / `editor.diff` 1.1；隐藏快照、文件绑定预览／固定标签、持续重算、统一视图、差异导航和滚动联动 |
| 原生代码列 | `ui.code_editor` 1.1；1.0 默认行为保留，1.1 通过 `Document.code_alignment` 显式启用本地补白；来源不成为第二份可编辑文档 |
| 块边缘操作 | `editor.actions` 1.0；现有文档几何定位按钮，操作发出时再次校验来源 revision，保留原生编辑状态 |
| 列联动 | 1.0 保留有界语义滚动；1.1 同组以不可变行槽与实际原生软换行几何形成共享显示视口、独立横向滚动，补白不进入文本；超过预算明确独立滚动 |
| 可见性 | `ui.visibility` 1.0；真实窗口、dock 折叠和 tab 选择采样；只通知已运行实例，不以采样启动插件 |

插件负责暂存／分支树、拓扑与分页历史、index 提交、精确 patch 暂存、仓库与 linked worktree 识别、网络操作、stash、高级操作预览、交互式变基计划和三方合并策略。命令使用可执行文件与参数数组；诊断遮蔽 URL userinfo；不保存凭据，不使用用户输入拼接 Shell。

## 历史 0.1.0 阶段的针对性验证

以下保留补白接入之前的实际执行记录，窗口结果属于 0.1.0，不能用于证明当前 0.1.1 窗口通过。后续修改必须复验相关项；被 ignored 的测试不计入普通测试通过数。0.1.1 的 55/17 项及本轮结果见上方恢复实施记录。

| 命令／入口 | 当前结果与范围 |
| --- | --- |
| `cargo test -p plugin-protocol workbench` | 6 通过；代码列／动作、补白 1.1 独立协商和序列化验证 |
| `cargo test -p plugin-runtime panel_visibility` | 3 通过；所有者、能力和冷实例边界 |
| `cargo test -p editor-app native_code_sync -- --test-threads=1` | 3 通过；真实三栏滚轮、来源选择／文本不变、场景撤销取消定位 |
| `cargo test -p editor-app panel_visibility -- --test-threads=1` | 3 通过；实际 dock 折叠／同组选择与隐藏 fallback、原生 close／重新显示／hide |
| `cargo test -p editor-app extensions::windows_tests -- --ignored --test-threads=1` | 旧 0.1.0 ZIP 的 5 项全部通过（335.11s，`target/git-work-windows-final.log`），涵盖下面三组行为；显式执行 ignored，并非普通 workspace 测试计数 |
| 主比较窗口的 3 个回归 | 原生输入／撤销、双击固定、隐藏快照、统一视图选择／只读／导航、40 次刷新与回收、长软换行双向滚动 |
| 独立窗口原始回归 | 真实 9 宽标记、选块与 Undo、中文 emoji、主窗口其他文件下 Ctrl+S、磁盘冲突明确覆盖、隐藏及隐藏后手动刷新保留草稿与 MERGE_HEAD |
| 新增独立窗口合并回归 | 真实按钮点击验证 base 初始化、逐块接受、Undo 保留非冲突修改、非冲突单独应用、手工编辑后明确确认和保存暂存；主窗口选中另一文件时仍操作正确原生文档 |
| 独立访客单元测试 | 最新 55 通过；含真实 Git LF index／CRLF 多块合并、SHA-1/SHA-256 intent-to-add、硬重置阻挡路径与禁止隐式 autostash，以及普通／linked worktree × merge／apply 两种 rebase 后端连续同文件冲突的身份与结束清理 |
| 实际 WASM 包 → Manager → Git | 16 项初轮为 14 通过、2 失败（890.25s，`target/git-package-test-final.log`）；通过项包括大提交说明与 updateRefs 保护的交互式变基、部分暂存／重命名／未出生 HEAD、二进制 add/add、7 项真实仓库／Stash／Pull、3 项 index 边界。失败的两项变基恢复已定位并修复临时只读 base 栏范围错误，定向复验 2 通过（102.31s，`target/git-rebase-resume-final.log`） |
| 新增远程变化 force-with-lease 包回归 | 1 通过（47.90s，`target/git-lease-final.log`）；预览后远程推进，确认时旧 lease 被拒绝，没有覆盖新的远程提交 |

补充修复包括：Edited 回执不能将较新文档版本降回旧版本；快照在连续输入中失效时重新枚举同一文档身份并读回最新版本，关闭／重新打开不借用旧身份；冲突文件建议按完整状态与稳定操作身份统一发布，rebase 步骤使用稳定 orig-head／onto 身份，未知／准备中暂停写入口，新操作撤销旧建议但保留宿主文档和 Undo。已解决文件仍可核对，不再启用旧 base／块／整版本写入。linked worktree 的 rebase 元数据通过经过校验的 `worktrees/<id>/...` Git 命名空间读取；成功结束后 Git 遗留的 `REBASE_HEAD` 不单独表示仍有进行中的 rebase。

新交互式变基包回归的早期缺失视图索引，不是 helper 的继续协议失败：临时中栏用 base 文本却错配 right ranges，导致公开 UI 校验撤回实例。新 guest 回归已红→绿。定向实际包回归覆盖原生文档请求 ApplyEdits／保存／暂存后继续，以及恢复计划被插入 exec 时拒绝执行并保留草稿与 Git 状态。上述两项与新增 lease 用例使用最后一次 linked 元数据修复前的 final 快照（SHA-256 `7423c1e7471d1abf12f43a72087806f7da4f55d13171d9f944c908322c691522`）；不能把不同快照的分批结果冒充同一 ZIP 全量 17 项通过。此历史阶段的 55 项访客和 5 项原生窗口通过；后续 0.1.1 同包 17 项通过另行记录。

可见性初次 fixture 假设面板默认在同一 tab 组而失败。Nanobug 实际默认拆分 leaf；修正 fixture 通过公开 DockArea API 明确合并测试 peers 后，3 项通过。没有为迁就测试改变产品布局。

合并窗口夹具此前遗漏生产 actor 的文档事件转发，且仅按 Git 进程数判定空闲，可能在原生场景追上 Manager 之前点击旧按钮。夹具现将真实文档事件送回 Manager，并要求无文档事件且场景 revision 连续稳定后才结束泵送；点击前核对面板与 Manager revision。按钮点击定位到实际 Base 控件，避免点中包装容器的空白。未直接注入业务事件，也未降低生产来源版本校验。

## 补白实现与验证进展

用户允许在本地 `ui/` 新建自定义组件，但要求不复制或 fork gpui-kit、gpui-base 等上游。此授权已采纳，不再沿用此前“补白延后”的范围结论。

当前锁定的 gpui-base 0.7.0 未公开直接修改 display map 的接口。现已验证另一条公开组合路径：本地组件裁剪原生显示片段，最终原生挂载保留原 `EditorState` 的输入、选择、IME 与 Undo；根级 RootPlugin 适配真实指针事件，InputHandler 只适配几何。补白只进入不可变行映射与像素布局，不写入源文本，不建立第二份可变 Rope 或撤销栈。

一次性 public-API 试验位于本地忽略目录 `target/git-spacing-probe`，没有复制上游实现。运行：

```powershell
cargo test --manifest-path target/git-spacing-probe/Cargo.toml --target-dir target --offline -- --nocapture --test-threads=1
```

实际 3 通过（包含 2 个原生布局试验）：

1. Inline token 拒绝空源范围和换行；CodeEditor 模式丢弃 token，不能借此产生显示补白。
2. 同一 EditorState 分段裁剪、重复挂载后，只有最后一处 input/caret/IME 几何生效；试验测得前一挂载被后一挂载覆盖。不能作为可编辑补白实现。
3. 额外 compile-fail feature `missing-display-seam` 访问 `EditorState.display_map` 得到预期 E0616 私有字段错误；仅说明此路径不公开。

这些早期试验仅否定未经输入适配的简单裁剪，不能据此认定公开组合不可行。未接入的 `src/single_mount.rs` 未运行，不计入实测。

后续 `forward_mouse.rs`、`aligned_bridge.rs` 和 `aligned_scroll.rs` 已通过公开 API 验证：延迟原生指针转发、根级恢复事件隔离、真实光标及 IME 候选框、中文 emoji 预编辑／提交／Undo、跨补白拖选、3 处不同高度补白（含大于视口的空白）、第 22 行点击、长软换行、滚轮与翻页。`aligned_scroll` 最近 2 项通过（`target/git-spacing-scroll-probe.log`），另验证稳定绘制后没有持续通知或动画帧。

生产实现位于 `ui/controls/aligned_rows`、`aligned_pointer`、`aligned_editor`，接入主比较和插件代码组。`ui.code_editor` 1.1 新增 `Document.code_alignment` 显式启用，1.0 默认行为保留；4M 单元由同场景组共享，输入时重算归还旧预算，不为每个组另发 4M。布局受每源 1 MiB／2001 行、测量高度上限约束，失败明确独立滚动；搜索或代码操作打开时整组暂停并显示提示。生产接入后的全量验证仍在进行，下面旧候选包结果不包含此能力。

## 历史门禁与仍有效的交付限制

此前 `cargo test --workspace --exclude editor-app` 首次遇到 Windows `msvcrt.lib` 链接失败；同环境以 `-j1` 重试全量通过。补白接入之前的阶段门禁 `cargo test --workspace --exclude editor-app -j1`、`cargo check --workspace`、`cargo fmt --check`、`git diff --check` 均通过；不代表本轮隔离候选的最终门禁。普通 workspace 测试未执行 ignored 项。原生辅助程序 `cargo test --manifest-path ../Git-for-Nanobug/native/Cargo.toml` 编译通过但包含 0 项测试，不把它当成行为覆盖；辅助程序行为通过实际包变基用例验证。

当前尚不能宣称全部功能完成：补白生产接入后的集成复验尚未结束。真实 WASM 包验证按上述快照分批执行，旧候选 ZIP 的 5 项原生比较／合并测试全部通过；没有将分批结果伪装为新 ZIP 的全量包测试。当前没有生产界面截图验收；GPUI 测试覆盖真实布局和输入，但不替代维护者对深浅主题、缩放和整体外观的验收。macOS/Linux 未构建或实测，原生变基资产当前声明 Windows x86-64。

外部 Git 可并发，内部队列并非整个仓库的独占事务。尤其 `stash drop` 的 CLI 没有 reflog CAS：执行前核对 immutable stash OID 与 selector，可以拒绝已观察到的变化，但不能声称消除核对与删除之间所有外部竞争。
