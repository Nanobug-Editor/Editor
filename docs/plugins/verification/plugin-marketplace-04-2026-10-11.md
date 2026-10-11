# 插件市场 04：无随附 Windows 交付验收

状态：Windows 行为验收完成，原 37 项失败及图像组稳定性已解决。最终代码为 `556658e065fa63abcf53404b1205e5e624009086`，交付入口为 [宿主 PR #101](https://github.com/T-miracle/Nanobug/pull/101)和[市场工单 #5](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/issues/5)；合并及议题关闭状态以这些链接的真实记录为准。最终复核于北京时间 2026-10-12 完成。

## 固定点与输入

删除阶段比较点为 `1bfc4cf10b3a67edb43f4f01003f1c6e2592cec2`；回归修复比较点为 `0b33b7f91971d883b44a35dd5240266bd6a52431`。全部实施在 `codex/marketplace-versions` 隔离工作树完成，未在主工作区编辑、暂存或提交其既有改动。删除前依赖及覆盖映射见 [inventory](plugin-marketplace-04-inventory-2026-10-11.md)。

[固定输入](../../../crates/plugin-runtime/tests/fixtures/packages/README.md)含 21 个 ZIP 和两个正式 Pages 快照；23 项实际 SHA-256 核对均匹配。初批七个已审核 Release 保持原版本、ID、文件名和字节。新增的 SDK Launch/Canvas 是最小独立公开 SDK 消费者，只作为测试输入，不发布为市场插件。原 Alpha/Beta 验收变体保留原字节；原源码、SDK 和工具链摘要未知，继续在 [ledger](../../../crates/plugin-runtime/tests/fixtures/packages/inputs.json)逐项注明。

正式短组合的冻结目录仍对应注册表 `8d1a8ca28d5c7a6a88dd2c93c2f9ce4598c38b99`，目录 SHA-256 为 `5c5ce9e3a992c92ea7ca84db5340bec4df33c25294089b14454ed86aeb931adf`；统计测量时间为 `2026-10-11T07:53:39Z`，摘要为 `280e8e1333598baefeb1dfbaba02cd3336e71a686680de6b3e4549258c185726`。测试替换传输地址为受控 HTTP，保留正式元数据及包字节。

Markdown 回归使用独立发布的 0.17.2：源码 `8480eb38e059d575cb5dafd740ed14f035d20936`，ZIP 616350 字节，SHA-256 `cac840637b5348c553da810e0d101f318088714caec18e30710bbc97c8327e13`。公开 SDK 来源仍为 `b77e6317bf7e3d3d3d0fe9490cabd362d266e0a5`，SDK 摘要仍为 `dcac05a1312eb110b7343b8b460fd9d5b6cefe3ad27753e78c519b7b85779e4c`。详见[独立包验收](https://github.com/T-miracle/Markdown-for-Nanobug/blob/985ea81ed762ea7a62f371ec4dfdf273cc9f0fe4/docs/verification/22-marketplace-regressions-2026-10-11.md)。

## 最终行为与修复

- 新安装不携带、扫描、推荐或自动安装插件，仍可使用纯文本、默认主题及内置终端。市场与本地导入共用公开 Package、Manager、InstallControl 和权限确认；已有插件、禁用选择、身份及私有数据继续读取。
- 删除 14 个迁出目录的 285 个跟踪文件。grammar/query、图标、WASM、迁移及原生测试改用摘要固定的真实包；SDK 补全、运行和 Canvas 夹具独立使用公开构建入口，不添加本地宿主路径依赖或 SDK 副本。
- 修复受控原生启动对显式 PATH 的解析，先校验授权，再按规范化目录解析；Windows 键名按大小写无关处理，不更改全局环境。真实子进程回归保留 Unicode 和字面量 argv，并确认撤销。
- 配置、Preview、NativeEditor、viewport 回执和实例撤销测试迁到现行公开契约，保留权限、revision、故障和生命周期断言。结构提供者在违规后撤销；测试不再期待退休实例继续服务。
- Markdown 在局部重解析中保留任务节点身份和焦点，在构造解析器前检查输入预算；超限保留只读预览及原生可编辑源码，恢复小文档或空文档时清理限额状态，不提高 fuel 或节点配额。
- 安装提示使用真实双语插值；详情在有界区域滚动，SDK 选择、执行确认和取消留在可见 footer。普通进度初始为 200 高，异步确认到达后只扩窗一次，保留用户较大的窗口；未放宽原紧凑窗口断言。
- 插件恢复按钮保留在诊断滚动区外，真实 Enter 在 mouse-up 前即触发恢复；链接键盘定位等原生布局完成后再清除待显示目标。
- 录制 UI 测试不启动环境中的默认在线目录检查；此限制只存在于 `cfg(test)`。显式 HTTP 夹具、真实后台 actor 和生产传输保持原链路。升级测试等待实际安装版本发布；ConPTY 回归等待完整重绘，保留原历史次数与占用行断言。

## 共同门禁与实际原生回归

Windows x86_64 MSVC，使用既有 Rust/Cargo 1.98.1、Visual Studio 工具链及准备好的原生依赖；未安装新的全局编译器或 SDK。

| 检查 | 最终结果 | 任务内日志 |
| --- | --- | --- |
| `cargo fmt --check` | 通过 | 交付命令输出 |
| `cargo test --workspace --exclude editor-app` | 238 passed / 0 failed / 188 ignored | `target/marketplace-04-non-ui-closure.log` |
| `cargo check --workspace` | 通过 | `target/marketplace-04-check-closure.log` |
| 全部应用测试，含全部 ignored | **686 passed / 0 failed / 0 ignored**，855.91 秒 | `target/marketplace-04-app-closure2.log` |
| 全部 runtime ignored，50 个测试二进制连续执行 | **188 passed / 0 failed / 0 ignored** | `target/marketplace-04-runtime-ignored-final.log` |
| runtime 图像组，同进程串行 | 14 passed / 0 failed / 0 ignored，70.02 秒 | 同上 `ui_images` 段 |
| runtime structure 组 | 3 passed / 0 failed / 0 ignored，26.56 秒 | 同上 `structure` 段 |
| Markdown 独立单元 / 真实包原生集成 | 76 / 88 passed；原生 88 项亦包含在最终 686 项中 | 独立包记录及应用日志 |
| 原生市场全部场景 | 14 passed；包含在最终 686 项中 | 应用日志 |
| SDK 迟到确认 / 紧凑进度窗口 | 各 1 passed；真实默认尺寸，无测试侧 resize | `marketplace-04-late-consent-closure2.log`、`marketplace-04-compact-progress-green.log` |
| `cargo build -p editor-app --release` | 通过，37.98 秒 | `target/marketplace-04-exe-closure2-build.log` |
| 市场 Python / 正式 CI | 13 passed；正式 CI 及 Pages deploy 均成功 | [Actions 38154042902](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/actions/runs/38154042902) |

普通 workspace 命令中 188 项 ignored 不计通过；其实际执行由单列的 runtime ignored 结果证明。应用最终命令为：

```powershell
cargo test -p editor-app --release --no-run
# 保留在 deps 中，避免破坏原生 fixture 的相对定位。
Copy-Item target/release/deps/editor_app-93e7ac438177c654.exe target/release/deps/marketplace04-app-closure2.exe
$env:NANOBUG_UPGRADE_QA_ROOT = 'C:/Users/Tmiracle/.codex/tmp/market04-closure-suite2'
$env:ME_EDITOR_PROFILE_HOME = "$env:NANOBUG_UPGRADE_QA_ROOT/profile"
target/release/deps/marketplace04-app-closure2.exe --include-ignored --test-threads=1
cargo test -p plugin-runtime --tests --release -- --ignored --test-threads=1
```

每个代码提交核对“此前提交＋index”；含其他待提交根因时导出 index 树隔离验证。最终应用二进制另从宿主树编译并核对绝对夹具路径。中间共享 target 的副本含隔离目录路径，造成 15 项缺原生输入；该运行不作为最终通过证据。统一增高窗口导致紧凑测试失败已修复。最终连续 686 项结果覆盖所有原失败名称。

## 保留的首轮失败历史

首轮为应用 36 项失败、runtime structure 一项失败，另有图像组同进程不稳定。在干净 `1bfc4cf`、同字节包及原断言中已复现。该归因没有作为关闭依据。

[失败账本](plugin-marketplace-04-native-failures-2026-10-11.json)保留原 `result`、`baseline_result` 和日志；新增 `final_result` 与最终日志。36 个应用名称逐项在最终通过日志中找到，structure 一项由连续 runtime 结果覆盖。未删除测试、扩大 ignore、提高资源配额或通过插件 ID 增加宿主专属 API。首轮完整候选记录仍可从[原提交](https://github.com/T-miracle/Nanobug/blob/0b33b7f91971d883b44a35dd5240266bd6a52431/docs/plugins/verification/plugin-marketplace-04-2026-10-11.md)读取。

## 最终 Windows 产物与安装

发行载荷和便携解包均只有程序、图标和宿主许可三个文件，无插件目录或源码。直接调用 Cargo、Inno Setup 6.7.3 和归档工具，不调用退役脚本。

| 最终产物 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `dist/marketplace-04-final2/editor/Nanobug.exe` | 109918720 | `232e6548214d4aa7e78b2928813c26430a0d574904d052711f8d63ac6c40a648` |
| `dist/marketplace-04-final2/installers/Nanobug-Setup-0.1.0-x64.exe` | 21352622 | `b3f2ae1dfea7af757ef6b9efe425dcaab596b0c51bb3ea7bcc894072e905807c` |
| `dist/marketplace-04-final2/Nanobug-0.1.0-windows-x86_64.zip` | 30207862 | `1e58b8096005a130986a26a2ae8a40f2291548430feeb2e737c36fad76c45c44` |

最终程序完成便携 SDK 导出、干净安装、旧载荷安装、原位升级及两次卸载，六阶段均 exit 0；新安装和升级的 EXE 摘要与上表一致，安装后 SDK 导出也 exit 0。隔离用户的 40 个文件在每阶段保持原相对身份和 SHA-256；未递归删除旧应用目录的历史插件文件。结果为 `target/marketplace-04-closure2-install-results.json`，前后摘要在 `target/marketplace-04-closure2-install/private-{before,after}.json`。

实际安装使用独立验收 AppId/运行锁及无用户快捷方式的模板；载荷与 Files 规则保持生产模板。生产安装器保留运行保护；未关闭用户正在运行的 Nanobug。安装器载荷、SDK 检查与 GPUI 自动化证据分别记录，不冒充人工操作已安装 GUI。

## 正式市场与最终短组合

[Markdown PR #1](https://github.com/T-miracle/Markdown-for-Nanobug/pull/1)已合并，[Release v0.17.2](https://github.com/T-miracle/Markdown-for-Nanobug/releases/tag/v0.17.2)附件固定上述摘要；[市场 PR #7](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/pull/7)合并提交为 `5ba32ff21d16ffde5340d78f1236f651a9201be0`。正式 Pages 读回七个插件，Markdown 同时保留 0.17.2 与 0.17.1；新版本 asset ID `631092655`，旧版 `630116607` 的源 commit 和摘要不变。统计读回测量时间 `2026-10-11T16:10:50Z`，Markdown 当时 total 9，版本分别 1 和 8；下载次数不等于安装人数。

首次 PR CI 的 403 为未认证 GitHub API 限流。市场 `03924cd9929cc10492aac97c80c7ce42e631a043` 将 Actions 只读凭据限定到精确 HTTPS API 来源，拒绝跨域重定向，ZIP 不携带凭据。13 项测试及正式 PR/main CI 已验证；PR 仍仅使用可信基线代码、固定 inert Package 校验器和候选数据，不运行访客。

`unbundled_official_market_install_and_existing_user_upgrade` 在最终 686 项中通过：无随附启动、受控 HTTP 上的正式 TOML 0.2.3、原用户 0.2.2 与私有数据贯通原生确认、安装、高亮、红点、历史/日志、升级、目录失败门禁、恢复及撤回；正式统计快照和测量时间也实际断言。正式 Pages 的线上客户端消费另由 `marketplace_published_pages_installs_reviewed_release` 通过。

## M01–M26 结果

| 验收 | 最终依据 |
| --- | --- |
| M01–M04 | 01/03 的审核、来源/许可/归属、不可变 ZIP 与目录详情；本次 0.17.2 再经 PR/CI/Pages |
| M05–M07 | 搜索/分类/排序、无随附启动、离线结果及原生安装确认；最终市场组通过 |
| M08–M12 | 兼容历史版本、升级约束、增量权限、取消重试、完整 ZIP/清单校验；最终市场组通过 |
| M13–M16 | 缓存/刷新失败门禁、迟到结果、只检查更新及红点；真实 actor 回归通过 |
| M17–M21 | 实际统计/时间、撤回、公开权限/信任、升级事务与旧数据、七仓身份连续；01–03 及最终组合 |
| M22 | 源码删除的依赖闭包、固定输入和关键消费者完成；应用 686、runtime 188 全部通过 |
| M23 | 无随附发行、无自动推荐、隔离安装/升级/卸载及 40 文件数据保留通过 |
| M24 | 中英文、深浅主题、14/18/24 字号、键盘/焦点、中文输入与 composition、滚动及原生布局通过 |
| M25 | 三项 Rust 门禁、全部相关应用及实际 WASM、独立审查和 Windows 最终交付证据完整 |
| M26 | 正式 Release、审核 PR、Pages、统计及实际客户端消费均通过 |

细项矩阵引用 [01](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/blob/main/docs/verification/01-reviewed-install-2026-10-10.md)、[02](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/blob/main/docs/verification/02-versions-updates-2026-10-11.md)、[03](plugin-marketplace-03-2026-10-11.md)。未变化的六个插件和初批版本不重复构建；变动的 Markdown 在独立仓库重新构建、验包和验证。

## Standards

按 `1bfc4cf…0b33b7f` 完成删除与文档初审修复，再按 `0b33b7f…556658e` 独立复核最终代码：未解决规范违规 **0**。保留非阻断的 Duplicated Code 建议：几个 Markdown 测试重复分离并重排 LocateViewport，可后续合并测试辅助逻辑；本单不扩大重构。

## Spec

最终独立复核：产品契约缺漏 **0**、错误实现 **0**、范围蔓延 **0**。迟到确认提示 P2 已由真实默认窗口回归解除。最终连续测试、正式发布及安装结果补齐剩余交付条件。审查不替代行为测试，二者均有独立结果。

## 实测边界

本单 M24 使用原生 GPUI 自动化输入、composition 和实际布局；没有实测物理 OS IME 候选窗口。这不代表其他 run-config 规格的专项人工验收已完成。macOS/Linux、签名及公证未验证；没有发布新的宿主版本标签。父市场规格 #1 和历史 Nanobug #100 不因本单完成而关闭。
