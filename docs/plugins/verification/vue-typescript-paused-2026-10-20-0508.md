# Vue / TypeScript 暂停检查点

保存时点：2026-10-20 05:08，Asia/Shanghai。用户最新要求“暂停所有工单并保留进度”，已停止实施与诊断代理；当前没有本轮运行中的构建、原生验收或桌面应用。仅保存工作树、产物和记录，恢复必须收到新的继续指令。

## 工单与交付状态

新 #121 的既有完成、推送和关闭保留。新 #122–#126 本轮未提交、未推送、未关闭；旧 #108–#118 和父 #107 不执行、不改写、不关闭。功能专项通过与正式交付完成分别记证。

| 场景 | 已保存的实际进度 | 恢复后剩余 |
| --- | --- | --- |
| S2 / #122 | 共享脏图、非空私有选择偏好、来源 F12、实际补全 resolve／导入及跨文件事务通过 | 最终双包、普通桌面、提交交付 |
| S3 / #123 | source/sealed 102 项、历史八行事务、两种 external、四项 Windows 格式化及语法资源闭包通过 | 最终双包、普通桌面、提交交付 |
| S4 / #124 | 四角色映射／事务／替换／卸载通过；四类冲突拒绝和移除恢复整项 PASS 280.28s | 最终双包、普通桌面、提交交付 |
| S5 / #125 | 通用回滚／离线／租约、真实内层退出通过；真实外层正反向候选／EOF／恢复全链 PASS 164.27s | 附加 Worker ignored 回归仍失败；最终双包、桌面与交付 |
| S6 / #126 | 双轴增量复核与最终生产门禁已保存；两份桌面候选输入快照已固定 | 定位下述失败、打包、桌面、正式支持元数据、最终双包复验与三仓交付 |

完整证据见[实施日志](vue-typescript-remaining-2026-10-19.md)、[双轴审查](vue-typescript-remaining-review-2026-10-20.md)、[S3/S4 验收](vue-typescript-s3-s4-native-acceptance-2026-10-20.md)。[覆盖账本](vue-typescript-remaining-coverage-2026-10-19.md)最后批次早于本检查点，其中 B1 尚待 S4、B5 尚待 wide 的表述由本记录更新：S4 整项和生产 wide 已实际通过，不能再次误列未执行。

## 工作树与保留方式

| 仓库 | 目录／分支 | HEAD 比较基线 |
| --- | --- | --- |
| Editor 集成树 H | `C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor`，`codex/vue-typescript-remaining` | `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` |
| TypeScript T | `C:/Projects/RustProjects/TypeScript-for-Nanobug`，`codex/vue-typescript` | `873e8c3d3ebe3a86a8bb1e028a3c4bcd1c557b94` |
| Vue V | `C:/Projects/RustProjects/Vue-for-Nanobug`，`codex/vue-typescript` | `f8e5880bf89161f16723ae7be1c76458d50a676c` |

修改保留为各自未提交工作树和未跟踪文件，没有 reset、stash、清理、归档或新提交。暂停字节与 Git 状态账本保存在 H 的 `target/vue-typescript-paused-2026-10-20-0508-inputs.json`；target 中原失败日志、成功日志和冻结程序继续保留，不能当作可随意清理的缓存。

用户原树 `C:/Projects/RustProjects/Editor` 未用于暂存提交；原 Excalidraw SHA-256 仍为 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970df9f934c6`。旧暂停 stash `465649b4f1396d5aeba5617326de16af6b5989a9` 和旧工作树保留，不整包重新应用。

## 最后通过的宿主与门禁

- `cargo fmt --check` 通过，包括最后 Worker 测试格式修正后的 `target/s6-final-worker-trust-fmt-corrected.log`。
- `cargo test --workspace --exclude editor-app -j1`：85 个成功 suite，**319 PASS / 0 FAIL / 327 ignored**，`target/s6-final-delivery-non-ui.log`。不将 ignored 记为通过。
- `cargo check --workspace -j1`：PASS 8.77s，`target/s6-final-delivery-check.log`。
- 正常宿主构建 PASS 2m19s；`target/s6-final-delivery-native-host/editor-app.exe` SHA-256 `996bf4fdbb8fcf79a4949c6ae7fe3035c1cfe1b53e3de6b1d48badf52f498448`。
- 最后 `cargo test -p editor-app extensions::worker::worker_tests:: -j1 -- --nocapture --test-threads=1`：**6 PASS / 0 FAIL / 8 ignored、1.62s**，`target/s6-final-worker-catalog-all-green.log`。
- 中英文安装进度 ignored 用例显式 **1 PASS、0.14s**，`target/s6-worker-progress-locale-final.log`。

上述最后两处源码更新仅为 `#[cfg(test)]` 的旧文案期望，不改变 non-UI、normal 或 check 读取的生产实现。当前最后测试程序 `target/s6-final-delivery-tests/deps/editor_app.exe` SHA-256 `84b78e9e054d8e2af3d0f8365ac95cd24f108ee5aff5c447f51082488c8d0a6c`。

完整 S4 与真实 Windows S5 使用较早冻结程序 `target/s6-native-normal-frame/deps/editor_app.exe`，SHA-256 `706332eba442bf2ccb266fbe23b33b7f49c84d2cb43be487a25e0c2ebcabb399`。后续仅改测试文案；不得将这些通过重写成在 84 程序上运行过。

## 当前失败断点：附加 Worker ignored 回归

入口：`extensions::worker::worker_tests::cancelled_preparation_does_not_replay_an_older_trust_grant`，必须显式 `--ignored --nocapture --test-threads=1`，不能直接运行内部 helper。

1. 普通归属测试与该 ignored 屏障曾查找旧 literal `Plugin command owner changed`；现在两处均改为当前 `plugins.command_owner_changed` catalog 与 `StaleRevision`，源文件 `crates/editor-app/src/extensions/worker/worker_tests.rs` SHA-256 `ff6902558fd918d86c8953a740dc251cc445bd4bac069ed2a0df950d641e460f`。两轴独立静态复核 0 新问题，视图为空、旧版本、授权及 checked shutdown 断言均保留。
2. 旧 ignored 入口实际 FAIL 126.70s：`target/s6-worker-trust-catalog-red.log`；修正后 **仍 FAIL 129.11s**：`target/s6-worker-trust-catalog-green.log`。文件名的 green 不代表通过。两次均只报告 `wait_for` 第 212 行 `worker publication timed out`；修正后设了 `RUST_BACKTRACE=1`，仍无调用者帧，因此失败究竟在初始 view、中间命令或最后 barrier **未知**。
3. 暂停前发现输入定位更正：`run_preparation` 使用 `archive("capability-example")`，loader `tests/support/packages.rs::input` 取账本首个同 ID，实际是 **0.17.1**，SHA-256 `20f5d815b109f46ab54e22bf56ccd89497c83fd4b2619250294e1a02cad249d8`。此前按 `.19.1` 推测此入口输入不成立；不能据 `.19` 大小／摘要诊断当前失败，也不换包或改账本掩盖失败。
4. 暂停前没有增加阶段标签、track_caller 或状态快照，没有延长 60s，没有改产品。恢复先建立能区分各个 wait 的最小测试观测，再决定原因；不把整体 129s 当作单个期限，不盲重跑或增加期限。

两次失败没有取得本用例 checked shutdown 成功证据。测试进程均已退出；此前真实 S4／S5 的 checked cleanup 通过属于各自独立证据，不能代替本用例。

## 插件产物与桌面候选输入

当前插件源版本 T `.15` / V `.12`；TypeScript native 仍为 `.14`，Vue 两模块仍为 `.11`。支持元数据仍 pending，没有正式提升。最后已验证公开 SDK 为 `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`；新正常宿主的公开打包导出仍待实际核对。

保留的已验输入：

| 输入 | 路径（相对对应仓库） | SHA-256 |
| --- | --- | --- |
| TS `.14` Package | `T/target/s4-context-package-014/typescript-0.1.14.zip` | `92c29c83c8fa4e72fea83cfc11f3917773e43845c54a57d24e1f68d8dae5e9c9` |
| TS native `.14` | `T/artifacts/typescript-windows-x86_64.zip` | `71c2794f0833bb548d365255878362b4637750bdac05e7ff7e8392df73884f83` |
| Vue `.11` Package | `V/dist/vue-0.1.11.zip` | `8e665285ac0d2701ba513d2d11ddd63f61771a27e21843e6869f000a7ee16252` |

两份新的桌面候选输入已复制并逐字节校验，**尚未打包**：

- T `target/s6-desktop-candidate-015-source`：29 文件、40,488,127 bytes，独立构建 key `3914ed5b766817c2`。
- V `target/s6-desktop-candidate-012-source`：44 文件、21,415,826 bytes，独立构建 key `5192bbbb352d98ed`。
- 73 个复制文件闭合、47 个保留资源、TS native ZIP、Vue 十一项模块封印均不变。账本为 H `target/s6-desktop-candidate-inputs.json`、`target/s6-desktop-candidate-input-closure.json`；两个 `*-package` 输出目录尚未创建。

## 收到继续指令后的顺序

1. 先核三仓状态、当前 HEAD 与上述字节账本，保留原树和旧失败，不把未提交差异遗漏为已完成。
2. 定位并完成附加 Worker ignored 回归；若改生产源码，补相应真实 RED/GREEN、门禁与双轴复核，不能套用旧通过。
3. 用正常宿主公开 SDK／package CLI 构建上述桌面候选，先核每份 source 的 Cargo workspace_root／独立 target；串行 `-j1`，不修改或覆盖历史 ZIP。
4. 用当前双候选运行混合项目 S1，设置 `NANOBUG_S1_NATIVE_PROFILES` 保留成功私有 profile，再以 normal `.996…` 通过 `@oai/sky` 做普通桌面可见输入验收。没有完成此步骤，实际 OS IME 候选窗也仍未验收。
5. 之后才提升正式支持元数据、构建独立最终 `.15` / `.12` Package，核对资源／许可／SDK、按固定字节继承证明执行最终双包 S1／policy；最后审查和三仓普通提交推送、核对远端、读回新 #122–#126。

首批范围仍仅 Windows x86-64、TS 5.9.3、Node 24.19.0、Vue 2.6.14／2.7.16／3.5.22 的已验依赖组合。更早、未验补丁和未来版本按长期计划继续，Pug／Stylus／Nuxt 专属语义由独立插件承担。不创建 Release、不强推、不重写历史。
