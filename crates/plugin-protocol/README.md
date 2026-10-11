# Plugin SDK

公开契约的用途、权限、错误、限额、兼容与释放义务维护在 [Rust 源码注释](src/lib.rs)，供宿主和独立访客使用。多来源诊断见 [diagnostics](src/diagnostics.rs)，实验直接语言结果见 [direct](src/direct.rs)，标准 LSP 的实际能力边界见 [language](src/language.rs)，预览修复与只读源前提见 [workspace](src/workspace.rs)。

独立 SDK 只导出本 crate 的源码、Cargo 清单、WIT 与 JSON 协议资源，不内嵌或导出 Markdown。旧双语读者资料保留在 [documentation](../../documentation/README.md)，不替代当前源码契约。
