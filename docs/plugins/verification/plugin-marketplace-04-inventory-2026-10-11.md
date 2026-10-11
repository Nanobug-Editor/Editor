# 插件市场 04：删除前依赖盘点

状态：准备中，尚未删除源码或改变发行。依据市场 [工单 04](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/issues/5)；盘点宿主固定点为 `b77e6317bf7e3d3d3d0fe9490cabd362d266e0a5`。工单 03 的正式收录及行为验收仍是实施前置条件。

## 实际输入

`plugins/` 中有 14 个 `nanobug-plugin.json`：七个正式插件 `html`、`javascript`、`markdown`、`rust`、`svg`、`toml`、`xml`；已合并的历史 `rust-debugger`；六个示例 `example`、`capability-example`、`configuration-example`、`layout-example`、`tools-example`、`run-target-example`。Image 继续使用安装 ID `svg`。

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

## 尚待执行

先核对每个候选消费者及其测试过滤词，记录固定正式包和专用夹具的覆盖对应关系。验证替代输入后，再取消生产随附与推荐、移除过渡业务源码及六类示例、更新发行资料。最后执行共同三项门禁、受影响应用/实际 WASM 验收和 Windows 短组合。当前没有宣称这些步骤通过，也不关闭工单 04。
