# Base 控件边界与本地 UI 迁移

后续：[2026-10-21 统一 editor-ui 入口并清理锁文件](editor-ui-extraction-2026-10-21.md) 已替代本文关于允许保留弱可选组件条目的阶段性做法；本文保留当时的实现与验收证据。

日期：2026-10-19。状态：依赖边界和控件迁移已落实，针对性自动测试及 Windows 关键控件交互通过；全量应用测试保留下列 14 项失败，原生退出另有 AccessKit 错误，不宣称全仓或退出流程全绿。

## 依据与边界

用户要求所有 UI 使用 Kit 的 GPUI 原语和 `gpui-base`，允许参考 `gpui-component` 的行为与实现，但不依赖其组件或运行时。规则已写入仓库 [AGENTS.md](../../../AGENTS.md#文档状态与原生-ui)。图标继续使用与组件运行时独立的 Kit assets。

根 Cargo 依赖关闭 Kit 默认功能，仅显式启用 `assets`；测试仅追加 `test-support`。移除会重新启用组件层的 `tree-sitter-languages`，保留 0.7.0 版本。实际构建依赖树不再依赖 `gpui-component`、其宏 crate 及内建 grammar。本机 Cargo 的锁文件和 metadata 仍会列出弱可选依赖候选，不能据此判断它已启用；边界测试直接检查 `cargo tree` 的全部目标、全部应用功能及 normal/build/dev 边。

## 实现范围

- 按钮、输入、文本区、标签、滚动条、树、弹窗等继续由本地 UI 组合 Base 行为；移除生产源码和测试中的 `gpui_kit::component` 引用。
- 本地主题负责颜色、字体及 Base token 投影，Base Root 继续承担窗口内容、焦点与插件挂载。
- 文件树与源文档上下文菜单使用本地外观、Base 按钮及应用动作分发，保留子菜单、禁用项、窗口边界定位、键盘导航、外部点击拦截及原焦点恢复。
- 模态弹窗通过 Base Root 的本地 overlay 承载；Base 保留焦点陷阱、Escape/Enter 与业务回调的关闭否决。权限确认仍使用原来的授权、排队和生命周期入口。
- 停靠标题的本地类型擦除接缝仅投影标题与工具条，Base 继续拥有唯一布局、面板标识、资源生命周期、拖放与持久化。
- 源编辑器的查找/替换和代码操作使用本地 UI；查询、导航、文本效果及撤销仍交给原 Base EditorState，不另建文本或撤销状态。
- 语言高亮接入迁至本地插件 grammar 注册与解析模块，按项目限定键选择 WASM 工厂，保留注入目标白名单、增量解析、取消/超时和 Base 捕获样式接口。只读 Rope 快照不具有编辑权；没有宿主内建 grammar 回退。

## 验证记录

首轮测试修正了子菜单的绘制/点击边界及可访问选中状态、搜索浮层落在源编辑器下方的问题、权限弹窗的高度边界，以及整份文本快照误用旧解析树的问题。最后补齐本地按钮名称和输入框的原生焦点/值绑定，追加回归覆盖可访问名称、输入值和焦点。

| 检查 | 实际结果 | 本地日志 |
| --- | --- | --- |
| `cargo fmt --check`、`git diff --check` | exit0 | `target/local-ui-format.log` |
| 生产与测试源码检索 | 无 `gpui_kit::component`、`use gpui_component` 或 `extern crate gpui_component` 调用 | 全部 `crates/editor-app/src/` |
| `cargo tree --offline -p editor-app --all-features --target all --edges normal,build,dev --prefix none --format '{p}'` | 无组件运行时；自动边界测试通过 | `target/local-ui-cargo-tree.log` |
| `cargo check --workspace` | exit0 | `target/local-ui-workspace-check.log` |
| `cargo test --workspace --exclude editor-app` | 78 组，304 passed、0 failed、311 ignored | `target/local-ui-nonui-tests.log` |
| `cargo test -p editor-app ui:: -- --test-threads=1` | 最后一次共享控件变更后 140 passed、0 failed、36 ignored；含 IME、对齐、表单、主题、边界及 4 项源控件新增回归 | `target/local-ui-controls-tests.log` |
| 文件树本地菜单 | 10 passed、0 failed | `target/local-ui-explorer-menu-tests.log` |
| 语法诊断、XML 取消、语言包接入、权限确认 | 分别 7、1、6、1 passed；语言包组另有 3 ignored | `target/local-ui-{syntax,cancellation,languages,consent}-tests.log` |
| `cargo test -p editor-app -- --test-threads=1` | 一轮完整结果为 658 passed、14 failed、286 ignored；随后对最后的共享控件变更执行上述 140 项复验 | `target/local-ui-full-tests.log` |
| `cargo build -p editor-app --bin editor-app` | 最终 Debug EXE exit0，实际启动并完成下述交互 | `target/local-ui-native-final-build.log` |

ignored 项没有执行，不计为通过。编译保留 unused 与 Wasmtime/Tree-sitter 的链接警告。没有修改插件包、协议或 SDK，也没有为本次 UI 迁移重建与执行无关的 ignored 包测试。

## Windows 原生验收

通过 Computer Use 的真实鼠标、键盘与 Windows 截图核对，使用 `target/local-ui-native/profile` 独立配置和预置受限工作区，不启动项目工具，也不更改用户日常工作区。最终 EXE SHA-256 为 `9b40b8ab047dddc146b5bb701886564f65a10a89d5f0b6e8bf0e72cb9106fb03`。

- 打开真实文本和中文文件名，正常显示行号、标签与文本；中文查找显示 `1/1`，焦点和值由原生可访问树读回。
- 中文替换后可见源文本变化，Escape 回到源焦点，Ctrl+Z 还原；保存独立验证文件后脏标记消失。
- 文件树右键菜单、“新建”子菜单、父子选中与 Escape 关闭通过；源动作菜单键盘 Down 跳过禁用项，保留原动作分发范围。
- 深浅主题切换后正文、菜单、侧栏和边框正常。设置中字号从 14 px 调到 15 px，源字形、行高和界面同步缩放；设置关闭后查询值保留，关闭查找后恢复源焦点。
- 本地设置窗口和搜索面板保持在可见范围内。自动回归另外验证权限确认页的取消/确认按钮可见及关闭否决，不执行真实安装。

未交叉构建或原生验收 macOS/Linux；本轮的中文原生输入不等同于完整 Windows 输入法候选窗口验收，IME preedit、替换与取消由现有自动回归覆盖。

关闭验证窗口后读取 stderr，记录到 `accesskit_windows 0.34.0/src/util.rs:380` 对无效窗口句柄 `HRESULT(0x80070578)` 的 unwrap panic，随后出现 cannot-unwind。首轮与最终轮均在关闭与自动化读取窗口状态并发时出现，日志为 `target/local-ui-native/stderr-restricted.log`、`target/local-ui-native/stderr-final.log`。尚未验证没有辅助工具并发读取时是否发生，也没有迁移前的等价原生基线，不将退出流程报告为通过或断言它与本轮无关；本轮未修改或 fork GPUI/AccessKit 依赖。

## 全量应用测试的剩余失败

14 项分别为语言服务夹具/状态/定义跳转 6 项、宿主消息 5 项、原生视图保留断言 1 项、旧包详情范围控件断言 1 项、worker 错误归属文案 1 项；确切函数名和断言在完整日志中。

其中语言服务/状态/跳转和 worker 的历史失败已记录于 [项目入口验收](project-menu-startup-2026-10-14.md)；旧包详情范围断言也在迁移前的历史 Debug EXE 中复现于同一行。当前工作区既有 `extensions/native_controls.rs::hide` 明确保留 Base 实体并撤销场景，相关测试仍要求 `native_ui.is_none()`，本轮保留现有实现。没有完整保存本轮开始前的可执行源码快照，不能把全部 14 项都笼统归因为基线，也未为消除这些失败扩大修改范围。

本轮开始前工作区已有大量修改；它们不属于本轮成果，本记录不把既有工单和其他功能一并标记完成。未提交、推送或关闭议题。

返回 [UI 目录](../README.md)。
