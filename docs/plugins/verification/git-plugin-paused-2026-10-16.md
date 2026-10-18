# Git 插件暂停与恢复记录（2026-10-16）

用户于 2026-10-16 11:57（Asia/Shanghai）明确要求“暂停所有工单并保留进度”。本任务测试、原生应用及所有子代理已停止；不继续实现，不提交、推送、关闭议题或发布候选。已有 #119 完成状态保留，#120 暂停。下次需用户恢复后再执行后续步骤。

| 工单 | 保留进度（工作量估算） | 状态 |
| --- | --- | --- |
| [#119 / 01](https://github.com/Nanobug-Editor/Editor/issues/119) | `██████████` 100% | 已交付并读回 closed/completed；不是重新打开或回退 |
| [#120 / 02](https://github.com/Nanobug-Editor/Editor/issues/120) | `██████░░░░` 约 60% | 已暂停，仍未交付、未关闭 |

## 源码与隔离现场

- 宿主 `C:/Projects/RustProjects/Editor`，分支 `codex/git-plugin-workbench`，HEAD 与 origin 最后已核对的分支提交为 `2b9e1548a91925493ac89f397ff1cd8c0c0380e0`。本轮 #120 尚无普通提交。#119 交付提交 `09510cc58a8b434b815dfa5f9a7a3207f1221a55` 保留。
- 原工作区约 190 处其他主题改动继续保留，不得全量暂存。真实 index 本轮未改；本任务临时 index 为 `target/git-ticket119.index`，已包含 #120 的原生验收支架、实际包业务测试、冷观察测试及文档。停止时未验证的生产冷观察修复仍在原工作区，未合入该 index。
- 隔离源码 `target/git-ticket119-source/` 是实际 Cargo 输入，不包含其他主题的未提交实现。当前 App runner 为 `target/debug/deps/editor_app-b49bc269792187de.exe`，runtime runner 为 `target/debug/deps/git_plugin-bedae8640c39723e.exe`；恢复后以实际构建产物为准，不在 EXE 运行时重新链接。
- 混合文件修改前快照：`target/git120-before-cold-surface/extensions.rs`、`surface.rs`。两份冷观察生产增量需从这些快照隔离提取，禁止把原文件整份暂存。相对快照，extensions.rs 含测试 mod 两行及主题缓存/冷通知门禁；surface.rs 含冷环境返回与 focus callback 的 incarnation 检查。暂停时这些生产增量未编译、未验证。
- 外部访客 `C:/Projects/RustProjects/Git-for-Nanobug` 无 `.git`、无远程；源码修改仍在原位置。已新增目录选择失败回归但未修生产逻辑，因此当前源码比冻结包多一项测试（79 项），冻结包记录仍为 78 项。

## 冻结候选与证据

- 包：`target/git-plugin-final-120-tree/git-0.1.2.zip`，SHA-256 `b20aba4ff3408a39a5b2c466e63eec8a394843156fae156f52ef66717f0962d0`；协议 7、SDK 0.2.0、ui.code_editor 1.1。内嵌 exact SDK 指纹 `d97c7f684cb7a4ab0d4eb643f0ed45d8a521483a592a239f0f7cb3277ff8f60f`。
- 对应源码 ZIP：`target/git-plugin-final-120-tree/git-for-nanobug-0.1.2-source.zip`，SHA-256 `c1f79ed9c81c20cef668da4448d3e89c4cab8192f6221707d273750a01ab156d`。该 ZIP 不含暂停前最后新增的目录选择测试；不要误报为当前所有未提交源码。
- helper 仍为 0.1.0，摘要 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`。未复制 SDK 或上游项目，未发布 Release/市场登记。
- B20 访客 78/78、0 ignored、16.08 秒：`target/git120-guest-tree-final-sdk-tests.log`；构建 `target/git120-candidate-tree-final-build.log`。
- B20 定向实际包 4/4、122.76 秒：`target/git120-tree-and-boundaries-green.log`，覆盖两树并发分页、HTTP 失败、真实网络取消、非 UTF-8/大文本多文件冲突重开与旧回调拒绝。
- B20 其余 28 项：`target/git120-final-package-remaining.log`。暂停前 27 条 `... ok`；reset 模式仍在运行时停止，无完整摘要。该结果与上项合计为已结束 31 项通过、1 项未完成，不能报告完整 32 项通过。
- 访客正式双轴静态审查：固定点 `6b8433ed8cc04d9179c9a74204943facb10de883...4ffc8cbdc57ceda96b92cd3a516a46573efa744a` 三提交差异非空；Standards 硬性/判断均 0，Spec a/b/c 均 0，各轴最高无。此结论属于冻结包源码，不覆盖后来未执行的新增测试或后续修复。
- 隔离 `cargo fmt --check`、`cargo check --workspace` 此前通过（`target/git120-host-fmt.log`、`target/git120-workspace-check.log`）；不能套用于暂停前未验证的生产修复。#120 最终非 UI workspace、共享消费者、五窗口及原生矩阵门禁仍待完成。

## 仍未解决的真实红灯

1. 原生 B20 smoke 96.88 秒失败于 `a-review.txt timed out`：目录 src 已展开，但子页 loading。`target/git120-native-b20/dark-zh-CN-100/` 保留 native.log、真实 D3D11 截图及 evidence。生产项目关闭和输入退休成功，本轮无泄漏；本轮未到 IME，不能证明编辑后的退出问题已修。
2. 目录无业务作用的 Selected 仍发布全量视图并推进 UI revision，可让同一次原生动作已分配的子页请求过期。最小访客回归 `tree_request_tests::directory_and_ignored_selection_dispatch_preserve_the_admitted_scene` 在 0.00 秒失败，日志 `target/git120-directory-selection-guest-red.log`；生产修复尚未开始。新增实际包 Manager 回归 `workbench::tree_pages::git_package_directory_selection_keeps_an_already_allocated_child_page_current` 已落盘，尚未执行。必须保留真实文件/ref 选择的视图更新，不得简单静音所有 Selected，也不得放宽 Manager 精确 revision/来源门禁。
3. 冷启动宿主观察红灯 `extensions::cold_observation_tests::cold_native_observations_follow_live_incarnations_without_starting_a_guest`，0.14 秒，日志 `target/git120-cold-observation-red.log`。已授权并落盘的最小生产修改意图为：epoch0 不发送/缓存 Theme/Focus；新 incarnation 重发相同主题；旧 focus subscription 不借用替换实例。已暂停在生产增量未验证阶段。

## 恢复顺序

1. 先核对 HEAD、真实/临时 index、上述日志与原工作区未验证增量，继续保留其他任务改动。暂停备份入口为 `target/git120-paused-2026-10-16/`，包含临时 index 副本、冷观察增量补丁、源码/日志身份清单。
2. 将冷观察生产修复从修改前快照作三方增量合入临时 index，仅导出相关路径；先运行冷观察回归并修复真实失败，再跑相关可见性/生命周期消费者。
3. 先以冻结 B20 包执行新增 Manager 目录回归确认红灯，再修复访客目录/未知/忙碌 Selected 无业务变更的场景发布。exact SDK 定向转绿后全访客回归、重建新包，记录新摘要；B20 的通过记录继续保留历史身份。
4. 复验原生完整流程与 IME/Undo/退出，再执行深浅主题 × 中英文 × 100/150% 的八组合。DPI 仅向自有 HWND 发 WM_DPICHANGED，不改 OS 设置；系统 IME 候选弹窗未实测，不能冒称已验收。
5. 最终同包真实 Git、五窗口、共享 UI 消费者与仓库门禁，正式宿主固定点 `2b9e1548a91925493ac89f397ff1cd8c0c0380e0...HEAD` 双轴审查，全部通过后再普通提交、推送、核对远端并关闭 #120。任何未完成项都不能按完成关闭。
