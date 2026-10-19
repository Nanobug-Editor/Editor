# Git 面板列表布局精简

日期：2026-10-19。范围：按用户截图精简独立 Git 插件的底部面板。候选包 0.1.12 及配套宿主完成下述验证，尚未替用户安装或发布。

## 行为

- Git 面板只保留分支树和提交画布，移除搜索／目标输入行、提交表格头、底部状态与信息栏。顶部工具栏沿用 0.1.11 已移除的行为。
- 拓扑绘制区域左侧增加 16px 留白，标题首列相应让位；文字列、提交行身份、全宽选择及统一滚动坐标保持一致。
- “更多提交”和“仓库信息与操作结果…”加入分支树原生右键菜单；Stash 列表沿用同一菜单已有入口，不再保留底部按钮。
- 仓库信息对话框保留仓库身份、拉取偏好、上次远程获取时间和最近操作结果。非仓库目录也可查看信息／诊断及关闭对话框，不要求先成功发现仓库；这些只读动作不启动 Git 或私有存储请求。
- 不新增协议能力或权限，不改变暂存面板、比较窗口、危险操作确认及低频静默刷新策略。宿主测试调整为通过真实菜单访问已迁移入口，保留实际 Git 效果和诊断断言。追加定时器回归发现真实宿主命令不能清理本实例先前由普通回调创建的资源，已修复宿主通用权限检查；不引入 Git 专属分支。

## 已执行验证

- 独立源码使用配套宿主 `--plugin-cargo ./Cargo.toml test --lib`：100 passed、0 failed、0 ignored，日志 `Git-for-Nanobug/target/history-layout-unit.log`。已将过时表头／底栏预期更新为公开场景中无常驻栏、拓扑留白和按需信息检查。
- 通过公开 `--plugin-package` 构建实际 WASM、原生辅助程序和 ZIP；独立 56 文件源码快照逐项与当前源码比对通过，无 SDK 或上游源码副本。
- 真实包／公开管理器：130 个实际提交，首批 128、右键“更多提交”后 130，拓扑留白、无表头／底栏、仓库信息及 Stash 列表均通过；既有分支右键目标、禁用规则、确认后切换、新分支、合并预览及取消回归同包通过。首次批次中能力最低版本负例仍查找旧底栏，迁至比较窗口已有诊断后单独通过（47.17 秒，`target/git-history-layout-floor.log`），不把失败批次整体计为通过。主批次日志 `target/git-history-layout-manager.log`。
- 非仓库根目录与正常仓库加载回归通过（95.07 秒，`target/git-history-layout-project.log`）：从真实菜单读取无仓库诊断、无 Git 叶节点但保留仓库动作目录、正常仓库历史／暂存树及提交详情身份。旧测试的刷新按钮和空分支目录断言已按 0.1.11 的菜单行为同步，诊断及资源释放检查保留。
- 原生 GPUI 最终两项通过，96.33 秒（`target/git-history-layout-native-final.log`）：真实菜单禁用项不派发，确认后实际切换；真实完整 history 场景在深浅两种主题和 760／1100／1500px 三种宽度下绘制，分支树和画布顶部、底部齐平，画布保留至少 280px 高度，移除的控件不再出现。完整布局项仅观察绘制，不重放被动尺寸回执；菜单交互由另一项实际输入覆盖。早期同包两项亦通过，107.91 秒，保留 `target/git-history-layout-native.log`。
- 真实本地 bare 远程成功流程通过，71.15 秒：Fetch 后在信息对话框读取 UTC 时间，Pull、Push、已发布历史改写、独立确认及精确 lease 继续通过。日志 `target/git-history-layout-network.log`。
- 追加真实定时器回归时，迁移后的原生类型化菜单命令在中断后台读取前进入资源清理，旧宿主把本实例由定时器创建的进程也当成不可借用的提供者私有资源，导致命令未能打开信息对话框。旧包测试断言同步之后，该问题仍连续复现，不能归因于单次异步回执时序。通用 `Services::permits_cleanup` 回归先失败后通过（执行耗时 0.00 秒）：真实宿主命令可清理本实例早前资源，跨实例、嵌套委托、失效命令及已释放句柄仍拒绝；委托源清理自己分配资源的既有撤销规则保留。日志 `target/git-history-cleanup-unit-red.log`、`target/git-history-cleanup-unit-green.log`。真实定时器原场景修复后通过（137.31 秒，`target/git-history-layout-quiet-green.log`）：30 秒内不额外查询，无变化时保留已加载树页和场景，下一次后台读取被真实菜单命令取消，迟到结果不覆盖对话框，之后真实外部文件／分支变化仍自动观察并保持控件可用。
- 最终独立插件和根 `cargo fmt --check`、根 `git diff --check`、`cargo check --workspace --jobs 1` 均通过；`cargo test --workspace --exclude editor-app --jobs 2`：78 组、292 passed、308 ignored、0 failed，含新增通用清理权限回归。ignored 不计作通过。日志为 `target/git-history-layout-format-final.log`、`target/git-history-layout-diff-check.log`、`target/git-history-layout-workspace-check-final.log`、`target/git-history-layout-nonui-final.log`。期间有一次格式检查遇到并行侧栏改动的临时格式差异，本轮未代改这些文件；最后重复检查已通过。早期尚未加入清理权限修正时的 291 项记录保留在不带 final 的日志中。

## 候选身份与使用

- 插件 ZIP：`C:/Projects/RustProjects/Editor/target/git-history-layout-2026-10-19/git-0.1.12.zip`，809501 bytes，SHA-256 `66c54e3d60359c832c5bbbaa2549f4174e1c5fb4790c5daa53ac05f63f4b9e62`。独立源码仓库 `dist/0.1.12/` 同包留存。
- 包内 `git.wasm`：2489156 bytes，SHA-256 `1ead10f851e8c1e86f79d5299f717955a023929241a8562e90de1e50371bd868`；`manifest.json`：4123 bytes，SHA-256 `9a5245e63cfbc8bc18a80f865211a5642d6b5662bab2bd74eb20bd1907cd8fcb`。
- 两枚用户 SVG 及 `nanobug-git-rebase.exe` 摘要与 0.1.11 一致；包仍为五项，不附带插件 SDK。
- 源码：`C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.12/git-for-Nanobug-0.1.12-source.zip`，56 文件、166044 bytes，SHA-256 `59d37c1e07f5424a6f25e7a38e9dfc3d803d3c66be4b6080b6aa8658295c9069`；旁附文件清单和逐项摘要。
- 最终需使用本轮 `target/git-history-layout-2026-10-19/Nanobug.exe`，补充真实宿主命令清理早前后台资源的通用修正；118545408 bytes，SHA-256 `1e3d3d86aa662d9e233cc4f7bb56be8cb66ca268225973e5cb5c4e426011223c`。直接执行 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o C:/Projects/RustProjects/Editor/target/git-history-layout-2026-10-19/Nanobug.exe` 成功，原有编译／链接警告保留；不覆盖当前正在运行的默认 release 程序。以此宿主执行公开 `--plugin-cargo`，100 项访客测试再次通过（5.88 秒，`Git-for-Nanobug/target/history-layout-unit-new-host.log`）。通过配套宿主的插件管理器更新 ZIP；仅使用上轮宿主不能覆盖后台读取期间的菜单操作回归。

本轮未自动安装、修改用户配置、提交、推送或发布版本。宿主由当时工作区构建，保留先前及并行任务的改动；本记录只对本轮 Git 相关验证作结论。原生验收是完整面板的绘制与菜单输入回归，未重新执行全部窗口／中文 IME／显示缩放矩阵。
