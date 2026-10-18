# 标题栏应用菜单调整

日期：2026-10-14。状态：代码、相关交互测试、整体检查及 Windows 原生窗口验收通过。

本文保留此前五项应用菜单的历史交付记录。当前项目标题、菜单名称及入口移除以[项目切换菜单后续验收](project-switcher.md)为准：“打开文件夹”更名“打开新项目”，“项目首页”更名“项目管理”，移除“关闭项目”。

## 交付范围

编辑器左上角的品牌图标替换为可点击的菜单图标，保留 Nanobug 名称。截图框选的操作从标题栏迁入下拉菜单，按以下三组排列并用两条分隔线区分：

1. 打开文件夹、项目首页、关闭项目。
2. 快捷键。原键盘按钮和省略号菜单原本通向同一面板，合并为一个入口。
3. 退出应用。

复用本地 Base 按钮、SVG 图标和 `PopupMenu`，沿用中英文资源与主题 token。菜单先恢复原焦点并关闭自身，再进入既有项目路由、关闭确认和应用退出协调器；快捷键面板继续保留打开前的原面板上下文。运行、插件、设置及原生窗口按钮保留原位置。

## 验证对象与结果

基础提交为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`。主工作区同时存在其他任务的插件和运行工具栏改动，因此在已附加的工作区 `C:/Users/Tmiracle/.codex/worktrees/project-home/Editor`、分支 `codex/application-menu` 验证本次增量。十个相关 Rust 文件已按 Git 内容摘要核对一致，其他任务的文件未迁入验证副本。本次未提交或推送。

新增 GPUI 场景首先因缺少左上角菜单入口失败，完成实现后通过。原有项目生命周期场景改从实际菜单进入，复用已有的多窗口、未保存内容、保存失败与退出取消覆盖，避免重建同类测试。

| 验证 | 结果 |
| --- | --- |
| 项目首页与多窗口 GPUI 场景 | 22 passed、0 failed、3 ignored |
| 快捷键查询、绑定和面板焦点 | 11 passed、0 failed、2 ignored |
| 图标资源 | 4 passed、0 failed |
| 通用菜单的定位、键盘与滚动 | 4 passed、0 failed |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked -j 1` | 267 passed、0 failed、255 ignored |
| `cargo check --workspace --locked -j 1` | 通过，锚点修正后再次通过 |
| Windows 原生窗口 | 菜单位置、分组、深浅主题、快捷键入口、Esc、编辑焦点及菜单退出通过 |

上述 41 个相关测试来自锚点修正后的同一冻结程序，SHA-256 为 `5418243729f14254231f241a2d49d0812d8dd3a451c1d4050c77e06c1fffc47c`。五个 ignored 场景需要真实 WASM 包和运行时，本次菜单调整未重跑，不能计为通过。非 UI workspace 的 255 个 ignored 也未计为通过；本次没有重跑全部应用测试。

共享构建目录曾返回不含新增菜单场景的旧程序，相关输出不作为本次通过依据。最终使用 `cargo test -p editor-app --locked -j 1 --no-run --config 'profile.test.package.editor-app.debug=1' --message-format=json` 为应用包生成独立调试配置的产物，核对来源与新增测试名称，再冻结并串行执行上述四组场景。此配置仅用于本次命令，没有修改 Cargo 清单或用户环境。

原工作区的一次快捷键测试编译失败涉及其他任务中尚未完成的 `run_ui_tests::toolbar` 文件和 `ClipboardItem` 导入；隔离副本首次并行运行还观察到一次旧首页路径选择场景的 `no pending paths prompt`。这些尝试不替代最终冻结程序的结果，也未扩大为本次菜单之外的修复。

日志及产物保留在 `C:/Users/Tmiracle/.codex/visualizations/2026/10/12/01a12e28-69b5-7ea2-b14e-f155ef6bf763/`，最终相关日志为 `application-menu-verified-{projects,shortcuts,assets,popup}.log`，构建来源为 `application-menu-test-artifacts.jsonl`。

## Windows 原生验收

最终程序 SHA-256 为 `e0a8e6686fcaae2be8e08d63bc5ff080db6f71d0a764fa4b578cc80b7a0af8b6`。使用独立的既有验收 profile 和先前由用户选择受限模式的项目 A，没有操作用户正在运行的主目录程序或项目信任设置。

首次实际窗口发现测量 canvas 的静态位置使菜单额外下移。新增锚点断言先观察到 `52.5px` 与标题栏下沿 `33px` 的差异；为测量层指定四边零偏移，并以完整标题栏高度对齐后，断言及四组相关测试通过。实际菜单紧接标题栏下沿展开，三组操作完整可见，分隔线在两种主题下清晰。

点击菜单中的快捷键入口打开原面板，Esc 关闭后，Ctrl+A 实际选中原文档文本，确认焦点恢复。深色菜单 Esc 关闭后恢复浅色，再通过菜单退出验收程序并读回窗口和进程均已退出。本次没有通过原生输入修改验收文档文本。

实际效果：[浅色菜单](assets/application-menu-light.jpg)、[深色菜单](assets/application-menu-dark.jpg)。这些图片来自实际运行程序，没有以原型或生成图片代替验收。
