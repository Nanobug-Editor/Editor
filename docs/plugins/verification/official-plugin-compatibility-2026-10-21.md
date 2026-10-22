# 官方插件兼容性核验（2026-10-21）

状态：本批静态准入核验已完成；SDK 助手改造尚未实施。改造建议见[方案](../specs/official-plugin-sdk-adoption-2026-10-21.md)，来源范围见[源码调研](../specs/official-plugin-sdk-adoption-sources-2026-10-21.md)。

## 核验对象与固定输入

市场范围取自[当前官方目录](https://nanobug-editor.github.io/Plugin-Marketplace/catalog.json)及 [registry 固定提交](https://github.com/Nanobug-Editor/Plugin-Marketplace/tree/cd83b2f94814f6463f4104e5eaa6890f1132098a/registry)。目录为七个插件、八个已收录版本；Markdown 有两个版本。组织另有 TypeScript、Vue 仓库，核对时无 GitHub Release，也未列入当前目录；Git 按宿主维护文档中的本地独立交付对象补充，不计作市场收录。

- 核验时间：`2026-10-21T14:21:40.2541824Z`，Windows x86_64。
- 命令：`editor-app.exe --plugin-check <ZIP> --format json`。使用已有宿主，不编译新宿主，不重建或重新打包被测插件。
- 宿主：`target/plugin-developer-tooling/debug/editor-app.exe`；SHA-256 `c82565746b49011761884604da17e1bcbd30a45cb972b42a2a6ba20fda940856`。
- SDK：`8e2f359fedf819d8e8b6b3da1012722629e12d9731c02c38e5b01ccab8bd02e4`。当前仓库 HEAD `fc9dc531ef991315d73e21c8a092f12a0f1ff78b` 仅作审计参照；工作区有其他任务改动，不能用 HEAD 代替可执行文件身份。
- 市场目录 SHA-256：`0cff73e72a8b942cfdb24ae0d915c5baab9f07e7ef6b70087becbcd969664b24`；registry commit `cd83b2f94814f6463f4104e5eaa6890f1132098a`。
- [完整输入与结果账本](official-plugin-compatibility-2026-10-21.json)保存下载地址、发布源 commit、包字节数、完整 SHA-256、退出码及结构化诊断。原始归档和 stdout/stderr 保留在 `target/official-plugin-sdk-audit-2026-10-21/`。
- 全部检查结果 `executed=false`。本批没有安装插件、运行 WASM、启动 LSP/原生模块、下载服务依赖或操作用户的宿主会话。

## 已发布包：8/8 通过

直接下载目录指向的 GitHub Release ZIP，先核对完整 SHA-256，再执行生产 Package 准入检查；八份摘要均与目录一致，报告均无 warning/error。

| 插件 ID | 版本 | 实际退出结果 | ZIP SHA-256 |
| --- | --- | --- | --- |
| html | 0.3.1 | 0 / 通过 | `6f1ad3e33ade378f9fdbdedef5d1d8e6f91b4059f620f7a490f0dd97af4a4f5e` |
| javascript | 0.3.1 | 0 / 通过 | `a09afb41de50211e5e0a866d2698dde2a6d524b54dc74149eda573f42eabe144` |
| markdown | 0.17.2 | 0 / 通过 | `cac840637b5348c553da810e0d101f318088714caec18e30710bbc97c8327e13` |
| markdown | 0.17.1 | 0 / 通过 | `2438a081165de70acf0c30c560905167d46b44a5c30d7abf856fa420a14a17a3` |
| rust | 0.6.1 | 0 / 通过 | `9da637f2852318c51fb7b008c1abaefe4848a1d0896d6205210558a561de6867` |
| svg | 0.6.1 | 0 / 通过 | `911932c643bc9b7a538b3f4b5baa34e04a939256b61ee49bf6d2f9e84656a654` |
| toml | 0.2.3 | 0 / 通过 | `2b3dabe0b2f67b2ed2440fe5eb8e7ac2b3e408105accc2f7bbdc9a2ccf6ebeed` |
| xml | 0.2.2 | 0 / 通过 | `c6bde8af6e724d2c9570502293f4e9d024137761759205baffb6f10a8bd3dd8a` |

HTML、JavaScript、TOML 为声明式包，无访客组件；没有要求它们增加空 WASM。其余包的静态通过也不等于访客已成功实例化或业务功能已验收。

## 本地候选：4/6 通过，2/6 拒绝

只复制已有 ZIP 到本批输出目录后检查，未替换原件。候选不是已发布版本，不因版本号相同而混用摘要。

| 候选 | 实际退出结果 | ZIP SHA-256 |
| --- | --- | --- |
| git-local-0.1.23 | 0 / 通过 | `7d0ea99e484afafe343902a1cc30baaebbb7a618ae1c4ecee4144579727fe5bd` |
| markdown-local-0.18.2 | 0 / 通过 | `61a648070f6d659ffbadd9cf8a7f8da04110ba47d007a55f6ddcd28c93a31ca2` |
| typescript-supported-0.1.15 | 1 / 拒绝 | `beab4419c7004ce96d4bf2ce3f4a826f0c8f2a777a9a4b0c3896496f314c038b` |
| typescript-desktop-0.1.15 | 1 / 拒绝 | `422074c65d63d89b2f8b060feb15ceaaffaf4b7a3c77b19f6c6c665ebdb39f43` |
| vue-supported-0.1.12 | 0 / 通过 | `5f6a14c2e6b4aa139e33bcdbb3711d56ac03620b784f2dd86cda39810b7e1b73` |
| vue-desktop-0.1.12 | 0 / 通过 | `058cb1eec9dbef169aba8095c2e94c0f29935b845735d1c7488108e8a02e4a5e` |

Git 来源为[0.1.23 手势验收记录](git-branch-gestures-2026-10-21.md)的最终归档，摘要与该记录相同。当前 `C:/Projects/RustProjects/Git-for-Nanobug` 是独立源码目录，根目录不是 Git checkout，不能给它编造 HEAD。Markdown `0.18.2` 为本地未发布候选，不替代市场 `0.17.2` 的身份。Vue、TypeScript 分别检查了 supported 与 desktop 两种归档；同版本实际字节不同，全部保留。

### TypeScript 的首个拒绝点

两个 `0.1.15` 候选均返回：

- 退出码 1；`passed=false`，`stopped_at=package_admission`。
- `code=manifest`，`file=manifest.json`，位置第 480 行第 21 列。
- 包内 `language_servers[0].delegate` 含 `interactive: true`。

本次宿主 SDK 的 [Delegate 定义](../../../crates/plugin-protocol/src/language.rs)仅有 `contract`、`method`，并采用 `deny_unknown_fields`；本次宿主导出的 SDK 同样如此。由包内容、报告位置和该定义可确认这里存在目标宿主与候选契约不匹配。诊断止于清单解析，未继续验证后续能力、组件和业务。

处理要求是固定候选所需的宿主/SDK 配套，再重复完整检查；不能通过放宽未知字段校验或删掉 `interactive` 来宣称兼容。若正式目标需要新增通用交互能力，应依照已有语言规格与能力版本约定另行确认，不混入 SDK 助手改造。

## 结果的适用范围与复验

静态检查实际覆盖 ZIP、清单、声明能力、权限声明、资源及现行贡献校验。它不执行运行权限授权，不证明 LSP、native module、操作系统工具、用户业务、热更新或卸载清理已经通过。本轮结论是“八个收录包均被上述固定宿主静态接纳”；没有宣称所有官方候选都兼容或所有业务都通过。

示例复验（PowerShell，仓库根目录）：

```powershell
$checker = ".\target\plugin-developer-tooling\debug\editor-app.exe"
& $checker --plugin-check ".\target\official-plugin-sdk-audit-2026-10-21\packages\markdown-0.17.2.zip" --format json
$LASTEXITCODE

# 当前固定目标应拒绝此候选，退出 1。
& $checker --plugin-check ".\target\official-plugin-sdk-audit-2026-10-21\packages\typescript-supported-0.1.15.zip" --format json
$LASTEXITCODE
```

本次为调查与文档交付，不修改插件/宿主源码、不提交、推送、安装或发布。文档本地链接、尾随空白、14 份归档与报告/宿主/SDK 身份、JSON 结果一致性及 `git diff --check` 均已核对通过。无 Rust 源码改动，不要求无关的完整 Rust 构建。
