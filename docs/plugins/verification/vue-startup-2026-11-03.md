# Vue 大项目启动、定义跳转与故障后关闭

日期：2026-11-03。本轮修复已完成下述检查与行为验收；本地合并结果另存各仓库的任务记录，未推送或发布。

## 输入与范围

- 宿主任务分支 `codex/vue-startup-fix-20261103`，独立 Vue 包 0.2.1。
- 用户指定的现有 Vue 2.6.10 JavaScript 项目，先验证 `src/App.vue` 与 `src/main.js`。锁文件 706651 字节，1218 个 Vue 文件；项目源文件、依赖和锁文件均不修改。
- Vue 使用自身的语言进程，JavaScript 0.4.1 独立处理 `.js`；指定项目验收不安装 TypeScript 插件。JavaScript 复用 TypeScript 仓库的原生运行时 0.1.18；对应 TypeScript 包同步为 0.1.20。
- Node 24.19.0，TypeScript 5.9.3；legacy 适配器 0.1.13，modern 0.1.12。Vue 2.6.10 的新增支持仅限本次验证操作，不继承 2.6.14 的全部写法矩阵。
- 日志与截图保存于项目外 `C:/Users/Tmiracle/.codex/task-artifacts/vue-startup-fix-20261103/`。

## 确认的原因与修改

1. 原 `ReadFile` 把锁文件字节编码为 JSON 整数数组，客体在反序列化响应时耗尽 fuel，尚未到达插件的文件长度检查。新增面向所有插件的 `workspace.files 1.5` / `ReadWorkspaceText`，返回严格 UTF-8 字符串；保留 1 MiB 磁盘、工作区授权、路径和响应预算，不增加执行限额。Vue 元数据改用这个接口。
2. 原兼容表缺少准确的 2.6.10 记录。新增明确的 legacy 候选，继续拒绝缺失/冲突证据，不将任意 2.6 补丁号当成已验证版本。
3. 适配器为每个普通 SFC 保存空诊断快照，第 1025 个文件耗尽 1024 条状态限额，导致 adapter factory 失败。空结果不占槽，修复语法后删除旧警告；真实警告的上限与过期源版本校验保留。
4. 项目关闭会再次向已退役的暂停实例发送 Snapshot，必然得到 `Plugin is paused`。检查点现在保留这种实例的上一次成功磁盘快照并继续退役；仍可重试的客体拒绝、磁盘保存失败及原生资源清理失败继续返回错误。
5. 宿主 `ProjectWindow` 不再插入顶部错误行，也不往确认框正文追加操作错误。项目打开、关闭、恢复和最近项目持久化错误统一使用已有本地消息控件的可关闭弹窗，不挤压编辑区或移动输入焦点。
6. `main.js` 的 `@/App` 省略了 `.vue`，JavaScript 编译器原本只返回不存在的路径占位。共享原生运行时仅在静态 import/export 没有真实源码定义时，按选定编译器的继承路径配置查找有界、无歧义的真实文件；保留未保存快照和规范化工作区边界，不制造语义符号。普通字符串、缺失文件和同名扩展冲突仍返回空结果。

## 已执行的反馈循环

| 场景 | 修复前 | 当前已取得结果 |
| --- | --- | --- |
| 真实旧 Vue 包与大锁文件 | `all fuel consumed`，`large-lock-old-red.log`；桌面 `app-vue-startup-red.png` | 新传输完整读取锁文件；政策矩阵 2 项通过，含候选/冲突证据保留 |
| 1218 个普通 SFC / 已消失警告 | 两项均触发 `Extension status snapshot quota exceeded`，`extension-status-red.log` | 两项通过，`extension-status-green.log` |
| App.vue 自动启动与 import | 新文本传输暴露了准确版本未支持，再暴露空诊断限额；失败记录保留 | `project-entry-cached-green.log` 两项通过、0 跳过。打开文件后在任何定义请求前等待 active；App.vue 跳到 ant-design-vue locale，main.js 的 `@/App` 跳到 `src/App.vue`，原文件字节不变 |
| main.js 的组件入口 | JavaScript 0.4.0 返回空目标，`project-entry-import.log` | JavaScript 0.4.1 的源码回归及真实宿主回归通过，桌面 F12 同样打开 App.vue；`main-js-autostart-green.jpg`、`main-js-f12-green.jpg` |
| 暂停后关闭 | 冻结真实 WASM `fault-spin` 后，checked close 得到相同 `Plugin is paused`；`paused-close-red.log` | `paused-close-green.log` 2 项通过、0 跳过；覆盖当前项目、应用与停放作用域，保留旧快照、释放锁且可重新打开 |
| 宿主错误呈现 | 原生测试检测到 `project-error` 顶部行；`project-popup-red-retry.log` | `project-popup-green.log` 47 项通过、6 项原有忽略；包括弹窗、编辑区尺寸不变、可关闭，以及真正持久化错误仍可重试。普通桌面窗口显示右下角消息弹窗，关闭按钮有效，截图 `host-error-popup-green.jpg` |
| 源码 / 密封导出语义套件 | 首次运行缺少 worktree 的 `target/`，34 项准备失败，保留原日志 | 创建该目录后各 110 项通过、0 跳过；`vue-source-green.log`、`vue-sealed-green.log` |

原生回归先因本任务旧桌面占用 `editor-app.exe` 导致 Windows `os error 5`；关闭该隔离实例后重复同一 Cargo 入口，成功运行并获得顶部行的红灯，不将构建失败算作回归红灯。

最后一轮真实项目回归首次遭遇外部依赖下载超时：`project-entry-import-green.log` 中 App.vue 未完成安装、main.js 成功。随后用显式 `NANOBUG_TEST_DEPENDENCY_SEED` 复制先前已准备的不可变依赖缓存，仍经过公开包安装和权限检查，两个文件均通过；不复制注册表、授权或可写实例状态，不修改用户项目。

## 最终检查与交互

- `cargo fmt --check`、`cargo check --workspace` 通过；`cargo test --workspace --exclude editor-app` 为 889 通过、450 忽略、0 失败。忽略项不算本次通过。
- `workspace_text` 的 UTF-8、响应预算、协商、权限与句柄测试 2 项通过；真实 Vue 政策包回归 2 项通过。日志分别为 `workspace-text-green.log`、`policy-final-green.log`。
- Vue 源码与密封适配器各 110 项通过；TypeScript 原生源码 50 项通过并完成类型检查；JavaScript 密封运行时导航 2 项通过。均使用 Node 24.19.0，均无跳过；日志为 `vue-source-green.log`、`vue-sealed-green.log`、`typescript-source-final.log`、`javascript-native-green.log`。
- 已显式运行 Vue/TypeScript 原生混合场景，1 项通过（`native-mixed-final.log`）；不将包被安装等同于所有 TypeScript 交互已覆盖。
- 桌面实际打开 App.vue，观察启动就绪后按 F12 跳到 `zh_CN.js`；实际打开 main.js，等待启动就绪后在 `@/App` 按 F12 跳到 App.vue。对应截图为 `app-vue-autostart-green.jpg`、`app-vue-f12-green.jpg` 及表中 main.js 截图。这里只证明文档打开后的自动启动，未声称无文档的冷项目扫描会启动进程。
- 隔离桌面实例均正常关闭。Vue 生命周期报告的准入、启动、激活和清理均通过，但整体标为 `inconclusive`（业务探针未执行）；JavaScript 报告为 `passed`、正常终止。业务证据采用上述原生回归与实际键盘交互，不用生命周期报告替代。
- 真实项目最终 `git status --short` 为空；未安装或修改其依赖、锁文件和源码。

## 交付输入

所有 ZIP 通过宿主公开 `--plugin-package` 入口生成，保留旧版本包。Vue 0.2.1 要求本次宿主提供 `workspace.files >=1.5,<2`，不能只更新插件并继续使用旧宿主。

| 包 | 目录（编辑器主项目下） | SHA-256 |
| --- | --- | --- |
| Vue 0.2.1 | `dist/vue/vue-0.2.1.zip` | `9096c4a5dad8898b6b9f5b225d035e9bc81d60f1125cd81a3e3af11b2e4c5f70` |
| JavaScript 0.4.1 | `dist/javascript/javascript-0.4.1.zip` | `0cb4a3ba72ded6281215884c7f305588c4bf755074697d6b24114f98a88eca2f` |
| TypeScript 0.1.20 | `dist/typescript/typescript-0.1.20.zip` | `e2e550901552a144446c28258264e3a7f0ffb83be8b4253b91ac4ca50c0ef98e` |

共享运行时 0.1.18 的 SHA-256 为 `5923625bcae3bae4a3d3b2e88773919c2ab1b326257d01d3721a6ec5498c64e9`；JavaScript provenance 锁定 TypeScript 提交 `ffb7f9b9a095be91264269885c9de28804bf9e86`，17 个 native 文件逐个核对一致。桌面宿主二进制 SHA-256 为 `676d7eb0ae81d2c1cd971a59c477668193408c9582f88547f678fda9bd8388e5`，内嵌 SDK 为 `2a60f23c413ff91ca1356d37876e83a6572e0c139cab88c7402c457e47b0ce27`。

## 复验入口

在独立 worktree 中使用本身的 `target/`。真实包路径通过环境变量指定，不依赖已安装用户配置：

```powershell
$env:NANOBUG_VUE_PACKAGE = 'C:/Projects/RustProjects/Editor/dist/vue/vue-0.2.1.zip'
$env:NANOBUG_JAVASCRIPT_PACKAGE = 'C:/Projects/RustProjects/Editor/dist/javascript/javascript-0.4.1.zip'
$env:NANOBUG_VUE_NAVIGATION_WORKSPACE = '<用户指定的既有项目>'
# 可选：使用已准备的不可变依赖输入；不设置时按正常安装流程下载。
$env:NANOBUG_TEST_DEPENDENCY_SEED = '<已验证隔离 profile>/runtime/dependencies/versions'
cargo test -p editor-app existing_vue_project -- --ignored --test-threads=1 --nocapture
cargo test -p editor-app app::projects::tests -- --test-threads=1
cargo test -p plugin-runtime --test fault_recovery -- --ignored
cargo test -p plugin-runtime --lib workspace_text -- --include-ignored
# 政策场景另需按 Vue tests/fixtures/compatibility 的公开构建入口准备提供者与消费者包，
# 设置 NANOBUG_VUE_POLICY_PROVIDER_PACKAGE、NANOBUG_VUE_POLICY_CONSUMER_PACKAGE。
cargo test -p plugin-runtime --test vue_compatibility -- --ignored
cargo fmt --check
cargo test --workspace --exclude editor-app
cargo check --workspace
```

未把 ignored 用例、历史矩阵或候选标签当作本次通过或发布证明；Vue 2.6.10 的支持范围保持本轮实际验收的启动和导入导航。
