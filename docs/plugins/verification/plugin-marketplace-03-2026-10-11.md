# 插件市场 03：公开 SDK 与独立分发集成

2026-10-11，依据[市场工单 03](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/issues/4)。本记录区分宿主基础交付与七个插件的正式市场交付；本单已完成下述正式收录和线上消费验收，工单 04 另行执行。

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

## 已公开的独立来源与固定附件

宿主公开 SDK 固定为 [`b77e6317bf7e3d3d3d0fe9490cabd362d266e0a5`](https://github.com/T-miracle/Nanobug/commit/b77e6317bf7e3d3d3d0fe9490cabd362d266e0a5)。七个独立仓库均由 T-miracle 维护，MIT 许可证已经按源码 commit 读回；固定 tag 指向下表源码，稳定 Release 非 draft、非 prerelease。七个源码提交的 CI 均通过独立公开 SDK 构建。Image 保留 `svg` 安装身份，Rust 只发布合并包。

| 独立仓库 | 安装 ID / Release | 源码 commit | 完整 ZIP SHA-256 | 源码 CI |
| --- | --- | --- | --- | --- |
| [HTML](https://github.com/T-miracle/HTML-for-Nanobug) | `html` / [0.3.1](https://github.com/T-miracle/HTML-for-Nanobug/releases/tag/v0.3.1) | `68b8c7baabd0790cba884aa19e4f2523b2b2e7ce` | `6f1ad3e33ade378f9fdbdedef5d1d8e6f91b4059f620f7a490f0dd97af4a4f5e` | [通过](https://github.com/T-miracle/HTML-for-Nanobug/actions/runs/38121945903) |
| [JavaScript](https://github.com/T-miracle/JavaScript-for-Nanobug) | `javascript` / [0.3.1](https://github.com/T-miracle/JavaScript-for-Nanobug/releases/tag/v0.3.1) | `18dc8818787857b75cb1ae1b4ffd88edfa35a631` | `a09afb41de50211e5e0a866d2698dde2a6d524b54dc74149eda573f42eabe144` | [通过](https://github.com/T-miracle/JavaScript-for-Nanobug/actions/runs/38121955827) |
| [Markdown](https://github.com/T-miracle/Markdown-for-Nanobug) | `markdown` / [0.17.1](https://github.com/T-miracle/Markdown-for-Nanobug/releases/tag/v0.17.1) | `31516790679c2ab591105d11e6fd15e974b72505` | `2438a081165de70acf0c30c560905167d46b44a5c30d7abf856fa420a14a17a3` | [通过](https://github.com/T-miracle/Markdown-for-Nanobug/actions/runs/38121964796) |
| [Rust](https://github.com/T-miracle/Rust-for-Nanobug) | `rust` / [0.6.1](https://github.com/T-miracle/Rust-for-Nanobug/releases/tag/v0.6.1) | `942a905e4d4dfe02a9b9b162c35e3c2788691d5f` | `9da637f2852318c51fb7b008c1abaefe4848a1d0896d6205210558a561de6867` | [通过](https://github.com/T-miracle/Rust-for-Nanobug/actions/runs/38121974536) |
| [Image](https://github.com/T-miracle/Image-for-Nanobug) | `svg` / [0.6.1](https://github.com/T-miracle/Image-for-Nanobug/releases/tag/v0.6.1) | `cac09895b6bc9cd227a6e318edb8c868da41084e` | `911932c643bc9b7a538b3f4b5baa34e04a939256b61ee49bf6d2f9e84656a654` | [通过](https://github.com/T-miracle/Image-for-Nanobug/actions/runs/38121983340) |
| [TOML](https://github.com/T-miracle/TOML-for-Nanobug) | `toml` / [0.2.3](https://github.com/T-miracle/TOML-for-Nanobug/releases/tag/v0.2.3) | `a1a4677118403ac01e6a0fd2f667cfa788aa9bd2` | `2b3dabe0b2f67b2ed2440fe5eb8e7ac2b3e408105accc2f7bbdc9a2ccf6ebeed` | [通过](https://github.com/T-miracle/TOML-for-Nanobug/actions/runs/38121992075) |
| [XML](https://github.com/T-miracle/XML-for-Nanobug) | `xml` / [0.2.2](https://github.com/T-miracle/XML-for-Nanobug/releases/tag/v0.2.2) | `69749e0d37ca28571e39101678f77a051a939ca8` | `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a` | [通过](https://github.com/T-miracle/XML-for-Nanobug/actions/runs/38122000455) |

最终附件由公开宿主 CLI 按 Windows 平台生成，经惰性 Package 检查后上传；GitHub 读回的附件摘要、文件大小、tag 和发布正文与候选记录一致。随后生产目录构建器使用真实公开 GitHub HTTP、许可证、源码 tag 和 ZIP 重新导出七包候选目录；七个 HTTPS 下载附件全部经过完整 SHA-256 与 Package 校验。候选先在任务隔离输出中验证，再按维护者本批授权合并发布；正式 Pages 及客户端消费结果见下文。

ZIP 摘要锁定审核过的完整字节，附件被替换会使目录构建或客户端安装失败；相同源码重建仍可能因归档时间戳或构建输入不同产生不同摘要。本次保留上传的固定附件，不通过覆盖 Release 或重建别名改变既有版本；摘要不能单独证明 ZIP 由所列源码构建。

`xml_plugin_package_enters_existing_language_registry --ignored` 使用最终 XML 附件通过：1 passed / 0 ignored，668.67 秒。覆盖真实语言注册、LemMinX、补全、导航、schema 与设置变更。

`cargo test -p editor-app marketplace_official_releases_install_with_native_consent -- --ignored --test-threads=1 --nocapture`：1 passed / 0 ignored，147.81 秒。该单一参数化场景逐包使用隔离工作区和数据目录，经生产市场安装按钮与权限确认、真实 HTTP 下载、完整 ZIP 摘要、公开 Worker/Manager 事务和实际 WASM 激活，并检查六种语言的识别与真实 grammar captures。浏览及确认前下载数为 0，安装后各为 1；安装清单、权限、ID 和摘要与固定包一致。中英文及深浅主题交替执行。

七包从确认到安装、贡献发布和高亮的观测时间依次为 HTML 4.90、JavaScript 4.75、Markdown 22.03、Rust 19.49、Image 20.14、TOML 0.39、XML 69.21 秒。首次执行在 XML 已提交安装记录后超过共享 60 秒预算而失败；实际 XML 对照场景持续推进设置与服务验收，排除了永久安装阻塞。新增七包场景单独使用 180 秒上限以覆盖 debug 构建中安装、补全和结构实例的独立 WASM 编译，普通回归仍用 60 秒；未修改生产行为或把失败那次报告为通过。

其余代表行为使用同次已编译的 `target/debug/deps/editor_app-6b5fbeb5e3fc48db.exe <用例> --ignored --test-threads=1 --nocapture` 逐项运行，固定输入为最终 Release ZIP，避免为每包重新链接宿主。以下均为 1 passed / 0 ignored：

| 原生代表行为 | 实际用例 | 时间 |
| --- | --- | --- |
| HTML/XML 配对标签、重命名、中文输入及 Undo | `installed_xml_html_tags_share_native_rename_linked_input_and_undo` | 78.50 秒 |
| JavaScript 格式化、替换提供者及原分析能力保留 | `installed_javascript_formatter_can_be_replaced_without_changing_analysis` | 7.56 秒 |
| Markdown 未保存编辑、原生预览及分屏资源回收 | `delivered_markdown_preview_tracks_unsaved_native_edits_and_reclaims_split` | 20.59 秒 |
| Image 的 PNG/JPEG/GIF/WebP 原生只读绘制 | `image_package_draws_readonly_formats_in_the_native_file_container` | 17.99 秒 |
| 合并 Rust 的 Cargo 两目标调试、市场替换确认取消及会话回收 | `cargo_artifacts_debug_in_parallel_and_retire_through_native_management` | 55.09 秒 |

新增测试后最后共同门禁再次通过：`cargo fmt --check`；`cargo test --workspace --exclude editor-app` 237 passed / 188 ignored；`cargo check --workspace` 0.96 秒。普通 `marketplace` 组 7 passed / 5 ignored，6.46 秒；新增七包 ignored 场景已单独显式通过，不把其余未执行项算作通过。当前测试变化为 cfg(test)，SDK 与正式包输入仍是 b77e631，无须因此重新发布包。

## 正式收录与线上验收

维护者于 2026-10-11 明确要求“这几个本地插件帮我自动通过审核”，授权本批七个既有本地官方插件的固定版本收录并合并。授权只覆盖本批；以后版本和第三方仍按市场规格经 PR 审核。已核对各包来源、许可证、权限、原生依赖、固定 tag 和完整 ZIP 摘要。

[PR #6](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/pull/6) 对 `6cec7a06e1727d4d1d28406f4d9d973bc7c0098f` 的[可信 CI](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/actions/runs/38122382439)通过，独立 Standards 与 Spec 复审分别 0 阻断。PR 已于 2026-10-11T07:48:46Z 合并，市场提交为 `8d1a8ca28d5c7a6a88dd2c93c2f9ce4598c38b99`；[main 校验、统计及 Pages 部署](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/actions/runs/38122634435)均 success。

维护者要求七个新项目使用 `X for Nanobug` 名称，因此重新创建 `X-for-Nanobug` 仓库。七个新仓均保留原源码 commit、tag、安装 ID、版本、ZIP 名和字节；main 只有双语 README 的命名与链接更新，未移动发布标签或重新打包。新仓七个独立构建 CI 已全部通过，下表提供实际 run 链接。新地址经生产构建器真实 HTTPS 重新导出，许可/tag/Release/全文摘要及惰性 Package 校验均通过。

- 新地址七包原生安装复验：`marketplace_official_releases_install_with_native_consent --ignored`，1 passed / 0 ignored，131.83 秒；XML 66.90 秒。隔离数据目录、确认前零下载、确认后恰一次、真实安装/激活及语言高亮断言均保留。
- 正式 [catalog.json](https://t-miracle.github.io/Nanobug-Plugin-Marketplace/catalog.json) 返回七个新仓条目，与生产构建器快照逐版本相符；[stats.json](https://t-miracle.github.io/Nanobug-Plugin-Marketplace/stats.json)包含七包及实际测量时间 `2026-10-11T07:53:39Z`。
- `marketplace_published_pages_installs_reviewed_release --ignored`：1 passed / 0 ignored，1.34 秒。通过公开 Worker 的正式 Pages URL 刷新，再从新仓 GitHub Release 下载 TOML 0.2.3，以公开 Manager 安装到隔离目录；身份、权限、版本、完整摘要、启用及持久化缓存正确，正式统计可读取。这项只验证真实网络/安装链路；原生确认与七包代表能力由前述共享 GPUI 用例验证，不重复完整矩阵。
- 首次线上用例在部署前空目录状态下失败，未将其记为通过；部署成功后使用相同正式网络入口完成上述验收。新增测试释放 actor 状态锁后再断言，避免测试失败破坏清理；未因此改变生产协议、SDK 或正式包。
- 本阶段三项共同门禁通过：格式检查、非 UI workspace 237 passed / 188 ignored、workspace 编译检查；新增源码仅为 cfg(test)。市场契约/统计 9 passed。七包 SDK 与 ZIP 输入未变，复用前文代表能力证据。

## Standards

从固定点 `2cdc683` 审查宿主完整工作区差异和新增文件。硬性违规 0，判断性坏味道 0。复查覆盖缓存租约/收据、事务配额与提交边界、SDK 文档、通用市场确认、本地控件和测试输入路由；复制的外部 Rust 成果有明确来源。

## Spec

宿主基础及七仓迁移范围复审无未修复阻断；缓存所有权、会话影响、可选能力版本和平台裁剪问题均已修复。通用权限、实例身份、取消及旧数据保护未见退化。追加从固定点 `b77e631` / 市场 `4d7ee9a` 审查七个候选注册记录、批次审核说明与真实七包安装测试，两轴均为 0 个新阻断。源码审查不代替原生行为、人工注册表审核或正式 Pages 验收。

## 交付边界

七个独立项目已使用公开 `--plugin-package` 入口构建；Markdown 73、Rust 6、Image 8 个 guest 单元测试通过，XML guest 编译通过但无单元测试，XML 行为仍须由宿主真实包场景验证。源码/资源差异审计仅发现 Markdown 的外部文档测试输入改为仓库内冻结夹具；业务内容保持现有成果。冻结文档 SHA-256 为 `4d8b0b633fd38c7e2d2f5e23d87b19e4fe8e8b61576a833d9fac3321c2556b75`。

七仓、固定源码提交及稳定 Release 已公开，完整 ZIP 摘要已登记在候选注册记录，七包原生首次安装及各自代表行为已通过。已按维护者本批授权完成注册表 PR 收录、正式非空 Pages、统计和客户端线上消费验收。工单 04 删除源码前必须先验证替代测试输入，删除前盘点见[工单 04 准备记录](plugin-marketplace-04-inventory-2026-10-11.md)。手工操作系统 IME 和非 Windows 平台未验证。
