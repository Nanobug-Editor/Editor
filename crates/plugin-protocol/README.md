# Plugin SDK

公开契约的用途、权限、错误、限额、兼容与释放义务维护在 [Rust 源码注释](src/lib.rs)，供宿主和独立访客使用。多来源诊断见 [diagnostics](src/diagnostics.rs)，实验直接语言结果见 [direct](src/direct.rs)，标准 LSP 的实际能力边界见 [language](src/language.rs)，预览修复与只读源前提见 [workspace](src/workspace.rs)。

SDK 只导出源码、Cargo、WIT 与 JSON 构建输入，不包含此 Markdown 或其他仓库读者资料。

HTTP、密钥、私有状态、计时器、通用树与编辑器装饰分别见 [http](src/http.rs)、[secrets](src/secrets.rs)、[storage](src/storage.rs)、[timers](src/timers.rs)、[tree](src/tree.rs) 与 [decorations](src/decorations.rs)。

默认按命令、语言、视图或服务需求启动，后台用途须明确声明，见 [activation](src/activation.rs)。结构化配置、变化通知、适用的热应用与密钥引用见 [settings](src/settings.rs)；宿主版本下限、稳定同 major 演进、实验能力及迁移义务见 [lib](src/lib.rs)。包版本、传输协议、能力版本和宿主版本分别处理。

独立 SDK 只导出本 crate 的源码、Cargo 清单、WIT 与 JSON 协议资源，不内嵌或导出 Markdown。

- [SDK 源码与使用注释](src/lib.rs)
- [组件接口](wit/plugin.wit)
