# 插件市场 04：删除前依赖盘点

状态：实施与验收中。七个正式独立来源已完成工单 03 的收录、安装和线上验证；工单 04 已取消生产随附扫描/推荐及发行载荷，并在关键替代输入验证后移除 14 个源码目录的 285 个跟踪文件。依据市场 [工单 04](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/issues/5)；删除前盘点源为 `b77e6317bf7e3d3d3d0fe9490cabd362d266e0a5`，本单比较点为 `1bfc4cf10b3a67edb43f4f01003f1c6e2592cec2`。未解决验收项不关闭工单。

## 实际输入

删除前 `plugins/` 中有 14 个 `nanobug-plugin.json`：七个正式插件 `html`、`javascript`、`markdown`、`rust`、`svg`、`toml`、`xml`；已合并的历史 `rust-debugger`；六个示例 `example`、`capability-example`、`configuration-example`、`layout-example`、`tools-example`、`run-target-example`。Image 继续使用安装 ID `svg`。移除只作用于隔离工作树中的跟踪源码；本机主工作区、用户已安装实例、设置和私有数据不参与删除。

对 crates、Cargo 清单、根双语 README、installer 执行 `rg -l 'plugins[/\\]|join\("plugins"\)|bundled'` 并限制源码/清单/发行文档类型，得到 106 个候选文件。该集合包含正常用户包目录、历史说明和普通 bundle 字样，不能将全部结果机械删除；需逐项区分实际源码输入与用户持久化路径。任务本地原始候选清单为 `target/marketplace-04-input-consumers.txt`，不是交付门禁的通过记录。

## 覆盖迁移安排

| 现有依赖 | 替代输入与保持的行为 | 实施后的必要验证 |
| --- | --- | --- |
| 正式语言 grammar/query、清单、图标资源；language、outline/folding 和编辑测试直接读取 `plugins/*` | 从固定版本正式 ZIP 读取验证过的 Package 资源；协议格式测试使用专用最小声明夹具 | 动态识别/高亮、查询与折叠、实际安装及语言编辑回归；不能恢复内建 grammar |
| Markdown/Image/XML/Rust 的真实 WASM 及 native 场景，`dist/plugins/*.zip` 测试输入 | 七个独立仓库已经发布的固定 ZIP；版本、源码及 SHA-256 明确记录。测试输出目录不作为生产 fallback | 迁移后重跑受输入路径变化影响的 native/ignored 消费者，引用未变化业务的既有完整验收 |
| 六类示例及运行目标示例参与 SDK、权限、UI、存储、执行与生命周期测试 | 在公开 SDK/Package/Manager 接缝准备专用测试夹具，保留每项原有可观察断言 | 先验证替代夹具，再删除原 `plugins/` 示例目录；不得删除测试、扩大 ignore 或提供插件专属宿主 API |
| 历史 Rust/Rust Debugger 升级及 UI/data 迁移输入 | 保留真实旧版本冻结包，只作为迁移测试，不上架，不以新包别名替代旧版本 | 安装身份、设置、私有数据、作用域和失败恢复；删除源码不卸载用户历史实例 |
| `app/distribution.rs`、`extensions/bundled*`、worker/surface 的随附扫描、推荐与安装路径 | 新安装只保留用户主动打开的在线市场及本地导入；现有用户的管理器数据继续读取 | 空安装、首次离线、没有语言插件的纯文本和默认主题、终端；市场失败门禁 |
| `installer/windows/nanobug.iss` 的 plugins 递归载荷及便携目录说明 | 安装器/便携包只携带应用所需文件及许可证，不依赖同级 plugins | 干净载荷检查、实际 Windows 安装/升级、旧用户目录保留及组合验收 |
| 根 `Cargo.toml` exclude、SDK/development CLI、根双语 README 与 installer README | 明确独立插件仓库、公开 SDK、直接 Cargo/宿主/归档/安装器工具入口 | 检查现行路径/命令/链接、无退役脚本或文档站依赖 |

## 已建立的替代输入

固定输入位于 [packages](../../../crates/plugin-runtime/tests/fixtures/packages/README.md)，18 个真实 ZIP 加正式目录及统计快照共 20 个输入按 [ledger](../../../crates/plugin-runtime/tests/fixtures/packages/inputs.json) 核对 SHA-256。七包来自独立稳定 Release；六个示例来自记录的宿主源码/SDK；旧 Rust、Rust Debugger、TOML 只用于升级回归，不上架。另外保留原 Alpha/Beta 专用验收包，不能从通用示例推断其可互换；两包原 SDK/源码/工具链摘要缺少记录，逐项注明未知，不套用全局构建基线。资源消费者先通过公开 Package 检查再提取到临时根，不恢复内建 grammar。SDK 补全改用嵌套的最小独立项目，保留全部七类公开 UI 类型断言；开发目录测试通过公开 stage/read_directory 接缝取同一冻结示例。

| 覆盖族 | 迁移后的入口 | 已取得的关键结果 |
| --- | --- | --- |
| grammar/query/动态语言、outline/folding、XML/HTML 编辑 | test_packages::root/text；固定正式 ZIP | 独立资源加载通过；应用默认套件 500 通过、0 失败、185 ignored；ignored 另行执行 |
| 首次启动/旧随附选择/本地导入/受限恢复 | markdown_tests::distribution、worker::bundled_tests | 20 个原生用例通过，六个配置/故障用例通过；无自动随附安装 |
| 公开 SDK、权限、编辑请求、UI、数据迁移 | frozen Package → 公开 Manager；原有断言保留 | capability 2、code_highlighting 9、composable_layouts 1、composable_ui 12、data_migration 5 通过；剩余实际 WASM 扩展仍在汇总 |
| 真实正式安装、升级、失败门禁与撤回 | worker::marketplace::tests::delivery | 两个 locale/theme 场景均使用原生输入、真实 HTTP/ZIP 和 Manager；已通过，新增字号缩放复验另记 |
| Rust SDK 补全、native LSP 生命周期 | readiness_tests 最小独立项目；真实 lsp_fixture | 七类类型补全通过；原生 LSP 11 个用例通过 |
| Windows 载荷和用户数据 | 实际 ISCC/ZIP、独立 AppId 的安装/升级/卸载 | 应用载荷不含插件；安装、旧载荷升级和卸载均 exit 0，20 个隔离用户文件摘要不变 |

## 失败归因及收尾

为核对迁移前行为，在固定点 `1bfc4cf` 的独立工作树准备同字节 ZIP，再使用原测试断言运行。Markdown 14 个失败均在该基线复现，旧安装确认用例也复现相同失败；该基线的 SDK 原用例通过。另一个六项探针复现旧运行投影、布局恢复及 composed UI 的失败。完整失败类别与命令以本单验收记录汇总，不能把这些失败或 ignored 算作通过。

曾将测试程序复制到 `target/` 导致原生 LSP fixture 相对查找失效，已在正确的 `target/debug/deps` 位置重测并得到 11 项通过。运行时 image 生命周期共享全局工作池；并发失败需独立串行复验。剩余门禁、审查和平台限制仍须读回真实结果，工单目前保持开启。
