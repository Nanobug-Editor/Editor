# Git 分支快速筛选与连续绘制

日期：2026-10-21。状态：实现、插件回归、最终实际包和原生帧验收通过，候选已生成。此轮没有创建／关闭工单、安装插件、重启用户程序、提交或推送。

## 已证实的问题与取舍

用户反馈分支选择仍有明显白屏并要求参考 IDEA。此前 0.1.21 已保留等待期间的模型数组，但每次筛选仍启动 `git log`，随后串行读取首条提交的文件和说明；这些读取进入业务 busy 门禁。没有按分支复用已接受结果，返回已看过的分支仍等待进程。先建立真实分支事件／回执回归，再修复；没有把数组非空或控件禁用等同于实际画面可见性。

- 红灯：通过配套宿主 `--plugin-cargo ./Cargo.toml test --lib branch_switch_restores -- --nocapture`，首轮失败于返回 main 后首条仍是 topic 的 OID。日志在独立项目 `target/branch-fast-switch-red.log`。
- 实际包红灯：保存的 0.1.21 包经公开 Manager 安装，原生分支行点击后、任何进程轮询前，目标 topic 仍绘制 base；增强回归失败于 `visited branch did not publish immediately`。`target/git-branch-fast-switch-2026-10-21/native-baseline-red.log`。该场景未捕获旧包的零文字绘制帧；不能把旧内容等待夸大为已经复现像素全白。
- 参考 [JetBrains 的 VcsLogFiltererImpl](https://github.com/JetBrains/intellij-community/blob/master/platform/vcs-log/impl/src/com/intellij/vcs/log/visible/VcsLogFiltererImpl.kt)：无详情过滤时按 matching heads 创建内存可见图，需要时再使用索引／VCS。这里只采用缓存、图过滤与读取分离的思路，没有复制上游实现或声称实现其完整持久化索引。

## 实现与边界

业务全部位于独立 `C:/Projects/RustProjects/Git-for-Nanobug`，版本从 0.1.21 提升到 0.1.22。新 `src/history/cache.rs` 管理最多十二个分支首屏、每屏最多 128 行、四 MiB 的已计量提交元数据预算；完整 OID 预览另有十二项／两 MiB 预算。它们均为不可变查询结果，不是编辑器文本或 Undo 副本。

命中缓存后直接恢复列表及首条预览；首次选择则从已加载图投影以当前 ref tip 为起点的已知祖先，并读取有界真实页。无法投影时继续绘制最后完整列表，右侧标注正在加载的筛选；新结果完整后按原有首尾区间增量替换，不预先清空。首次未缓存读取仍受真实 Git 和仓库大小影响。

只读历史队列不拥有全局禁用权。分支选择及其只读进程回执只发布 history 面板，暂存输入场景保持原 revision。当前操作可关闭其拥有的只读子进程，新的分支筛选 generation 拒绝迟到的成功、失败及 A→B→A 回执；被预览等操作打断的冷筛选在该操作结束后恢复。已取消筛选不能把旧分支行数当新分支的分页 skip。写操作、仓库发现、比较／编辑器请求继续遵守原有忙碌、审查与权限门禁。

`for-each-ref` 额外观察 objectname，分支名相同但 OID 变化也会使历史缓存失效；HEAD 变化、重新发现工作区同样处理。按 OID 缓存的文件／完整说明保留不可变语义。没有新增公开能力、协议兼容分支、安装权限或 Git 专属宿主 API。

宿主改动仅为测试：`ui/plugin/canvas.rs` 的 `cfg(test)` 观察记录实际 paint 提交的可见文字及 revision；`extensions/windows_tests/incremental_review.rs` 增强原生鼠标、公开 Manager、目录树和绘制回归。生产宿主逻辑与 SDK 契约不因本轮改变。

## 验证

| 入口 | 实际结果 |
| --- | --- |
| 宿主 `--plugin-cargo ./Cargo.toml test --lib` | 132 passed，0 failed，0 ignored；包含缓存预算、投影、迟到回执、OID 移动、写保护、取消后分页及恢复。独立项目 `target/branch-fast-switch-unit-final.log` |
| 实际包逐帧回归，显式 `--ignored` | 第一轮 1/1，约 61 秒；146 条提交的仓库中，128 行首屏与 2 行分支反复切换。暂存树／提交文字／提交文件树持续存在，native paint revision 对应当前场景且文字非空；命中不启动子进程，暂存场景 revision 不变化 |
| 实际包双／三源预览及未保存文本保护，显式 `--ignored` | 第一轮 2/2，151.23 秒。最终摘要包复验另列 |
| `ui::plugin::` | 69 passed，17 ignored，0 failed，3.71 秒；未执行 ignored 不计作通过 |
| `cargo fmt --check` | 通过 |
| 独立项目 `cargo fmt --all --check` | 通过 |
| `cargo test --workspace --exclude editor-app --jobs 2` | 通过；完整日志 `target/git-branch-fast-switch-2026-10-21/nonui.log` |
| `cargo check --workspace --jobs 2` | 通过，15.04 秒，既有告警保留 |

首轮长短列表四次命中从原生鼠标操作到同步发布分别为 54.613、57.875、54.836、57.464 ms，均在任何进程轮询前取得目标提交且子进程数为零；这些是 GPUI 原生测试中的事件＋发布样本，包含测试绘制开销，不是显示器实际呈现延迟或持续 FPS。原始 `native/branch-fast-switch.json` 保留；最终候选样本见补录。

最终摘要包复验：`final-tests.exe extensions::windows_tests::incremental_review::git_branch_queries_never_blank_populated_review_and_patch_only_changed_files --exact --ignored --nocapture --test-threads=1` 通过，1/1、60.85 秒。使用 `NANOBUG_GIT_CANDIDATE=<候选 ZIP>` 和 `NANOBUG_GIT_NATIVE_OUTPUT=<候选目录>/native-final`；日志 `native-final-candidate.log`，样本 `native-final/branch-fast-switch.json`。四次命中分别为 **52.396、52.579、51.467、52.960 ms**，子进程数均为零，当前原生绘制文字非空；输入发布未重建暂存场景。通过真实外部 checkout、新文件及删除文件继续核对增量与展开保留。

最终包的两个历史比较回归也显式通过，2/2、20.16 秒，日志 `comparisons-final-candidate.log`。为缩短重复编译，使用此前冻结的 `target/diff-scroll-2026-10-21/optimized-tests.exe`；该宿主的生产契约与本轮相同，本轮宿主新增部分全部为 cfg(test) 观察和夹具，未把它当作新原生帧回归执行器。新的逐帧与缓存断言由本轮重建的 `final-tests.exe` 运行；两个执行器均经公开 Manager 安装同一最终摘要 ZIP。`git diff --check` 同时通过。

中间一次全量插件回归曾失败两项：扩大默认选择条件覆盖了后台刷新中的原选择及多父选择。已将重选限制为显式筛选，随后全量恢复通过。初次新测试还曾私自调用私有动作构建方法而编译失败，已改为通过公开 UI 事件路径；均未当作成功验证。

## 候选身份

候选目录：`C:/Projects/RustProjects/Editor/target/git-branch-fast-switch-2026-10-21/`。`baseline/git-source.zip` 保存本轮开始的 68 项独立源码，`guest-task-changes.patch` 只比较本轮访客差异。最终源码归档含 70 项，排除 target、dist、SDK 及上游源码，逐项账本为 `guest-source-inputs.json`。

| 产物 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `packages/git-0.1.22.zip` | 886386 | `239a0033145048333fff43528287b1cf74c8dd2cba6ec3af7484cc0e5c30ff25` |
| 包内 `git.wasm` | 2784041 | `9a1f86b9a7bc4cbf5ef366e15346cdcbe549cf4022ec77a167f2e4199c9e9b8c` |
| `packages/git-for-Nanobug-0.1.22-source.zip` | 205836 | `67be8fbda3b8e24b3de956699dfda5ced3882c7d6508ff3d07f6117a1f6fa926` |

原生 helper 448000 bytes，SHA-256 `73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a`，没有变化。打包使用此前的配套宿主 `target/diff-scroll-2026-10-21/Nanobug.exe`，公开 SDK 摘要 `33da6ec515afebef7d70f4998df203f22c318d14b0e9661bdd4fa6f596982c13`；缓存 src/ 下 53 个文件与当前公开契约逐项相同。当前已安装的仍是旧 0.1.21 摘要；本次只生成新候选，没有覆盖运行中的包。

源工作区是 main 的 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 加既有未提交修改；既有比较滚动、树补丁及其他任务成果全部保留。用户布局图 SHA-256 仍为 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6`。没有运行完整工作台矩阵或系统 IME 候选窗验收，不借本轮关闭历史工单。

返回[插件目录](../README.md)。
