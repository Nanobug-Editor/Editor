# editor-ui 统一入口与锁文件清理

状态：目录与依赖迁移完成，相关验证通过；全量宿主测试保留 14 项已在抽取前程序复现的失败。用户已确认保留 Kit 正式入口、不 fork、不升级依赖，并采用 `crates/editor-ui` 与 `mod.rs`。

## 依赖与模块职责

- Kit 继续关闭默认功能、仅启用 `assets`。测试功能直接启用锁定的 `gpui-base 0.7.0`、`gpui-pre 0.3.7` 和 `gpui-pre-platform 0.3.7`，移除 Kit `test-support` 的弱引用。
- Cargo 在现有锁文件基础上重新解析，删除 `gpui-component`、`gpui-component-macros` 和仅由它们保留的六个间接包；没有升级第三方包。新增包仅为本地 `editor-ui 0.1.0`。
- 原 `editor-app/src/ui/controls` 的本地控件迁入 `editor-ui/src/<组件名>/mod.rs`。绘制、状态、辅助文件与测试保留在相应组件目录，拥有子文件的内部模块也使用 `mod.rs`。统一导出在 [lib.rs](../../../crates/editor-ui/src/lib.rs)。
- 主题、字号、语法颜色、共用静态图标和翻译资源一并集中。宿主与 UI 编译同一份物理翻译目录，不维护翻译文件副本。
- 消息面板的通用呈现和消息等级迁入 `message_panel`；宿主保留消息发布与窗口路由。关闭请求沿用 `projects::CloseWindow` 名称，由宿主继续处理未保存确认和资源释放。
- 宿主的插件视图适配仍负责实例、文档 revision、语言提供者和资源授权；控件库不反向依赖 `editor-app`。宿主资源组合与已启用主题选择也保留在应用中。
- 对齐编辑器与行边操作栏通过构造函数接入，内部字段没有为跨 crate 访问改成公开字段。原生文本、IME、选择、Undo/Redo 与停靠状态继续由原 Base 实体管理。

## 测试接入

Kit 测试扩展已替换为 GPUI 的公开帧与输入分发接口。`editor-ui::testing` 仅在测试/`test-support` 下公开，供无界面回归和有界 Windows 原生子进程共用。文本在同一窗口更新内逐字符分发，并完成帧而不推进执行器，保留草稿 Change 合并与回执排队行为；Enter/Tab 不额外注入模拟 IME 文本。

固定 WASM 包加载及公开按需激活继续复用 runtime 测试辅助文件，未新增生产测试接口。新增守卫同时检查实际依赖树、锁文件包名和控件入口结构，见 [宿主架构测试](../../../crates/editor-app/src/ui/tests.rs)。AI 约定同步到 [AGENTS.md](../../../AGENTS.md#文档状态与原生-ui)。

## 验证

所有命令在本仓库执行，metadata 核对 workspace 为 `C:\Projects\RustProjects\Editor`、target 为其 `target/`。没有混用另一份源码工作区；共享工作区的原有修改随其文件保留，不归为本轮成果。

| 检查 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --check` | exit0 | `target/editor-ui-format-2026-10-21.log` |
| `cargo check --offline --locked --workspace` | exit0 | `target/editor-ui-extraction-check-2026-10-21.log` |
| `cargo test --offline --locked --workspace --exclude editor-app --jobs 2` | 83 组，410 passed、0 failed、334 ignored；其中 editor-ui 为 78 passed、0 failed、10 ignored | `target/editor-ui-workspace-tests-2026-10-21.log` |
| `cargo test --offline --locked -p editor-app ui:: -- --test-threads=1` | 76 passed、0 failed、28 ignored；包含依赖树、锁文件与统一入口守卫 | `target/editor-ui-host-ui-tests-2026-10-21.log` |
| 表单草稿、Change 回执与场景切换 | 7 passed、0 failed；Unicode 输入、独立编辑状态和旧场景撤销保持原有行为 | `target/editor-ui-form-delivery-tests-2026-10-21.log` |
| 全量宿主测试程序 `--test-threads=1` | 634 passed、14 failed、300 ignored；失败集合与抽取前冻结程序逐项复现一致 | `target/editor-ui-app-tests-2026-10-21.log`、`target/editor-ui-pre-extraction-failures-2026-10-21.log` |
| Windows 原生初始化、真实输入与退出 | 显式运行 ignored 的 `native_initialized_windows_exit_without_tls_failure`，1 passed；内部完成 KitRoot、NativeEditor 两个真实 Windows/D3D11 子进程 | `target/editor-ui-native-input-exit-2026-10-21.log` |
| 静态边界与资源 | Kit 仅启用 assets；全部功能、目标和 normal/build/dev 边无组件库；锁文件无组件及宏库；47 个入口目录，src 根部仅有 lib.rs；迁移后的字面量资源路径有效 | `target/editor-ui-dependency-tree-2026-10-21.log`、`Cargo.lock`、宿主架构测试与源码检索 |

全量与原生宿主测试使用 `cargo test --offline --locked -p editor-app --no-run` 生成的 EXE，复制 EXE 与匹配 PDB 到 `target/component-lock-validation-2026-10-21/editor-ui-host-tests.exe` 后执行，避免 Windows 上并行构建重链接正在运行的文件。该程序 SHA-256 为 `55370851fe41f2878108bc1a637661c75f239f5bfb5a32a6a4cb83b081357b9d`；抽取前、已切换测试依赖的冻结程序为同目录 `editor-app-tests.exe`，SHA-256 为 `6cb163fbba0bcb4b93e9a0cb4fee02d3beb3e9f5c694e2e8c64b1339274e40cc`。

14 项剩余失败为语言服务夹具/状态/跳转 6 项、宿主消息 5 项、保留原生视图的旧断言 1 项、旧包范围控件断言 1 项、worker 错误文案 1 项。每项均在抽取前冻结程序中失败，集合完全一致；没有将它们报告为通过，也没有为了迁移修改这些产品行为或断言。

用户随后授权继续排查这些失败；原因、修复和后续全量检查独立记录于[剩余 14 项测试验证](remaining-tests-2026-10-21.md)，不覆盖上表的迁移阶段历史结果。

验证期间修正了共享辅助文件同名覆盖、schema/包测试引用的旧主题与图标路径，以及依赖 crate 中 `cfg(test)` 不生效造成的可访问值快照缺失。原辅助文件已恢复，新增按需激活辅助函数使用独立的 `support/ui_demand.rs`。UI 快照由 `test-support` 显式开启，正式构建仍只在可访问客户端需要时镜像文本。

首次完整链接还遇到 `LNK1104: msvcrt.lib`。核对本机文件存在后，仅为该验证进程设置 `LIB` 为已安装 MSVC `14.50.35717/lib/x64` 及 Windows SDK `10.0.26100.0/{um,ucrt}/x64`，随后重复同一 Cargo 测试入口并通过；未安装工具链或修改全局环境。保留既有 unused、private_interfaces 和 Wasmtime 链接警告。

ignored 项未执行的不计通过；本轮没有重建无关 WASM 包。未交叉验收 macOS/Linux，没有手工复验完整 Windows 输入法候选窗口；原生结果限于上述真实窗口输入与退出切片，不将历史并发可访问查询下的 AccessKit 异常宣称为已修复。未提交、推送或发布。

2026-10-19 的弱可选锁文件记录及当时的失败保留为历史证据；本轮的锁文件要求更严格，不能继续用“未编译即可保留组件包”作为通过条件。

返回 [UI 文档目录](../README.md)。
