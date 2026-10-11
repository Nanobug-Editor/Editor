# 插件市场 04：无随附交付与覆盖迁移

状态：实施候选，验收存在未解决项，[市场 #5](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/issues/5)保持开启。七包收录及线上发布属于已交付的 [03](plugin-marketplace-03-2026-10-11.md)，本记录不改变首批固定版本的审核结论。

## 比较点与输入

宿主比较点为 `1bfc4cf10b3a67edb43f4f01003f1c6e2592cec2`，工作分支为 `codex/marketplace-versions`。全部实施在隔离工作树完成；主工作区已有改动、暂存区和真实用户数据未纳入。删除前盘点及迁移对应关系见 [inventory](plugin-marketplace-04-inventory-2026-10-11.md)。

替代输入为 [packages](../../../crates/plugin-runtime/tests/fixtures/packages/README.md)中的 18 个真实 ZIP 和两个正式 Pages 快照；共 20 个 SHA-256 已逐项核对。七个正式 ZIP 的版本、ID、文件名和字节与已审核的独立 Release 相同。六个退役示例和历史 Rust/Rust Debugger/TOML 只用于测试，不进入市场、应用或 SDK。原 Alpha/Beta 配置验收变体保留原字节；其原源码/SDK/工具链摘要未知，在 [ledger](../../../crates/plugin-runtime/tests/fixtures/packages/inputs.json)逐项注明，不能推断与通用示例等价。

正式目录快照为注册表提交 `8d1a8ca28d5c7a6a88dd2c93c2f9ce4598c38b99`，SHA-256 为 `5c5ce9e3a992c92ea7ca84db5340bec4df33c25294089b14454ed86aeb931adf`；统计快照的测量时间为 `2026-10-11T07:53:39Z`，SHA-256 为 `280e8e1333598baefeb1dfbaba02cd3336e71a686680de6b3e4549258c185726`。测试只把传输地址改为本地受控 HTTP，不以另造元数据代替正式快照。

## 实际变化

- 生产启动不再扫描可执行文件旁或开发目录中的插件 ZIP，不再按打开文件自动推荐或安装。普通市场与本地导入继续使用公开 Package、Manager、InstallControl 和原生权限确认。
- 在验证替代输入后，移除 14 个插件/示例目录的 285 个跟踪文件；用户已装插件、身份、禁用选择和私有数据继续读取。开发隔离配置也不再自动安装随附包。
- grammar/query、manifest、图标、实际 WASM、旧版迁移和 native 测试改从摘要固定的 Package 读取。共享加载器仅在测试中使用，保留原来的行为断言、权限和生命周期接缝。
- SDK 补全改为最小独立项目，仍断言七类公开 UI 类型。该项目声明独立 workspace，使用宿主 SDK 缓存，没有本地 SDK 源码副本或 path 依赖；开发目录场景使用公开 stage/read_directory。
- Windows 安装载荷只含程序、图标和宿主许可；根双语 README、安装器说明及插件资料入口同步更新。构建和打包直接调用工具，没有恢复退役脚本或文档站。

## 共同门禁与原生测试

本次使用 Windows x86_64 MSVC、现有 Rust/Cargo 1.98.1 及已准备的原生依赖。没有新增全局编译器或 SDK。

| 实际检查 | 结果 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo test --workspace --exclude editor-app` | 237 passed / 0 failed / 188 ignored；ignored 不算通过 |
| `cargo check --workspace` | 通过，3.81 秒 |
| `cargo test -p editor-app` | 500 passed / 0 failed / 185 ignored，128.02 秒；后续移除空条件及修正文档不改变行为，追加编译和受影响 ignored 验证 |
| `cargo build -p editor-app --release` | 最终源码状态通过，34.50 秒 |
| 新启动、本地显式导入、旧选择与 actor 失败 | 20 个原生用例通过，149.82 秒 |
| 原 Alpha/Beta 配置变体、树、故障和 rollout | 13 个实际 WASM/native 用例执行，9 passed / 4 failed；四项在正确原始输入的基线中复现 |
| SDK 补全、真实 LSP 生命周期 | 七类补全通过，51.50 秒；LSP 11 项通过 |
| 显式隔离配置/升级资料准备与 Neovim Unicode/resize | 三项实际执行通过；两项是准备夹具，不冒充人工 GUI 验收 |

运行时的 188 个 ignored 场景按冻结输入逐项显式执行。第一组 capability/code-highlighting/composable-layout/UI/data-migration 为 29 passed；development 为 2 passed，editor-edit 为 11 passed；editor-images 共 10 项取得通过结果，后四项各自独立执行。其余 39 个二进制首次为 133 passed / 3 failed；复核后逐项共有 187 项至少一次通过，结构提供者一项仍失败。这个逐项数字不等于连续运行完整套件通过，失败没有改为 skipped。

ui_images 的同进程串行组为 12 passed / 2 failed，477.35 秒；失败为 workspace-switch 和 scheme-case 的 Ready 断言。二者在独立进程各为 1 passed，63.79/32.30 秒；先前 document-close/revision 两项独立复验也通过。保留严格断言并补充公开资源状态诊断；测试包含共享全局工作池，同进程稳定性仍待排查，没有把独立通过冒充整组通过。

structure 的串行组为 2 passed / 1 failed，363.56 秒。`independent_structure_validates_replies_and_denies_io` 在 malformed/quota 输入后出现 `Structure provider has been retired`；固定点原代码使用同字节包也为 0 passed / 1 failed，65.75 秒。该失败属于已复现基线问题，仍保留断言，不宣称解决。

应用的 185 个 ignored 场景已取得逐项执行结果：149 passed / 36 failed。失败清单及所对应的基线日志见 [native-failures](plugin-marketplace-04-native-failures-2026-10-11.json)。这些结果中不包含基线执行；也不把修复夹具路径前的假失败作为最终结果。

首次把 Windows 测试程序复制到 target 根目录，导致其相对定位的 LSP fixture 缺失；在原 target/debug/deps 位置重测 11 项全部通过。SDK 独立项目的 nested workspace 归属错误已修复并取得通过。Alpha/Beta 原包恢复后重新跑了相应消费者，没有用通用包替换其 WASM，也没有削弱断言。

## 已复现的基线失败

在独立、干净的 `1bfc4cf` 工作树使用同字节 ZIP 和原测试断言复核：

| 失败覆盖族 | 数量 | 实际基线结果 |
| --- | --- | --- |
| Markdown 工具栏/输入延迟、导航请求、源码布局、任务交互及过期状态 | 14 | 相同断言失败；Markdown 全组当前为 62 passed / 14 failed |
| native build/discovery/run、typed editor request、tools、outline/XML 组合 | 16 | 相同投影、等待或布局断言失败 |
| 配置 rollout | 4 | 恢复原 Alpha/Beta 字节后 0 passed / 4 failed，239.35 秒；此前缺夹具的结果不作为归因证据 |
| composed WASM preview、旧 installer prompt | 2 | 相同基线失败；LSP 夹具定位正确时仍复现旧 prompt 问题 |

应用共 36 项未解决，另有上述一项 runtime structure 基线失败及图像组稳定性问题。基线复现说明本次路径迁移没有引入这些已观察失败，并不证明相关功能正确，也不构成忽略测试或关闭本单的依据。没有为这些失败改生产 UI/API、删除断言或扩大 ignore。它们继续保留在回归资产中。

## 最终 Windows 产物

最终载荷只有 `Nanobug.exe`、`Nanobug.ico`、`licenses/alacritty-terminal-LICENSE-APACHE`。便携 ZIP 解包后核对同样三项；实际便携 EXE 的 `--export-plugin-sdk` 返回 0，SDK 不含 Markdown 或 ZIP。

| 产物 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `dist/marketplace-04/editor/Nanobug.exe` | 109853184 | `5208da4226101dfe078622eaadf59e21774e0bee3e0a068da581e2f74870632d` |
| `dist/marketplace-04/installers/Nanobug-Setup-0.1.0-x64.exe` | 21377325 | `29a0b1feb91b1040efedccb58dd4c1e29a235ac3a17a144459b10068c4ae105e` |
| `dist/marketplace-04/Nanobug-0.1.0-windows-x86_64.zip` | 30207826 | `fa7e6183e2e4051a25b1f7162b751c03abc27f0f20b0a1ff0af14979eebd1e85` |

直接使用现有 Inno Setup 6.7.3 编译。最终 EXE 经独立验收身份的安装器完成干净安装、旧载荷安装、原位升级和卸载，均 exit 0；安装的 EXE 摘要与上表相同，干净安装没有 plugins 目录。每阶段核对隔离用户的 20 个设置/安装/私有文件，SHA-256 均保持不变；旧应用目录中的历史插件文件没有被递归删除。

隔离副本只调整 AppId、运行锁、快捷方式及语言文件位置，载荷与 Files 规则保持生产模板；不使用真实用户的安装身份。第一次使用生产运行锁的尝试检测到已运行的 Nanobug，正确取消安装，随后使用隔离身份完成目录验收。生产配置仍保留原运行保护；未关闭用户应用或操作其未保存文档。

## 最终短组合与 M01–M26

`unbundled_official_market_install_and_existing_user_upgrade`：1 passed / 0 ignored，1.89 秒。实际公开 Worker/Manager、正式 TOML 0.2.3 ZIP、原用户 TOML 0.2.2 和私有数据贯通无随附启动、目录刷新、原生确认、安装、高亮、升级红点、历史/日志、升级、目录失败门禁、恢复及撤回。统计使用上述正式快照并验证测量时间。英文/深色 14 字号和中文/浅色 18 字号通过原生输入、焦点、滚动及真实布局断言。

| 验收 | 当前结论与证据 |
| --- | --- |
| M01 | 01/03 已交付：PR 收录与本批维护者授权，见 03 |
| M02 | 01 的来源/许可/归属校验已交付 |
| M03 | 01/02 的同版本摘要与实际包校验已交付；本单固定输入逐项摘要核对 |
| M04 | 01/02 已交付：正式目录/详情直接消费 |
| M05 | 01 已交付搜索/分类/排序；本单组合再验证原生搜索 |
| M06 | 无随附启动、无缓存失败、纯文本/默认主题及内置终端自动化通过 |
| M07 | 01/03 的真实包和原生权限确认已交付；本单组合复验 |
| M08 | 02 已交付兼容稳定版及历史选择 |
| M09 | 02 已交付升级限制；本单保留显式本地导入 |
| M10 | 02 的权限增量确认已交付 |
| M11 | 01/02 的进度、取消、重试已交付 |
| M12 | 01/02 的完整 ZIP 摘要和实际清单拒绝已交付 |
| M13 | 01/02 已交付失败门禁；本单组合复验故障/恢复 |
| M14 | 01/02 的缓存/时间/迟到结果门禁已交付 |
| M15 | 02 已交付检查调度，不自动升级 |
| M16 | 02 已交付红点；本单原生标题栏组合复验 |
| M17 | 02/03 的正式统计已交付；本单读取固定实际快照及更新时间 |
| M18 | 02 已交付附件失效/撤回；本单复验撤回不卸载 |
| M19 | 01/03 已交付公开权限/信任/资源接缝；本单启动/导入回归通过 |
| M20 | 02 的升级事务已交付；本单旧版本身份与私有数据复验 |
| M21 | 03 已交付七仓独立构建、身份连续和合并 Rust |
| M22 | 迁移和源码删除已实施；原断言保留，实际 native 失败仍未解决 |
| M23 | 无随附载荷、无自动推荐及隔离安装/升级/卸载通过 |
| M24 | 原生 GPUI 自动化组合通过；实际已安装 GUI 的人工操作系统 IME 未实测 |
| M25 | 实际 WASM 场景已显式执行；36 项应用失败、一项 runtime structure 失败及图像组稳定性阻止宣称全面通过 |
| M26 | 03 已交付正式 Pages/Release/统计链路；本单未修改正式注册表或 Release |

01/02 的细项矩阵分别见 [01](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/blob/main/docs/verification/01-reviewed-install-2026-10-10.md)和 [02](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/blob/main/docs/verification/02-versions-updates-2026-10-11.md)。未变化的七包源码和业务输入复用 03 的独立构建与代表能力记录，没有再构建相同正式 ZIP。最终组合使用本次源码编译的 GPUI 测试程序；安装器/便携 EXE 的安装、载荷和 SDK 检查另行实测，没有把这两条证据混称为人工操作已安装 GUI。

## 收尾限制

工单 04 保持开启：待处理上述实际失败、图像组稳定性及完成独立 Standards/Spec 审查。尚未将本单实施候选合入 main 或发布宿主版本。macOS/Linux、人工 OS IME 和签名/公证均未验证。前单七包正式市场审核与发布已完成，不受本单开放状态影响。
