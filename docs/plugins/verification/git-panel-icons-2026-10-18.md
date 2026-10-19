# Git 面板图标与底栏入口

用户指定暂存提交、Git 面板使用下载目录中的两份 SVG，并去除另外两个窗口的底栏入口。提交详情、冲突处理仍从 Git 操作打开；本次不删除这些业务功能。需求修订已写入 [Git 规格](../specs/git-plugin.md)。

## 实现与独立边界

独立源码 C:/Projects/RustProjects/Git-for-Nanobug 的 icons/staging.svg、icons/git.svg 分别采用用户文件的 1 条、3 条 path；已逐项比对 d 与 viewBox，轮廓保留。去掉 XML/DOCTYPE、来源元数据并使用 currentColor，让既有原生 Icon 提供主题颜色，也符合插件包的安全 SVG 格式。比对记录为 target/git-panel-icons-geometry.json。

图标及排序由 Git 清单声明，构建描述将 icons/ 纳入独立包。两个工作窗口声明 show_in_window_controls=false。宿主公开 Panel 增加此字段，ui.panels 升为 1.1；窗口组在直接按钮与溢出菜单共用的入口投影处过滤。省略字段默认 true，序列化时省略未改变的默认值；旧插件不因此增加新字段或失去入口。false 必须明确要求 ui.panels >=1.1，原 default_visible、会话显隐与 SetPanelVisibility 保持独立。

宿主没有 Git ID 分支、专属 API、业务图标资源或上游副本。SDK 用途与门禁见 Panel 字段注释及 [工具文档](../../../crates/plugin-protocol/TOOLS.md)。Git 版本升为 0.1.10，保留 0.1.9 静默刷新代码；源码账本确认旧 53 文件中仅 Cargo 清单／锁、插件清单、构建描述和双语 README 改变，再新增 2 份 SVG，没有修改访客 Rust 业务源码。

## 验证

- 插件 101/101 测试通过，6.37 秒；插件 cargo fmt --check 通过。日志 target/git-panel-icons-guest-tests.log、git-panel-icons-guest-format.log。
- 通用能力回归由两个任意插件身份验证省略字段保留默认行为、默认值不序列化、新能力下限及 ^1 不能授权隐藏；包含在本次非 UI 工作区测试中。
- contextual_panels_omit_window_controls_and_keep_programmatic_visibility 经真实 tools-example WASM 包、公开 Package/Manager 和 GPUI 验证：隐藏入口不出现，省略字段的其他入口仍出现，独立工作窗口可打开并正常关闭，18.03 秒，1/1 通过。日志 target/git-panel-icons-native-controls-public.log。开发中静态包和旧 Dock 夹具不满足独立窗口／能力／权限前置条件，相关失败保留，未放宽产品验证器；最终夹具按真实公开契约声明 ui.windows、ui.panels 和独立窗口位置。
- 实际 Git 0.1.10 的 Windows/D3D11 截图确认两个新图标已用于面板标题和底栏，另两个底栏入口消失。图标与入口绘制已核验；额外的长时闲置用例仍有下述焦点限制，未计为通过。
- 前两次闲置用例分别在 68.69、77.10 秒因 window_active=false 中止；场景、草稿、树与事件准入保持，窗口清理为零，但 shutdown_complete=false。证据保留于 target/git-panel-icons-native/ 和 target/git-panel-icons-native-final/。
- 隔离复测期间工作区新增与本任务无关的 exclusive 停靠实现，重新编译报 E0425／E0061，见 target/git-panel-icons-native-appearance-isolated.log。本次未回退或修复这些并行改动。下表是这些改动出现前的实际通过记录，不宣称其覆盖后来的整个工作区。
- 将已通过公开包开关回归的测试程序保存为 target/git-panel-icons-native-runner/editor-app-tests.exe，SHA-256 2b6b53f8db115083fa0643b4eab7027c7432caf6f4d103911395c2041fe39fea，再以相同包运行，期间不启动其他验证进程。第一段闲置 35022 ms／548 帧通过，单次 D 替换原选区成功，菜单正常打开；后续仍因窗口失去前台焦点中止，136.51 秒。未削弱焦点断言或强制重放输入，三张实际截图均已查看；第一段没有观察到后台进程，第二段未完成，因此不宣称本包完整的两段闲置／实际定时读取回归通过。最终记录在 target/git-panel-icons-native-retained/dark-zh-CN-100/evidence.json，原生窗口与 HWND 退役为零。

| 检查 | 实际结果 |
| --- | --- |
| cargo fmt --check | exit 0；target/git-panel-icons-format-final.log |
| cargo test --workspace --exclude editor-app --jobs 1 | 78 组、290 passed、0 failed、305 ignored；target/git-panel-icons-nonui-workspace.log |
| cargo check --workspace --jobs 1 | exit 0；target/git-panel-icons-workspace-check.log |
| cargo build -p editor-app --release --jobs 1 | 最终重复构建 exit 0，8.74 秒；target/git-panel-icons-host-build-final.log |

真实包的 GPUI 回归均显式加 --ignored；没有把 305 个跳过用例当作通过。原有 dead_code／链接警告保留。没有重复完整 Git 业务矩阵，也不将既有 0.1.9 验收直接计为 0.1.10 已重验。

## 交付身份

已通过构建的主分支 Release 宿主另存为 C:/Projects/RustProjects/Editor/target/git-panel-icons-2026-10-18/Nanobug.exe，SHA-256 42697b01aa15f23276cbed53903922e93c1612f9c786f916280d77f4ddc7cbbf。原构建输出为 target/release/editor-app.exe；后续其他任务可以继续更新主工作区，本次交付副本保留实际构建身份。

安装包：C:/Projects/RustProjects/Editor/target/git-panel-icons-2026-10-18/git-0.1.10.zip，806949 bytes，SHA-256 bb935fad826c6b39312073cb9292483c794e5e08038826b5ceae61e7ce379bcb。由新宿主通过 --plugin-package 直接构建，SDK 缓存摘要 ad38a7ef5fecfae68a1c0bdd7a6d6a3e551a5fb47a8495b58f9ef9d3c159ef6b；协议 7、SDK 0.2.0、不增加权限。包只有 WASM、清单、两份 SVG 与原生 helper，helper 摘要仍为 73937bb666aee70923a509bc6147ab104730072b5d382143a2a0ded43e79ff6a。完整条目摘要见 target/git-panel-icons-package-identity.json。

独立项目 dist/0.1.10/ 保留相同安装包、55 文件源码归档与逐文件账本，均逐项核对当前源及归档。源码 ZIP SHA-256 a537f241fc2575f4edecb730092e56a25fdb561347950e85d57bd5e73af4aab3，无 SDK／上游项目副本。安装包旁有使用说明.txt。需要使用新宿主，并通过插件管理器更新独立包；本次未替用户安装、改写私有数据、提交、推送或发布。
