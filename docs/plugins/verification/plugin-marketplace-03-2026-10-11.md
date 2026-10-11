# 插件市场 03：公开 SDK 与独立分发集成

2026-10-11，依据[市场工单 03](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/issues/4)。本记录区分宿主基础交付与七个插件的正式市场交付；当前工单仍进行中，不因本地包构建成功关闭议题。

## 来源与范围

在隔离工作树 `C:/Users/Tmiracle/.codex/worktrees/marketplace-versions/Editor`、分支 `codex/marketplace-versions` 实施，宿主基线为 `2cdc683118b7798add6436eaca3fe9a0e07e3056`。主工作区 `C:/Projects/RustProjects/Editor` 的已有改动只读核对，未修改其文件、索引或提交。

Rust 0.6.0 与 Rust Debugger 的业务合并已在其他任务完成，来源、功能和历史结果见[合并方案](../specs/rust-plugin-consolidation.md)及[历史验收](rust-plugin-consolidation-2026-10-09.md)。本单只迁入合并包独立构建所需的公开契约、运行时和受影响测试闭包，不将业务合并计为本单成果，也不独立发布 Rust Debugger。

公开协议继续使用 protocol 7；通用延迟依赖能力为 `dependencies 1.2`。替换声明、服务配置和平台要求面向全部插件，未增加插件 ID、语言或服务器专属分支。SDK 导出只包含源码、Cargo、WIT 和 JSON 资源；Markdown 留在公开仓库供维护者阅读。

## 本单修复与可观察行为

- 市场操作捕获精确的被替换安装记录，在确认窗口显示替换身份及实际运行/调试会话影响。取消不会下载或改动旧记录；确认后仍在准备与提交前复查记录、目录 generation、包摘要和工作区信任。
- 缓存命中的延迟依赖也取得租约，并写入当前包版本收据；替换后卸载旧记录、删除旧下载源并清理缓存，新包仍可离线启动。准备完成后直接分发，不重复进入准备队列。
- 替换事务在发布前分别检查设置、提供者偏好和事务总量上限；提交后采用已验证的内存投影，避免成功落盘后再因读取失败返回失败。超额候选不会替换原安装或私有数据。
- 原生运行配置测试使用现行公开插件配置和宿主 Shell 表单，复用已有 `host_templates::owns` 输入路由。共享 HTTP 服务器仅用于测试真实目录/ZIP 传输。
- 选定平台后，打包器同时裁剪已省略服务的语言服务引用及备选引用，保留通用资源；未知引用仍然拒绝。可选服务接受当前已协商的更高依赖能力要求，不错误要求 `>=1.2` 同时包含 1.1.0。XML 保留其他平台显式配置本机 LemMinX 的原有能力。

## 当前实际验证

工具链为本机现有 Rust/Cargo 1.98.1、Windows x86_64 MSVC、`wasm32-wasip2`；未安装新工具。MSVC 链接首次缺少 `msvcrt.lib`，重新导入已安装的 Visual Studio 18 Community 开发环境后重跑。

- `cargo fmt --check` 通过。
- 最终 `cargo test --workspace --exclude editor-app`：237 passed / 188 ignored；ignored 不计为通过。
- 最终 `cargo check --workspace` 通过，5.08 秒。
- `cargo test -p editor-app marketplace -- --test-threads=1`：7 passed / 4 ignored；`sdk_export` 4 passed；`local_import` 2 passed。
- `cached_adapter_survives_replaced_owner_uninstall_and_offline_restart --ignored`：1 passed，44.51 秒；先观察旧实现删除源后失败，再完成修复。
- `replacement_quota_failure_preserves_the_original_installation --ignored`：1 passed，51.67 秒；先观察旧实现已替换原记录，再验证提交前拒绝及重启保留。
- `cargo_artifacts_debug_in_parallel_and_retire_through_native_management --ignored`：1 passed，84.11 秒。两个真实 Cargo/CodeLLDB 调试目标暂停后，从生产市场按钮打开确认窗口，检查实际会话影响并取消；两个目标和无关 Shell 任务继续运行，再完成原有停用/移除清理验收。
- `market_wasm_replacement_requires_visible_consent_without_new_grants --ignored`：隔离运行 1 passed / 0 ignored，128.30 秒。最终复跑曾在后台恢复阶段超时；停止并行重编译后单独运行通过，未把超时那次计为通过或隐去。
- Rust 合并功能用当前独立包复验 4 passed / 0 ignored，218.99 秒；真实调试 7 passed / 0 ignored，144.04 秒。真实 Cargo 目标用例首次因历史固定文件名 `rust-0.6.0.zip` 缺失失败，改用真实 `rust-0.6.1.zip` 后 5 passed / 0 ignored，101.29 秒；不把新版 ZIP 伪装成旧版。
- 可选依赖与平台裁剪回归均先观察旧实现拒绝合法候选，再完成修复；可选依赖组 3 passed，独立构建组 9 passed。最后共同门禁包含新增未知引用拒绝断言并通过。
- 最终公开 `--plugin-package ... --platform linux-x86_64` 分别生成并检查 HTML、JavaScript、XML 目标包。HTML/JavaScript 省略 Windows 服务但保留资源；XML 保留手工配置服务声明。这是跨目标打包检查，不是 Linux 执行验收。
- 最后 `cargo test -p editor-app marketplace -- --test-threads=1` 7 passed / 4 ignored，5.36 秒；公开 CLI 与惰性 Package 检查器构建通过，35.73 秒。

实际 SDK guest 夹具 ZIP SHA-256 为 `6228ee43d31af039f9dff2483f331f046ff37b70b11de6f3ff49c1258505421a`。CodeLLDB 1.12.3 VSIX SHA-256 为 `a916e509308dac817732f63ca604a8b93ed29cd16f38a2fa9f0b64ed58e8f51a`。旧 Rust 0.5.1 / Rust Debugger 0.2.0 的冻结包只作迁移测试输入，不上架。

## Standards

从固定点 `2cdc683` 审查宿主完整工作区差异和新增文件。硬性违规 0，判断性坏味道 0。复查覆盖缓存租约/收据、事务配额与提交边界、SDK 文档、通用市场确认、本地控件和测试输入路由；复制的外部 Rust 成果有明确来源。

## Spec

宿主基础及七仓迁移范围复审无未修复阻断；缓存所有权、会话影响、可选能力版本和平台裁剪问题均已修复。通用权限、实例身份、取消及旧数据保护未见退化。本结论不代表七个独立 Release 或正式目录已验收。

## 交付边界

七个独立项目已使用公开 `--plugin-package` 入口构建；Markdown 73、Rust 6、Image 8 个 guest 单元测试通过，XML guest 编译通过但无单元测试，XML 行为仍须由宿主真实包场景验证。源码/资源差异审计仅发现 Markdown 的外部文档测试输入改为仓库内冻结夹具；业务内容保持现有成果。冻结文档 SHA-256 为 `4d8b0b633fd38c7e2d2f5e23d87b19e4fe8e8b61576a833d9fac3321c2556b75`。

仍需固定并公开宿主提交、七个插件源码提交和 Release、登记完整 ZIP 摘要、完成注册表 PR 人工审核、七个包的实际市场首次安装及代表行为、真实 Pages/Release 核验。工单 04 删除源码前必须先验证替代测试输入。手工操作系统 IME 和非 Windows 平台未验证。
