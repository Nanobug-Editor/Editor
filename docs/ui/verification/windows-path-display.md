# Windows 完整路径显示修正

日期：2026-10-14。状态：本次范围实现、回归及原生验收完成；完整应用测试保留 8 项修复前已有失败。未提交、推送或发布。

## 原因与边界

Windows 的 `std::fs::canonicalize` 返回扩展命名空间路径，例如 `\\?\C:\Projects\Editor`。这是有效的内部文件路径。此前多个展示入口直接调用 `Path::display` 或 `to_string_lossy`，将内部前缀带入项目列表、窗口标题、提示和错误信息。已有的“复制完整路径”单独隐藏过前缀，其他入口没有共享这项规则。

现在由 [editor-core 的路径格式化模块](../../../crates/editor-core/src/paths.rs) 统一呈现，原生适配层复用同一实现。仅转换人可读的文本，不进行文件系统访问、不重新规范化、不改写路径对象。

| 输入 | 显示 |
| --- | --- |
| `\\?\C:\Projects\Editor` | `C:\Projects\Editor` |
| `\\?\UNC\server\share\项目` | `\\server\share\项目` |
| 普通磁盘路径、相对路径 | 保持原样 |
| 设备及卷 GUID 命名空间 | 保留有效命名空间拼写，不伪造普通磁盘路径 |

中文、空格、盘符大小写、目录后缀和标准库的异常 Unicode 替换语义保持一致。非 Windows 平台沿用标准路径显示。规范化身份、窗口去重、最近记录、会话键、权限边界、资源 URI、原生 I/O 目标及已有子进程适配接缝保持各自权威；不对任意文本进行全局字符串替换。

## 已修正入口

- 项目管理列表、项目下拉条目、项目标题及完整路径提示，盘符根目录名称的回退显示。
- 文件标签完整路径提示、外部文件的问题列表标签、语言定义预览及引用定位标签。
- 删除、冲突、恢复及清理等文件操作提示，文档保存状态和未保存目标诊断。
- 原生选择器回填的路径文本、克隆目标的选择器显示、运行配置的路径错误。
- 文档、工作区、本地历史的类型化错误，以及插件项目、依赖事务、安装提示、构建和工具发现的宿主诊断。
- 复制完整路径/相对路径改为使用共享格式化，保留原有根目录相对值 `.`。

检索剩余的 `display` 与 `to_string_lossy` 调用后，按用途检查持久化键、路径标识、进程参数、权限、协议、资源路径和普通文件名；这些调用不能仅因含有前缀而改成显示文本。构建程序自身或插件提供的任意输出未被字符串清洗。本次没有新增协议能力或修改权限、文件访问与进程生命周期。

## 回归与诊断证据

通过真实 GPUI 首页/项目窗口建立回归：加载包含规范化路径的外部最近记录，从实际 Base 控件快照检查路径标签，按现有选择流程打开项目，再检查标题入口及下拉标签，同时断言项目仍使用原规范化根。修复前连续两次失败，单次测试执行约 0.03 秒。

只调整近期项目行的显示后，首页断言通过、原根身份断言通过，测试继续在标题与下拉标签处失败；证实问题来自展示边界。随后统一替换相关入口，完整场景通过。共享格式化的五个测试覆盖磁盘、UNC、普通路径、设备命名空间及异常 Unicode；原生读取不存在的文件和历史目录创建失败的回归先失败后通过，错误对象仍保留原目标与 I/O 原因。

实际定向命令：

```powershell
cargo test -p editor-core path_display_ --locked -j 1
cargo test -p platform-windows native_path_errors_display_without_verbatim_prefixes --locked -j 1
cargo test -p editor-app project_paths_display_without_windows_verbatim_prefixes --locked -j 1 --target-dir C:\Projects\RustProjects\Editor\target\project-switcher --config 'profile.test.package.editor-app.debug=1' --config 'profile.test.package.plugin-protocol.debug=1' -- --test-threads=1
```

| 验证 | 结果 | 证据文件 |
| --- | --- | --- |
| 共享格式化 | 5 passed、0 failed | `path-display-core-{red,green}.log` |
| 原生路径错误 | 1 passed、0 failed | `path-display-native-errors-{red,green}.log` |
| 真实项目窗口路径回归 | 1 passed、0 failed，规范化身份保持 | `path-display-ui-{red,home-probe,green}.log` |
| `cargo fmt --check` | 通过 | `path-display-fmt-final.log` |
| `cargo test --workspace --exclude editor-app --locked -j 1` | 274 passed、0 failed、256 ignored | `path-display-workspace-tests.log` |
| `cargo check --workspace --locked -j 1` | 通过，保留既有警告 | `path-display-workspace-check.log` |
| 当前冻结程序全量应用测试 `--test-threads=1` | 553 passed、8 failed、236 ignored；163.36 秒 | `path-display-app-tests-final.log` |
| 全量结果内的项目与文件菜单场景 | 33 个项目场景、10 个文件菜单场景全部通过 | 同上 |
| `git diff --check`、本地文档链接 | 通过 | 交付时实际检查 |

完整应用的七个失败与[既有项目窗口基线](project-windows-main-integration.md)相同，另一个插件界面用例 `extensions::tests::incompatible_plugin_details_keep_preferences_and_update_uninstall_actions` 已在本轮修复前程序中复现相同失败，见 `path-display-existing-plugin-failure.log`。这些失败未改动，也未计为通过。

首轮完整应用还捕获正在并发实现的欢迎页克隆用例失败；最新源码的定向运行与最终全量运行均通过，首轮结果不替代最终结果。欢迎页的重新设计、克隆执行行为和其他工作区已有改动不归入本次路径显示成果。本次范围不要求构建真实 WASM 夹具，未运行的 ignored 不计为通过。

最终冻结应用测试程序 SHA-256：`a387a123bee1859f6a70a20f28e83405978ec13506ddca56ae992e3a6ff17331`。Cargo 返回的测试/原生产物来源及 45 个相关源码文件摘要均有记录；没有升级依赖或自行提交。

## Windows 原生验收

原生程序 SHA-256：`4933acf89c7e937ce8996d2f9e26e555e9b33bfcd8b010da5c3c7624e60509f2`。在独立 profile、已保存受限模式的中文/空格项目路径中验证，没有操作日常 profile、信任提示或修改验收文档文本。

实际系统窗口标题、项目入口无障碍文本、下拉条目和悬停提示均显示普通磁盘路径；项目管理列表及无障碍文本也不再带前缀。深浅主题通过。点击已打开项目记录返回原窗口 ID，窗口数保持不变；读取最近记录文件确认内部仍保存 `\\?\` 规范化路径。最后通过应用退出流程关闭两个验收窗口并读回退出状态。

截图：[项目菜单与完整路径提示](assets/path-display-project-menu.jpg)、[项目管理列表](assets/path-display-project-home.jpg)、[深色菜单](assets/path-display-project-menu-dark.jpg)。图片来自实际运行程序，不以原型代替验收。

本轮日志、修复前/最终冻结程序、检索清单及来源摘要保留在 `C:/Users/Tmiracle/.codex/visualizations/2026/10/14/01a13876-e6ea-7f53-a781-938a62fad9eb/`，文件以 `path-display-` 开头。
