# 项目标题与项目切换菜单

日期：2026-10-14。状态：本次范围实现、交互测试、双轴增量审查及 Windows 原生验收完成；完整 UI 保留 7 个既有基线失败。未提交、推送或发布。

后续[Windows 路径显示修正](windows-path-display.md)已将项目、标题与提示中的扩展路径前缀隐藏。本文保留初次交付的原始截图与验证结果；当前路径显示以该后续记录为准。

## 已确认行为

- 编辑窗口左上保留独立应用菜单，静态品牌文字替换为文件夹名称与下拉图标；名称及图标均属于同一可点击区域，长名称省略并保留完整路径提示。
- 项目下拉分为“已打开项目”和“最近项目”。前者包含当前项目并显示勾选，点击激活对应系统窗口；后者排除已打开项目，点击通过既有流程在新窗口打开。
- 每项显示项目名称与路径，过长时省略、悬停显示完整内容。各组按成功打开顺序排列，普通窗口切换不重排；菜单限制高度但保留全部可滚动记录，不增加搜索或移除入口。
- 应用菜单为“打开新项目”“项目管理”“快捷键”“退出应用”，按项目、快捷键、退出三组排列；移除“关闭项目”菜单入口。
- “打开新项目”继续选择现有文件夹。“项目管理”打开或激活独立的既有首页，不替换编辑窗口。无项目时左上显示“选择文件夹...”，点击原生选择器，选定后在当前首页窗口打开。
- 最近项目沿用本机持久记录；管理页移除记录不关闭已有窗口，关闭后不重新加入已移除记录。未知信任、目录失效、取消及会话恢复沿用[现行项目窗口规格](../specs/project-windows.md)。

## 实现与验证接缝

复用已确认的应用级项目生命周期接缝：真实 GPUI 首页/项目窗口、菜单点击、原生选择器与统一打开/关闭流程。窗口登记只额外记录已激活项目的显示路径与成功打开顺序，信任预占窗口不作为已打开项目；没有复制文档、撤销或运行状态。

两行内容与分组标题由本地 `PopupMenu` 绘制，继续使用 Base 按钮及已有键盘、滚动和焦点行为，不修改插件 UI 协议。菜单通过稳定路径身份保留选择，窗口或最近记录变化时刷新，并将变化后的选中项滚入视野；点击再次检查活跃窗口，已关闭目标进入普通打开流程。首次不存在的 profile 按最近的已存在祖先归一化，避免保存创建目录后将窗口与历史记录分成两份权威。

六个新增测试覆盖首页标题选择/取消、最近项目新窗口打开、活跃窗口跳转与未保存文本、下拉展开期间关闭另一窗口、移除已打开记录后关闭、24 条历史记录的滚动/End 导航、Esc 焦点恢复、首次 profile 创建及动态移除/跨组移动后的选中项可见性。既有关闭项目回首页的会话场景继续调用原有生产关闭所有者；实际应用菜单测试单独验证入口移除、快捷键焦点及原生关闭窗口的未保存保护，不以测试旁路假装菜单仍有关闭入口。

## 验证对象与结果

起点为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`，保留工作区原有应用菜单前置改动。项目、菜单和原生程序在管理的隔离副本 `C:/Users/Tmiracle/.codex/worktrees/project-switcher/Editor` 验证，使用独立 `target/project-switcher` 缓存。15 个相关源码/资源文件与主工作区逐项 SHA-256 相同；`main.rs`、`app/shell.rs` 中本次字段、初始化及渲染接缝相同，副本没有混入并发问题面板改动。完整性记录为 `project-switcher-source-manifest.json`。

首页标题、项目路由、菜单移除均先观察到真实失败再实现。后续回归还捕获并修正空项目没有文档焦点时菜单动作无法路由、首次 profile 创建后共享键变化、长列表动态更新后的高亮不可见。最后一项失败时绘制标签位于 `y=-768.5px`；修正后移除与跨组移动两种交互均通过。

共享构建目录曾返回未包含新增场景的旧测试程序，该输出不作为通过证据。最终以以下命令取得 Cargo 返回的测试程序路径，核对六个新增测试名称，复制冻结程序后执行相关过滤及全量应用测试；没有修改 Cargo 清单或升级依赖。

```powershell
cargo test -p editor-app --locked -j 1 --target-dir C:\Projects\RustProjects\Editor\target\project-switcher --config 'profile.test.package.editor-app.debug=1' --config 'profile.test.package.plugin-protocol.debug=1' --no-run --message-format=json
```

冻结测试程序 SHA-256 为 `9d6b8df1dae3a0d4bbfe98d849e202a7516c40ae4492ef7b80280e1d36399804`。

| 验证 | 结果 | 证据 |
| --- | --- | --- |
| `cargo test -p editor-app project_switcher_ ... -- --test-threads=1` 的逐项红绿回归及最终冻结程序 `app::projects::tests --test-threads=1` | 28 passed、0 failed、3 ignored；包含六个新增场景及项目窗口生命周期 | `project-switcher-verified-projects.log`、`project-switcher-{profile,scroll}-{red,green}.log` |
| 同一冻结程序全量 `--test-threads=1` | 541 passed、7 failed、230 ignored；152.83 秒 | `project-switcher-app-tests.log` |
| 通用 PopupMenu 及快捷键 UI | 4 个菜单、11 个快捷键 UI 场景全部通过，已包含于全量结果 | 同上 |
| `cargo fmt --check` | 主工作区、隔离副本均通过 | `project-switcher-fmt-{root,isolated}.log` |
| `cargo test --workspace --exclude editor-app --locked -j 1` | 主工作区 268 passed、0 failed、256 ignored | `project-switcher-workspace-tests.log` |
| `cargo check --workspace --locked -j 1` | 主工作区通过，保留原有警告 | `project-switcher-workspace-check.log` |
| `git diff --check` | 主工作区、隔离副本均通过 | 实际命令输出 |
| Windows 原生窗口 | 标题、分组、深浅主题、窗口打开/跳转、键盘、中文文本、原生目录选择及退出通过 | 下述原生验收及截图 |

完整 UI 的七个失败名称与[现有干净主分支及项目窗口基线](project-windows-main-integration.md)完全一致：语言状态 popover、Rust 超时、状态指示器三个用例，Ctrl/中键定义导航三个用例，以及 Worker 命令归属错误文案用例。没有将全量 UI 记为通过，也没有扩大范围修改这些既有问题。早期主工作区曾遇到并发问题面板的 8 处编译错误及格式差异，未归入本次修复；最终主工作区格式检查和 workspace 编译均已通过。

本轮不涉及安装器、协议、真实 WASM 包或服务生命周期变更；对应 ignored 测试未运行时不计为通过。

## Windows 原生验收

最终原生程序 SHA-256 为 `81c61b25ec8faf12bed22a07cef5f0a0d8bf35f4282b088dc821cfcfc3faf083`，来自同一隔离源码的 Cargo 直接构建。使用独立 profile、中文/空格路径和已保存受限模式的三个测试项目，没有操作日常 profile 或信任提示。

从 A 的“最近项目”点击 B，系统窗口由一个变成两个；从 B 点击“已打开项目”中的 A，返回原窗口 ID，窗口数量保持两个。分组立即更新，当前项目勾选正确，普通激活不改变 B、A 的成功打开顺序。名称及路径按两行展示，长路径省略，菜单紧接标题栏展开；深浅主题均通过。

菜单 Esc 后 Ctrl+A 实际选中文档，输入“菜单验收 中文输入”得到未保存状态。通过鼠标切到 B，再以方向键与 Enter 回到 A，文本和未保存标记完整保留。撤销并保存恢复原文后结束编辑验收。原生文字输入及已有 GPUI IME 回归通过，本轮未单独操作系统 IME 的候选窗。

应用菜单实际显示“打开新项目”“项目管理”“快捷键”“退出应用”，没有关闭项目入口。“项目管理”打开独立首页；点击其“选择文件夹...”标题调用 Windows 目录选择器，取消保留首页，重新选定测试目录后在同一窗口 ID 进入项目。没有打开文档的空项目也能再次通过应用菜单打开独立管理页。最后经应用退出流程关闭四个验收窗口，读回窗口清单为空、验收进程退出。

实际截图：[浅色项目菜单](assets/project-switcher-light.jpg)、[深色项目菜单](assets/project-switcher-dark.jpg)、[应用菜单](assets/project-switcher-application-menu.jpg)、[首页标题](assets/project-switcher-home.jpg)。截图来自实际运行程序。

## Standards

规范子智能体的最终复审发现 0 项硬性违反、0 项有实际影响的坏味道。确认窗口/profile 共享权威、既有生产所有者与本地 Base 控件复用，注释和可观察交互回归同步，未混入并发任务改动。

## Spec

规格子智能体首次报告 1 项 P2：动态列表变化后高亮可能留在可视区域外。修复并由真实绘制边界回归验证后，复审报告缺失/部分需求 0、范围蔓延 0、实现错误 0；首次 profile 和空项目菜单修正符合既有窗口所有权及统一打开流程。

两轴结论均针对任务开始前快照到最终实现的未提交增量。因未获提交授权，`git diff <起点>...HEAD` 与提交列表为空，不能宣称完成完整提交差异审查；改用任务前快照的明确增量进行静态复审。各轴均无遗留发现，最严重的原规格 P2 已关闭。

全部日志、Cargo 产物来源、任务前快照、复审补丁及冻结程序保留在 `C:/Users/Tmiracle/.codex/visualizations/2026/10/14/01a13876-e6ea-7f53-a781-938a62fad9eb/`。隔离验证副本已归档为本任务可恢复的工作区快照。自动审批以 `blocked by policy` 拦截临时缓存的递归清理，`target/project-switcher` 仍保留。本次没有提交、推送、关闭议题或发布。
