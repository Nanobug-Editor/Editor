# Git 低频静默刷新回归

用户最终要求：允许自动刷新，但不要太频繁，刷新不能影响使用，改为静默刷新。此前“不要每隔 5 秒刷新”不解释为删除自动观察。新规则已同步到 [Git 规格 Q17](../specs/git-plugin.md)。本次不重开已关闭的 #119/#120，也不继承旧包的验收结论。

## 根因与范围

排查开始时，默认用户配置记录的是启用的独立 Git 0.1.7，包摘要为 8034109e64266b12e17d70fcb7c513de81d1b2130b690baa117ae44450135d71。交付前再次只读核验时记录已是 0.1.8，摘要 5c6c483fd60b9fc02277aadfe800408fbc324eab1b99ffac7e1ddf0acb9b3292；本次没有修改用户注册表、插件私有数据或停止用户进程，不能宣称用户正在运行新包。

独立 0.1.8 仍用 3 秒定时器和 3 秒观察门禁。它已保留无变化的树页和场景，但读到真实 status、refs、HEAD 或历史变化时退出静默发布，剩余只读队列仍使 busy 为真；视图据此整体禁用，当前输入又不能抢占这段剩余读取。因此单纯降低频率不够。

产品源码只修改宿主外的 C:/Projects/RustProjects/Git-for-Nanobug。宿主本次仅扩充既有公开包和原生回归，没有增加 Git 专属宿主 API，也没有复制 GPUI Kit、base 或 SDK 到插件。仓库原先的主分支构建缓存修复及其文档保留，不计为本次成果。

## 实现

- 包版本提升为 0.1.9；协议 7、SDK 0.2.0、能力与权限保持原基线，依赖版本不变。
- 后台定时器 30 秒；距上次完整历史观察不足 30 秒、正在处理任务或距最近用户交互不足 3 秒时跳过。跳过不会积压补刷，实际间隔可能更长。
- 将无变化时不发布场景的 automatic_observation 与可被当前输入撤销的 background_read 分开。真实变化可以发布新内容，剩余可选 Git 读取仍不占用全局控件忙碌状态。用户当前操作先撤销该读取，再走已有业务检查；不重放旧输入。
- 无变化时保留场景、树页、草稿、焦点、选择、展开、滚动与菜单。真实变化仅使其所属来源失效；旧 revision、index、文档版本和破坏性操作确认仍有效。
- 保存、激活、手动刷新及业务结束的显式刷新继续走原即时／合并请求入口。真实业务写入、原生文档恢复查询及失败的操作身份观察保留必要忙碌状态；静默标记不能让旧冲突建议恢复权威。

## RED 与最终回归

所有日志位于本仓库 target/。私有策略诊断只用于定位，最终新增行为由实际 ZIP 经公开 Manager 验证。

1. 原 0.1.8 经 refresh_stability 回归：28 秒稳定检查期间不应有新的自动 Git 进程，实际 process_count 为 1，RED，54.83 秒。日志 git-silent-refresh-manager-red.log。
2. 只延长间隔的诊断包，在外部状态和分支真实变化时，逐次 poll 检查发现 changed background data disabled staging/staging，RED，138.76 秒。日志 git-silent-refresh-changed-data-red.log。首次尝试遗漏关闭仓库信息模态框，属于测试前置状态错误，单独保留在 git-silent-refresh-changed-data-modal-attempt.log，不作为产品缺陷证据。诊断包 git-0.1.9-interval-only.zip 未交付、安装或发布。
3. 最终 0.1.9 包显式执行三个 ignored 集成测试，3/3 通过，266.77 秒，日志 git-silent-refresh-manager-final.log：refresh_stability 确认低频、可抢占读取、迟到输出不影响当前对话框、真实 status/refs 变化时控件仍准入；scene_activity 连续 65 秒真实输入跨两个定时器期限，恢复安静后自动看到外部提交，旧事件仍拒绝且焦点刷新及时；observation_failures 确认真冲突的原生 ReadDocument → LimitExceeded 仍发布必要忙碌权威，错误恢复后可用且资源归零。
4. 最终原生 idle_retention 回归 1/1 通过，134.90 秒，日志 git-silent-refresh-native-final.log。暗色、简体中文、100% 的实际 Windows/D3D11 窗口，两段无输入闲置分别为 35052 ms／620 帧和 35048 ms／612 帧；第二段确实观察到 Worker 中的后台 Git 进程，不能以未执行计时器冒充静默成功。逐帧检查原生控件及 Worker 事件准入、场景连续性、草稿与原选区、src/Local 展开、选中文件、11 px 滚动锚点、菜单及焦点。单次 D 替换原选区成功，单次 Escape 关闭菜单。四张 idle-state/menu-before/after 实际截图均已逐张查看，布局及保留状态相符。
5. 原生证据为 target/git-silent-refresh-native-final/dark-zh-CN-100/evidence.json，SHA-256 85660c1ab79dafab9392963a743e99845ca47f6dc9da004e60d955e196bc8262。shutdown_complete 为 true，无意外错误日志，Actor／GPUI 窗口与 HWND 正常退役。该用例明确为 idle-refresh-state-only，不代表重跑完整 Git 业务矩阵，也未验证系统 IME 候选窗口。

## 构建与规定检查

使用现有目标工具链，通过当前宿主 target/release/editor-app.exe 的公开 --plugin-cargo 和 --plugin-package 构建，未安装工具或更换依赖。该宿主 SHA-256 为 7ece4d1c03152c79d7166ef03b3fa4e4a8008c5b55d7ebd9d8707fd9c8c8c449；SDK 缓存身份为 943ea1befc2adad50202c39d30acbb6ed9525771dbddb09bf78454bbf14213e5。宿主与独立插件不共用 Cargo target 目录。

| 实际命令 | 结果与日志 |
| --- | --- |
| 插件 cargo fmt --check | exit 0，git-silent-refresh-guest-format-final.log |
| editor-app.exe --plugin-cargo ./Cargo.toml test --lib | 101 passed，0 ignored，git-silent-refresh-guest-tests-final.log |
| editor-app.exe --plugin-package . --output C:/Projects/RustProjects/Editor/target/git-silent-refresh-2026-10-18 | exit 0，git-silent-refresh-package-build-final.log |
| cargo fmt --check | exit 0，git-silent-refresh-host-fmt.log |
| cargo test --workspace --exclude editor-app --jobs 1 | 78 组、289 passed、0 failed、305 ignored，git-silent-refresh-nonui-workspace.log |
| cargo check --workspace --jobs 1 | exit 0，git-silent-refresh-workspace-check.log |

三个真实包测试和一个原生用例均设置 NANOBUG_GIT_CANDIDATE 指向最终包，并用 --ignored 显式执行；未把工作区跳过的用例计作通过。原生入口额外使用 NANOBUG_GIT_NATIVE_IDLE_RETENTION=1 和私有输出目录。现有 dead_code／链接警告保留，规定检查没有失败。

## 独立交付与限制

可安装包：C:/Projects/RustProjects/Editor/target/git-silent-refresh-2026-10-18/git-0.1.9.zip，804477 bytes，SHA-256 9fd8b1da543fcbec9fbf6376caa3446319bad08c22e16702b01fb45ae87ef413。同目录提供使用说明.txt、verification.json；插件自身 dist/0.1.9/ 保存相同 ZIP、53 文件源码归档和逐文件摘要账本，逐项核验归档与当前源一致。源码归档 SHA-256 为 14bb3a121371f1f6bd8d8fd8873b89d7f8ba9a49f109de89aec362e70751d38a。独立源码目录没有 Git 元数据，此快照固定本次可恢复源身份。

安装 ZIP 仅含 git.wasm（2474187 bytes，SHA-256 b7f0685aae1350d145ee041e4033a8d1e2368bc5afd23198f82b1f7969a7d627）、manifest.json 和 native/nanobug-git-rebase.exe。原生 helper 摘要仍为 73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a；打包后清单相对 0.1.8 只有包版本不同，没有新增权限或能力。结构化身份记录为 target/git-silent-refresh-package-identity-final.json。

需要用户通过 Nanobug 插件管理器选择此 ZIP 更新，修改宿主源码不会更新已安装的独立插件。本次没有替用户安装 0.1.9、修改全局环境、提交、推送或发布 Release。只重跑与本次相关的原生暗色／简体中文／100%组合；不将先前其他组合或完整业务矩阵计为新包已重验。
