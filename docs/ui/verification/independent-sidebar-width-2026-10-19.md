# 左栏独立宽度与右栏显示

用户确认的行为：打开右侧内部窗口时，左栏宽度及其内部组合保持不变，只从中间编辑区让出空间；关闭右栏后空间返回编辑区。左栏手动调整后的宽度同样保持。该规则适用于普通组合及带 `exclusive` 的单独占据窗口。

## 实现

默认资源管理器和大纲从 Center 比例分割移入 GPUI Base 的 Left dock，编辑区作为 Center 的弹性区域。右栏继续使用 Right dock。注册左侧插件时仍加入现有左栏，避免新增水平列；显式拖到编辑区左边缘时，在编辑器所在行新增预览对应的局部列，底部横跨该列与编辑器，见[层级与跨度修正](editor-row-drop-2026-10-19.md)。所有持久布局、分割交互和大小约束仍由同一个 DockArea 维护。

恢复历史会话时，将中心水平树中编辑器之前的侧栏子树整体移入 Left，包括多层水平祖先中的旧侧列。原有默认插件迁移未执行的会话先完成该迁移，再转换整个侧栏。保留顺序、内部高度偏好、实测宽度和现有实体；已有自定义外侧列按原顺序组合，空 dock 不占额外宽度。原来关闭的外侧列在创建实体前转为各叶隐藏，避免合并时意外展开；其宽度不加入当前可见左栏。Center 中局部上下布局继续保留。

隐藏侧栏内全部宿主窗口时，关闭区域并保留隐藏叶；重新打开资源管理器、大纲或插件时，通过统一的公开 DockArea 路径重新打开实际所在区域。设置窗口里的资源管理器按钮回到编辑器所属 HWND 执行停靠编辑。恢复时先转换旧树，再添加缺失的新窗口，避免将尚未绘制的临时分割测量固化进会话。

原生底部 dock 位于 Center 下方，原生左右侧栏贯穿停靠区域高度。拖入 Center 的局部列只占所在行；右栏打开时，这些工具列同样保留像素宽度，由编辑器分支让出空间。历史版本 0/1 的侧栏树仍转换一次，版本 2 起保留显式拖放后的行列关系。极窄窗口继续使用 Base 的边界约束，本次没有新增自由停靠或第二套布局状态。

## 验证

修改前，通过真实底部按钮打开右栏可复现左栏从 198px 缩到 148.5px，日志 `target/right-width-before.log`。改为独立侧栏后，最初的开关回归通过，日志 `target/right-width-after.log`。

补测曾暴露两个相关问题：原生 Left 被清空时会丢弃隐藏大纲；在添加缺失大纲之后二次加载临时 dump 会将旧组合变成极不均衡分割。已分别改为保留侧边宿主叶、在追加缺失窗口前转换旧树。旧测试夹具也改为先移除当前 Left，避免同时在 Center/Left 重复注册同一个宿主实体；仍逐字段比较完整保存布局与尺寸，没有放宽尺寸断言。

首次修正复验 `cargo test -p editor-app dock --jobs 1 -- --test-threads=1` 为 16 passed、0 failed、2 ignored，日志 `target/right-width-dock-verified.log`。侧栏专项 9 passed、1 ignored，单独占据三组 3 passed，日志分别为 `target/right-width-sidebar-tests.log`、`target/right-width-exclusive-tests.log`。最终追加回归结果继续记录于下文。

实际 XML 0.2.2 固定包（账本摘要 `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a`）进入公开 Manager，显式运行四方向大纲拖动、分隔线调整及工作区恢复：1 passed，82.49 秒，日志 `target/right-width-outline-docking.log`。该项由 TestAppContext 驱动实际插件和生产渲染器，不作为 Windows/D3D 原生验收替代。

Windows 原生首轮复验使用冻结 runner `target/right-width-native-runner/editor_app.exe`，SHA-256 `df7742f8f63e86aa5f317b5b93304fe2c9f0193cd47e2e9721ef79b594426b89`。沿用独占属性专项的隔离 Git 测试 ZIP，未改动用户安装；父测试 1 passed、125.24 秒。深色中文 100% 与浅色英文 150% 两个实际 D3D11 进程分别检查普通组合和独占暂存窗口，两种模式开关右栏均保持左侧实测 280 个逻辑像素；`right_width_shared`、`right_width_exclusive`、`exclusive_complete` 与 `shutdown_complete` 均为成功。日志 `target/right-width-native.log`，截图及证据 `target/right-width-native/{dark-zh-CN-100,light-en-150}`。已逐图核对深色独占打开/关闭右栏、浅色普通组合打开右栏三张图。

工作区门禁：`cargo test --workspace --exclude editor-app --jobs 1` 为 78 组、292 passed、0 failed、308 ignored；`cargo check --workspace --jobs 1` exit0。日志 `target/right-width-nonui.log`、`target/right-width-workspace-check.log`。308 项 ignored 未执行，不计为通过；上述 XML 与原生专项为另行显式执行结果。本次 UI 源码之外的已有工作区变更继续保留。

最终源码复查时，一次重链报 `LNK1104` 无法写入默认测试 EXE，日志 `target/right-width-final-dock.log`；尝试 `cargo rustc --bin editor-app --tests` 指定额外文件名时，Cargo 因选中多个编译目标拒绝参数，未产生候选产物，日志 `target/right-width-owned-runner-build.log`。随后按同一个 `cargo test -p editor-app dock --jobs 1 -- --test-threads=1` 入口重新执行成功，16 passed、0 failed、2 ignored，日志 `target/right-width-final-retry.log`。没有终止其他进程或移动正在使用的产物。

最后增加“全部隐藏后由插件入口重新打开独占叶”的可见回归，`cargo test -p editor-app exclusive_tests --jobs 1 -- --test-threads=1` 为 3 passed，日志 `target/right-width-final-exclusive.log`。冻结此轮完整测试程序到独立路径 `target/right-width-final-runner/editor_app.exe`，SHA-256 `40b04c3610cc4ca064f5aaf79fa02cee87ec919a94fd38620712eb1a71d68be9`；随后侧栏专项 10 passed／1 ignored，停靠回归 16 passed／2 ignored，日志 `target/right-width-sidebar-final.log`、`target/right-width-dock-frozen.log`。工具包专项 0 passed／2 ignored（未显式运行），不能计为通过，日志 `target/right-width-tools-final.log`。

该最终冻结程序再次显式执行 XML 四方向拖动及重启恢复，1 passed、78.73 秒（`target/right-width-outline-final.log`）；Windows 两组合父测试 1 passed、119.60 秒（`target/right-width-native-final.log`）。最终证据 `target/right-width-native-final/{dark-zh-CN-100,light-en-150}` 两组均为 `exclusive-dock-only`，普通／独占左栏均保持实测 280 逻辑像素，开关右栏成功；`shutdown_complete:true`、项目关闭成功、`unexpected_error_logs:[]`。已逐图核对最终深色独占和浅色普通组合打开右栏的截图，当前目标与初轮一致。

最终 `cargo check --workspace --jobs 1` exit0（`target/right-width-workspace-final.log`），`cargo fmt --check`、`git diff --check` 与本次修改文档的本地链接校验均通过。没有生成安装包；未运行完整 UI 测试集或原生完整八组合，OS IME 候选不在本次布局验收范围。

返回[原生 UI 目录](../README.md)。
