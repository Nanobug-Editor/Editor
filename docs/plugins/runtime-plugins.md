# 运行时插件平台

插件完整构建／ZIP 和隔离开发运行使用宿主 `--plugin-package`、`--plugin-build`、`--plugin-dev`；GUI 提供对应内建模板。现行命令见[根 README](../../README.zh-CN.md)，共享项目规则见[已确认方案](specs/plugin-development-packaging.md)。原仓库辅助脚本已移除，归档副本不参与构建。

主程序只运行当前能力协议（清单 protocol 7、api.base ^1）。插件使用声明式贡献，必要时附加 WASM 生命周期或策略钩子。语言识别、高亮、LSP、UI、进程和插件间服务按声明与能力协商，不按插件名称选择实现。

## 安装与热生效

官方插件源码已迁入七个独立 X-for-Nanobug 仓库。全新安装仅含纯文本编辑、内置终端和默认主题，不扫描同级插件目录、不主动推荐；首次离线且无目录缓存时显示市场获取失败。既有安装、身份、配置与私有数据仍由原管理器读取，删除仓库示例源码不自动卸载用户实例。审核、版本和 ZIP SHA-256 规则见[市场规格](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/blob/main/docs/specs/plugin-marketplace.md)。

标题栏“插件管理”提供已安装列表、审核后的在线市场、README、本地 ZIP 导入、安装、更新和卸载。确认来源、权限及所需原生安装步骤后，成功安装即可启用，无需重启编辑器；全局禁用时可明确在当前项目启用。LSP 依赖未就绪时单独显示原因，不把安装成功当作服务就绪。

语言、主题、图标与面板随安装、启停、更新和卸载同步到已打开的文档。面板隐藏、停用和卸载会释放原生输入目标并回收停靠空间。插件命令按清单注册；未安装或未启用时不会偷偷安装或扩大权限。

清单声明在左侧的普通面板默认加入现有侧栏，与资源管理器、大纲使用同一套独立分割、拖动和布局保存机制；用户保存的位置优先于清单的初始位置。没有现有左侧分区时使用通用侧栏回退，独立工作窗口仍须声明 `position=window`。

默认启用只发布静态贡献。命令调用、选中的语言功能、实际显示的视图及服务调用按需启动业务实例；查看清单、打开菜单或发现服务不启动它。旧 SDK 包省略激活声明时也使用这个默认值，声明式语言/主题/图标包无需空 WASM。连续后台工作通过 `activation.background.purpose` 和 `lifecycle.activation 1.0` 显式声明，仍受原信任、授权和资源预算限制。

首次请求保存原作用域、文档版本、权限与截止时间，并共享有界激活准备。取消、禁用、关闭及替换撤销原请求，迟到结果不能借用新 owner；初次创建 owner 不会错误撤销发起它的请求。具体错误、限额和释放义务见[激活源码](../../crates/plugin-protocol/src/activation.rs)。

结构化配置默认替换受影响实例；`configuration 1.1` 允许明确声明的热应用值及命名空间内的密钥引用。热应用先校验、持久保存，再向同一 owner 发送完整有效值的 Changed；应用失败明确报告并退役失败实例，不宣称恢复 WASM 内存或外部副作用。敏感值继续从系统密钥接口读取，普通配置只保存 key，不能授予权限或改变宿主语言、主题及信任。详见[配置源码](../../crates/plugin-protocol/src/settings.rs)。

稳定能力在同 major 保留消息与含义，新增 minor 功能须声明最低能力；实验 `language.direct 0.1` 独立标识。`minimum_host` 为完整 SemVer 下限，贡献清单 `host_version` 为范围，两者均独立于包/协议/能力版本。安装与恢复检查不兼容时保留既有设置、授权偏好和受管私有数据；协议 1–6 继续明确拒绝运行。

## 公共接口

WIT 世界 `editor:plugin/plugin@0.1.0` 提供 host.request 导入与 dispatch 导出。Rust SDK 0.2.0 使用下列公开类型，WIT 版本与各能力 SemVer 独立：

| 方向 | 契约 |
| --- | --- |
| 宿主 → 插件 | Invocation：Prepare、Activate、Event、Snapshot；事件含可选面板身份及 Notification |
| 插件 → 宿主 | Completion：请求 ID 对应的 Output 或 Failure |
| 插件请求宿主能力 | api::Request / Operation → Response；耗时请求先受理，再通过 Notification 返回最终结果 |
| 插件界面 | Output.views → 每面板 ui::Document；稳定节点 ID 与 revision 约束输入 |
| 插件间协作 | 版本化服务契约、方法结构与来源权限；消费者引用契约，用户可替换提供者 |

不再存在旧 Message/Reply/Scene、字符串宿主命令、chrome/controls 兼容字段或运行时转换。标准组件、SideTabs 与 Canvas 组成同一棵 UI 树，宿主的 gpui-base 行为和本地外观负责输入、焦点、布局、滚动、菜单及弹窗。SVG 预览接收带 DocumentVersion 的内存文本，必须回传 source，包含未保存编辑；过期结果不能覆盖新文档。

公开接口的现行说明见源码注释：[SDK](../../crates/plugin-protocol/src/lib.rs)、[UI](../../crates/plugin-protocol/src/ui.rs)、[语言](../../crates/plugin-protocol/src/language.rs)、[进程](../../crates/plugin-protocol/src/process.rs)、[服务](../../crates/plugin-protocol/src/service.rs)。宿主只导出 SDK 源码、Cargo 清单、WIT 和 JSON 协议资源；仓库 Markdown 不内嵌或随 SDK 导出。

文件预览可以通过公开 `editor.presentation` 声明源码／分栏／预览三态，通过 `editor.toolbar` 提供源码顶部操作；版本化范围编辑、图片输入、导航及视口请求分别核对文档身份、revision 和来源权限。Markdown 使用这些通用接口提供实时原生预览、任务框写回、同级图片导入和双向内容块滚动；SVG 使用同一底栏三态入口。详见[Markdown 使用说明](https://github.com/T-miracle/Markdown-for-Nanobug)、[导航](../../crates/plugin-protocol/NAVIGATION.md)、[视口](../../crates/plugin-protocol/VIEWPORT.md)和[代码块高亮](../../crates/plugin-protocol/CODE_HIGHLIGHTING.md)。新增插件仍只使用公开契约，不需要宿主按插件名称分支。

## 权限与原生执行

WASM 不继承宿主目录、环境、输入输出或网络访问。每次调用检查协商能力、安装授权、实例作用域与活句柄。包资源只读；工作区与私有数据使用受限根句柄，拒绝越界路径、设备路径、备用数据流与链接逃逸。工作区受限时撤销运行资源。

| 权限 | 范围 |
| --- | --- |
| assets.read | 当前包的只读资源 |
| workspace.read / storage | 获授工作区读取 / 当前实例私有数据 |
| editor.read / editor.write | 读取版本化文档信息 / 保存明确版本的文档 |
| ui.panels / clipboard | 自己声明的面板 / 原生剪贴板 |
| process.service.<id> | 清单固定服务；不能任意替换程序或参数 |
| process.exec | 使用程序路径与 argv 执行任意本机程序，支持 stdio 或 PTY |
| services.call | 调用声明的插件服务；完整调用链仍受来源权限约束 |
| dependencies.prepare / dependencies.install | 校验后准备私有依赖 / 经额外确认执行声明安装步骤 |

原生程序以当前 OS 用户身份运行，具有该用户的文件、网络与进程权限；WASM 权限不构成子进程文件沙箱。宿主经工具链解析程序并传 argv，不拼接 Shell 命令。插件自己的交互式 Shell 输入是用户授权的终端行为。

依赖在获授权的安装/更新流程中，或声明了 `on_demand` 的首次方法调用中，下载、校验并写入私有版本缓存。按需准备沿用已确认的依赖权限、原始调用期限和取消状态；原生安装步骤另行展示工具、参数及用途。不静默修改全局环境。取消只能停止尚未执行的请求、停止等待或尽力终止进程；不能承诺回滚已发生的外部副作用。

## 更新、故障与数据

1. 后台编译、验证候选包和私有依赖，旧实例继续处理输入。
2. 进入切换阶段，撤销旧异步请求与服务租约，获取最终快照和数据。
3. 在隔离副本执行数据迁移；仅允许访问包资源和私有副本。
4. 候选激活成功后以日志、备份和原子替换提交包记录与私有数据，再发布新实例。
5. 准备失败保留旧实例；切换失败恢复旧版本及数据并分配新实例身份，旧句柄和回调不会复活。

备份及恢复失败路径、休眠工作区升级见 [迁移契约](../../crates/plugin-protocol/src/lib.rs)。快照是插件的逻辑数据，不能恢复 OS 进程内存或撤销用户文档、外部文件等副作用。内置终端恢复有界历史和新 Shell；经 interactive.execute 服务创建的执行会话只恢复历史，不重跑程序。旧终端数据在任何组件激活前备份并导入，重复启动不覆盖已完成迁移后的数据。

WASM 指令、时间和内存有独立预算；队列、进程、文件、绘图和事件均有上限。超限定位到插件/作用域/操作；插件可独立重启，LSP 有限重试。停用与卸载结清请求、撤销订阅和租约并回收进程树。

默认数据根为 %APPDATA%/MeEditor/runtime-plugins，ME_EDITOR_PLUGIN_HOME 可指定隔离测试根。registry.json 保存安装与启用偏好，packages 保存不可变代码，scope 私有数据由宿主管理。历史 ID/目录只执行有限导入并保留备份。协议 1–6 的安装记录可展示更新、卸载及保留的偏好，但不运行旧代码；重装新版可导入保留数据。

## 构建与验证

当前插件构建通过实际主程序 CLI，完整候选由宿主校验并自动生成 ZIP，步骤见[根 README](../../README.zh-CN.md#插件开发)和[本次方案](specs/plugin-development-packaging.md)。仓库旧辅助脚本已移出，打包不调用这些脚本。

插件依赖主程序内嵌的 SDK，不引用宿主源码或复制 vendor。`--plugin-cargo` 按内容摘要管理接口缓存，插件声明 `plugin-protocol = { version = "=0.2.0", features = ["guest"] }`。显式 --export-plugin-sdk 供其他工具链使用；使用已编译插件无需 Rust/Cargo。

```powershell
cargo build -p editor-app
# LSP、安装器与进程契约使用本地原生夹具。
cargo build -p plugin-runtime --examples
# 完整构建 WASM／原生产物、资源和 ZIP。
.\target\debug\editor-app.exe --plugin-package ../Rust-for-Nanobug --output target/plugin-build
# 仅准备开发目录；实际重载测试使用此命令打印的目录作为夹具。
.\target\debug\editor-app.exe --plugin-build ../Rust-for-Nanobug
.\target\debug\editor-app.exe --export-plugin-sdk .\target\sdk-export
cargo fmt --check
cargo check --workspace
cargo test --workspace --exclude editor-app
# 宿主模板和 SDK 导出回归；其他实际交互测试按对应验收记录执行。
$env:RUST_MIN_STACK = '16777216'
cargo test -p editor-app plugin_development -- --test-threads=1
cargo test -p editor-app sdk_export -- --test-threads=1
```

真实 Package 测试输入已经按版本/摘要固定在 `crates/plugin-runtime/tests/fixtures/packages/`，准备或更换方法见其 README。实际 WASM 测试仍需显式执行所选 `--ignored` 场景；需要原生依赖时准备记录中的固定制品，不能以普通 workspace 测试的跳过结果计通过。目录候选的 `NANOBUG_DEVELOPMENT_CANDIDATE` 和 SDK ZIP 的 `NANOBUG_SDK_PACKAGE` 命令见[本次验收记录](verification/plugin-development-packaging.md)；不得用普通 workspace 测试的跳过结果替代真实验收。

现行插件项目均提供共享描述并独立打包，每包包含其 README。XML 包含结构回调、grammar 与 SVG 辅助 Schema；HTML、JavaScript 包含已构建的原生服务及许可。Rust 0.6.0 在一个 WASM 中提供 LSP 钩子、Cargo 配置/目标和调试服务；Windows x86_64 首次调试准备固定 CodeLLDB，其他平台仅打包语言与 Cargo。安装明确确认后，旧 `rust-debugger` 的启停状态和相关提供者选择通过通用有限替代事务保留，原私有数据及依赖收据保留。详见 [Rust 合并方案](specs/rust-plugin-consolidation.md)。仅提供语言资源的包无需空生命周期组件。Markdown 包同时包含两份 WASM grammar 和独立解析组件。终端为宿主内置原生模块，直接使用上游 alacritty_terminal；不构建或加载独立终端包，基础主题内置。

Windows 本次实际 WASM、目录重载及 GPUI 验收见[插件开发验收](verification/plugin-development-packaging.md)。既有 ConPTY、进程树、语言服务等平台完整矩阵见[历史最终契约验收](verification/plugin-api-contract-verification.md)，不计为本次重跑。macOS/Linux 未在本次环境实测或交叉构建，不以 Windows 结果替代其他平台验证。

原生安装器使用直接工具，步骤见[安装与分发](../../installer/README.md)。
