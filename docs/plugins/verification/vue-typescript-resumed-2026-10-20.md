# Vue / TypeScript 剩余场景恢复记录

> 最新：恢复后的 Worker、悬浮、配色、键盘、普通桌面及最终支持双包已完成技术验收。下文保留逐时点过程和旧失败；当前结论、精确支持范围及 Git／工单实际回执见[交付记录](vue-typescript-remaining-delivery-2026-10-20.md)。

日期：2026-10-20，Asia/Shanghai。15:00 按用户“继续执行”恢复新 #122–#126；#121 既有交付保留，旧 #108–#118 与父 #107 不执行或改写，不创建 Release。沿用已批准的规格、六场景工单、公开测试接缝、普通提交／推送／对应工单关闭及子代理授权。

## 恢复基线

[05:08 暂停检查点](vue-typescript-paused-2026-10-20-0508.md)及其字节账本继续保留。恢复时核对 H / T / V 的分支、HEAD、porcelain 状态及全部 265 个变更路径，均与保存时一致；没有重新应用旧 stash 或修改用户原树。远端读回 #121 closed，#122–#126 open。

继续使用 H `C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor` 的集成分支 `codex/vue-typescript-remaining`；Cargo metadata 确认 workspace_root 为 H、target_directory 为 H/target。两个插件源、旧归档与候选输入保持原身份。

## 当前执行

先定位 Worker 取消准备 ignored 回归：原始失败 129.11s 只有通用 `wait_for:212`，不能确定具体阶段。仅在测试辅助函数补 `track_caller` 和失败时的发布状态摘要，保留谓词、60s 预算、固定 capability `.17.1` 及完整权限／旧版本／checked shutdown 断言。当前构建并显式执行原测试；未取得结果前不记为通过。

只读代理同时复核最终验收缺口与候选打包输入，不运行 Cargo、Node 或原生窗口。根代理统一串行调度构建及实际进程测试，避免干扰冷准备与原生验收。

已有完整 S4 PASS 280.28s、S5 外层全链 PASS 164.27s、非 UI 319 PASS / 0 FAIL / 327 ignored，以及 fmt / check / 正常宿主构建通过均按原源码／产物身份保留。最后双包、普通桌面、支持元数据及三仓交付仍未完成。

实施与旧失败见[原日志](vue-typescript-remaining-2026-10-19.md)，最终交付状态见[交付记录](vue-typescript-remaining-delivery-2026-10-20.md)。

## Worker 失败定位与测试入口更正

观测版 no-run **PASS 1m48s**，Cargo 实际产物为 `target/debug/deps/editor_app-81b56278c415338a.exe`。第一次冻结误取无 hash 的 `editor_app.exe`，其摘要恰为 normal `996bf4…8448`，不是测试程序；已按完整路径和参数核对后停止本轮 PID 74016，未将空日志或该调用记为测试结果。随后使用 Cargo 明确产物，先 `--exact … --list` 确认恰好一个测试，冻结为 `target/s6-resume-worker-observation-corrected/deps/editor_app.exe`，SHA-256 `f82d76df413af046d01f853b382f82aa6ed94a174fd4eef00a258824a527af7f`。

实际原入口 **FAIL 127.43s**（`target/s6-resume-worker-observed-failure-corrected.log`），明确落在最后的撤权命令屏障行 666。此前视图为空和准备结束均已满足；失败时 ready=true、welcome 视图再次出现、安装 installed=false、状态仍为 `Dependency preparation cancelled`。因此不是初始编译／视图等待，也不是候选提交已成功。

三项假设依次核查：旧 queued grant 重放、准备结束恢复旧信任、命令准入未遵守当前信任。真实入口 `Extensions::set_workspace_trusted` 先将 `worker.trusted` 发布为当前状态，再发 `Work::SetTrust`；旧测试仅发消息，保留启动时 true。runner 每次 `select_workspace` 用共享 authority，故下一圈会按错误的 true 重新选择，之后 scope-read 可重建 view。该 prepared Err 不进入 commit，准备结构也不保存旧信任；preparation 在场时原有 retain 已剔除旧 grant，没有证据认定这些生产分支缺陷。

最小更正只在原测试的 true/false 通知前各 store 当前信任（Release），使其匹配现行宿主入口；原包、60s/10ms、旧 grant、撤权、安装失败、旧版本与 checked shutdown 断言保留。当前源码 SHA-256 `46d4d155ae45d3396ff95ae5013de727710699eb1110f602e73700d37c605577`，尚待真实运行。只读诊断依据见 `target/s6-resume-worker-observation.md`，不把原失配测试当产品安全缺陷，不为它增加宿主特殊分支。

随后从 Cargo JSON 的唯一 `profile.test=true` executable 冻结 `target/s6-resume-worker-handoff/deps/editor_app.exe`，SHA-256 `d69c12703872458ea38f971c19011d5b78033eeadaee617a2087d80d17e3bc94`，实际源／程序关系保存在 `target/s6-resume-worker-handoff-inputs.json`。普通 Worker 过滤 **6 PASS / 0 FAIL / 8 ignored、1.63s**；显式原取消准备用例 **1 PASS / 0 FAIL / 0 ignored、67.94s**，`target/s6-resume-worker-handoff-revocation.log`，原完整断言和 checked shutdown 均走完。旧失败日志仍保留，不能将它改称已通过。正向更新入口另外运行，不由撤权结果代替。

Spec 与 Standards 对上述 `46d4…5577` 的增量各 **0 项可行动发现**，核对了真实 UI 交接、锁在 panic 前释放、原期限与完整断言；静态结论与实际通过分别记证。恢复期尚未改生产实现，原生产门禁继续按其身份有效。

## 正常宿主 SDK 与候选输入

已实际运行 normal `996bf4…8448` 的 `--export-plugin-sdk target/s6-resume-sdk-export`，exit0，**54 文件、0 Markdown**；全部与公开 SDK 缓存 `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb` 逐字节一致，收据为 `target/s6-resume-sdk-export-proof.json`。候选 73 文件和原 24 元数据再次核对未变。唯一 Cargo 通道正在按各自 source root／独立 managed target 串行打包；当前还不将候选构建冒称最终支持提升或桌面通过。

两个桌面候选随后均由同一 normal 公开 `--plugin-package` 实际 exit0，构建前 metadata 分别核对了快照根与独立 managed target，`CARGO_BUILD_JOBS=1`、离线、串行；插件未持有宿主源码或 SDK 副本：

| 桌面候选 | 完整归档位置 | SHA-256 |
| --- | --- | --- |
| TypeScript `.15` | `T/target/s6-desktop-candidate-015-package/typescript-0.1.15.zip` | `422074c65d63d89b2f8b060feb15ceaaffaf4b7a3c77b19f6c6c665ebdb39f43` |
| Vue `.12` | `V/target/s6-desktop-candidate-012-package/vue-0.1.12.zip` | `058cb1eec9dbef169aba8095c2e94c0f29935b845735d1c7488108e8a02e4a5e` |

release WASM 构建分别 1m07s / 1m08s；日志为 `target/s6-resume-{typescript,vue}-candidate-package.log`。包内负载闭合与源快照不变仍在独立逐字节复核；这两包保持 pending 支持元数据，与后续正式支持产物分开保留。

独立闭包复核随后实际 **PASS**：T 22 / V 39 个 ZIP entries 与各自 descriptor 精确闭合；没有意外负载，47/47 保留资源、11/11 Vue 模块、73/73 源文件含锁文件均不变。相较旧包只有 T 6 / V 8 项已允许的 WASM、版本与说明元数据变化，详见 `target/s6-resume-desktop-candidate-packages.{json,md}`。

当前双候选实际 mixed S1 **1 PASS / 0 FAIL / 0 ignored、95.97s**，`target/s6-resume-candidate-pair-native.log`；相同 `d69c…bc94` 测试程序，原始三个 Vue 项目、未保存文本、语义与引用及 checked shutdown 均完成。成功保留 profile `target/s6-native-ui/profiles/s1-verified-Im6Nju`。另一个显式 Worker 正向更新入口实际 **1 PASS / 0 FAIL / 0 ignored、67.84s**，`target/s6-resume-worker-handoff-live-update.log`；原取消准备失败已与正向路径分开闭合。最后 `cargo fmt --check` 通过（`target/s6-resume-after-handoff-fmt.log`）。

## 普通桌面执行中

为避免在插件源码树保存验收输入，已将同一原始 Vue 三项目的 27 个 fixture 文件逐字节复制到 `H/target/s6-desktop-workspace`，依赖以显式 junction 指向原只读依赖目录；额外两个小型配置项目共享 `shared.ts`，用于实际原生歧义选择与自动导入。此私有副本及新增 fixture 不进入插件源码、Package 或 SDK，复制身份见 `target/s6-desktop-workspace-inputs.json`。

正常桌面使用成功 S1 profile 和 normal `996bf4…8448`，启动收据为 `target/s6-desktop-launch.json`；它是另一次普通程序运行，不将 S1 的 TestAppContext 结果改称普通 Windows 桌面通过。仅在实际观察并操作后记录相应项目，当前尚未标记完成。

新副本出现工作区信任提示后未代替用户操作。随后核对成功 S1 profile 已保存原 Vue 根的匹配信任记录，改用原项目继续，未编辑信任数据。新增副本的授权问题因此不再阻塞；启动身份见 `target/s6-desktop-original-launch.json`。

## 真实暖缓存悬浮崩溃：RED 与修复中

normal `996bf4…8448` 在原项目正常恢复三个 Vue 页签。鼠标取得 `message` 悬浮后按 Ctrl+I，进程实际退出，记录 `target/s6-desktop-original-stderr.log`。复制同一次编译的匹配 PDB 并用 `RUST_BACKTRACE=full` 再次启动，仍实际复现；`target/s6-desktop-hover-red-stderr.log` 将调用链定位为 `hover_context_current` ← 同步缓存复用判断 ← `EditorApp::on_show_definition_details`。此时父 EditorApp 正持有更新租约，正缓存的关联上下文检查再次读取父实体，触发 GPUI 双重借用 panic。鼠标公共路径也会在父更新内调用同一 provider。

新增回归保留既有 mixed S1 的真实三个文件、源文本和 owner-bound providers，在语义请求暖成正缓存后，仅 Escape 清展示，再逐个实际按 Ctrl+I。未改生产的冻结程序 `target/s6-cached-hover-red/deps/editor_app.exe` SHA-256 `9b6a15d3df165d3262cf39093a821c3a6f1f4adf33141223154b472aadaa5ce7`，真实 **FAIL 116.40s / 0 PASS / 1 FAIL / 0 ignored**，同一 `cannot read editor_app::EditorApp while it is already being updated`；日志 `target/s6-cached-hover-red.log`，源码身份见 `target/s6-cached-hover-red-inputs.json`。旧 S1 95.97s PASS 不涵盖此新增回归。

当前修复把缓存选择及关联上下文读取一并推迟到返回任务的后续 UI 轮次，仍保留共享 worker、取消隔离、缓存失效后重新请求及完成时 revision／生命周期检查。回归同时改为原生字节光标，避免把含中文和 emoji 的 LSP UTF-16 列直接交给原生标量列接口；两种旧位置都在同一 `message` 内，实际修复不依赖该位移。正在构建 GREEN，后续还需关联源关闭／同路径重开回归及同一普通桌面入口复验；当前不提升支持状态。

修复后 Cargo no-run **PASS 2m01s**，实际测试程序冻结为 `target/s6-cached-hover-green/deps/editor_app.exe`，SHA-256 `b41758b2bf392457adbe74f5ce52cbbb3d7d78d569afe203cedb58402dc24454`；生产 hover 源码 `999c226e894732e868fc867a0b233d408637029869a0775df33cbf90444db790`，完整关系见 `target/s6-cached-hover-green-inputs.json`。相同候选双包 mixed S1 **1 PASS / 0 FAIL / 0 ignored、97.62s**，逐个 Ctrl+I 卡片及原有语义、引用、诊断、checked shutdown 全部走完；日志 `target/s6-cached-hover-green.log`，保留成功 profile `s1-verified-x2ZMVu`。普通 hover 过滤 **12 PASS / 0 FAIL / 0 ignored、2.75s**；Spec 与 Standards 对当前三文件各 0 项可行动发现，收据 `target/s6-cached-hover-{spec,standards}-review.md`。

第一次共享源刷新误选 `context-selection-fixture`，在 panel.opened 事件即 **FAIL 35.65s**（null Operation），尚未到 hover 断言；`target/s6-cached-hover-related-refresh.log` 保留。核对清单和 guest dispatch 后更正为独立 `project-context-fixture-0.1.1.zip`（`08cc6b165dd1f582f8ab586909c29f9ce995f740393d0e71714caaed1f44ebdd`），未改生产或测试绕过失败。相同 b417 程序实际 **1 PASS / 0 FAIL / 0 ignored、26.25s**，`target/s6-cached-hover-related-refresh-corrected.log`；记录 `target/s6-cached-hover-refresh-inputs.json`。它验证目标文本不变时关闭关联草稿会重新获取磁盘结果，同路径重开后再次取得最新草稿。原生 lsp_fixture 取自当前 profile 的直接构建产物，不由缓存返回假值。普通宿主现正重新构建，桌面复验仍待实际操作。
