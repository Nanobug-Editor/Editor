# Git 工作台分支与当前工作区整合主分支

日期：2026-10-18。依据：用户要求切换到主分支并合并所有修改。状态：全部冲突已处理，合并后规定检查、真实 WASM 与定向 UI／原生回归通过，本地 main 整合完成，未推送。

19:41 用户追加 Release 启动失败：本记录的编译检查与测试不包含 `cargo build --release`，不能替代 IDE 的 Release 启动验收。现已复现同样的 111 个错误，按实际 profile 修复缓存并验证原入口；见 [Release 启动复查](build-cache-release-2026-10-18.md)。下列历史命令与结果保留。

## 范围与保留

原分支 `codex/git-plugin-workbench` 为 `910e23266a7f8a643509de34233c1d85d1bc2f04`，较本地 main 多 8 个提交。当前工作区 249 个已修改／未跟踪文件按明确路径纳入普通提交 `8bec548c28cc8162cb7deeae2bef537b88cac26c`，保存项目入口、预览标签、问题面板、插件管理与 Git 通用界面修复、测试及文档。提交树 `d44cb4b737966ab0ac25a456ff0d21cb0d4a8433` 与提交前 index 树一致；没有将被忽略的 target、用户 profile 或构建产物纳入版本控制。

保存前仅修正 `extensions/preview/toolbar.rs` 与 `extensions/surface.rs` 两处已有格式差异，未改变行为。此前[启动缓存修复记录](build-cache-2026-10-18.md)中的格式失败属于修正前的实际结果。

切换到 main 后，先以 `--ff-only` 纳入 origin/main 的 5 个提交，到 `005f6a8`，再普通合并已保存的 Git 工作台分支。保留双方原有提交，不重写历史。本次只整合当前仓库的分支与工作区；独立 Git 插件源码和既有 ZIP 不变，未推送远端、关闭议题或发布版本。

## 冲突处理

9 个文件存在冲突，逐项处理，不以整边覆盖替代整合：

- Markdown 测试继续使用 main 已交付的 `markdown-0.18.2-readme-pr.zip`；旧本地候选也保留在完整摘要账本中。
- 通用插件 UI 同时保留原生内容样式和 `form_delivery` 的草稿交付／回执能力。
- 视图发布使用 Git 工作台分支的通用 `required_capabilities()` 最低版本检查，覆盖 main 的 `ui.styles` 检查及其他公开能力。
- 样式 WASM 测试保留 main 对真实访客视图的验证，只改变声明，不用数据文件替代真实访客输出。
- 文档保留 Git／Vue／插件管理的新增记录，以及 main 的 Nanobug-Editor 组织入口与 Markdown 交付复验记录。

冲突处理后相对保存提交有 18 个文件不同，对应远端组织迁移、Markdown 夹具及复验记录；本轮整合说明与目录入口另外添加。249 个原改动文件仍在，54 个 Package 账本条目的实际 SHA-256 均一致。原生界面既有交付证据见[共享侧栏验收](../../plugins/verification/git-sidebar-panel-2026-10-18.md)和[Markdown 复验](../../plugins/verification/markdown-github-readme-presentation.md)，不将其计为本轮重新执行。

## 验证

初始合并 index 树为 `ea206c302cec972880a031b62b46281242ad07c5`。`git diff --cached --check`、`cargo fmt --check` 与 `cargo check --workspace --jobs 1` 已通过；首次非 UI 全仓测试为 289 passed／0 failed／305 ignored，78 组，见 `target/merge-main-nonui-tests.log`。

额外显式执行真实 WASM 样式协商用例，初次失败：旧测试把 `Environment::default()` 当成项目上下文，当前空环境为应用级管理，只接纳工作区包的元数据，因此安装并不触发工作区视图验证。测试补充真实临时项目路径，继续验证未协商样式时的 Prepare 拒绝与已协商时的真实 View demand；没有修改产品实现、绕过协议或替换访客。重跑 1 passed／0 failed／0 ignored，编译 6.37 秒、执行 34.61 秒。失败与通过日志分别保留为 `target/merge-main-styles-wasm-test.log` 和 `target/merge-main-styles-wasm-retest.log`。

原生 UI 首轮表单交付、内容样式和图片尺寸回归通过。会话组中 `plugin_dock_layout_survives_delayed_startup` 在默认及单线程均于测试准备阶段失败：它无条件切换右侧栏，并假定操作后为隐藏；此前加入的“消息默认隐藏”让该前提失效。此测试及相关产品源码相对保存提交没有变化。修正测试为仅在右侧栏已打开时关闭它，继续保留保存尺寸、插件延迟加载和重启后布局相同的断言；没有修改产品的默认显隐或恢复行为。初始与单线程失败日志为 `target/merge-main-ui-session.log` 和 `target/merge-main-ui-session-serial.log`。

后续 UI 使用仓库既有的单线程方式执行。最终结果：

| 检查／回归 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --check` | 通过；含本轮测试改动的格式修正 | `target/merge-main-format-complete.log` |
| `cargo check --workspace --jobs 1` | 通过 | `target/merge-main-workspace-check.log` |
| `cargo test --workspace --exclude editor-app --jobs 1` | 289 passed／0 failed／305 ignored，78 组 | `target/merge-main-nonui-tests-final.log`、`target/merge-main-nonui-final-totals.json` |
| 显式 WASM 样式协商 | 1 passed／0 failed／0 ignored | `target/merge-main-styles-wasm-retest.log` |
| 表单交付、内容样式、图片尺寸 | 8＋1＋1 passed；表单另有 1 ignored 未执行 | `target/merge-main-ui-{form-delivery,content-styles,image-dimensions}.log` |
| 会话、插件停靠、本地停靠 | 8＋2＋5 passed | `target/merge-main-ui-{session,plugin-dock,local-dock}-final.log` |
| 预览标签、项目入口 | 9＋12 passed | `target/merge-main-ui-{preview-tabs,project-home}-final.log` |
| 插件操作进度、关闭失败恢复 | 1＋1 passed | `target/merge-main-ui-plugin-progress-exact.log`、`target/merge-main-ui-close-error-final.log` |
| 真实鼠标标题栏回归 | 父 1 passed，3.40 秒；最小化、最大化／还原、关闭三个私有子进程均 exit0 | `target/merge-main-native-caption.log` 与 `target/merge-main-native-caption/{minimize,maximize,close}.log` |
| 显式 Markdown 原生 HTML | 3 passed／0 failed／0 ignored，150.52 秒；真实夹具的链接、未保存源文本、图片尺寸和导航 | `target/merge-main-ui-markdown-html-ignored.log` |

UI／原生合计实际执行 52 项通过，未执行完整 915 项应用测试，未将其他历史 UI 失败认定为已修复。全仓常规命令的 305 ignored 不计为通过，其中样式 WASM 用例已另外显式通过。最初 `plugin_manager_progress` 过滤词未匹配任何测试，随后以真实函数名执行为 1 passed；Markdown 默认命令的 3 ignored 也已显式重新执行，不能用 exit0 或空过滤冒充验证。

保留既有未使用代码及 Wasmtime／Tree-sitter LNK4217 警告。最后核对 source/index 无遗漏、无冲突标记，记录和目录的相对链接以及 `git diff --check` 通过。最终合并只额外修正两处测试准备前提；产品行为保留双方实现。

返回[文档总目录](../../README.md)。
