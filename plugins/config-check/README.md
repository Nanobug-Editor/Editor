# Configuration Checker / 配置检查

An independent WASM checker reads current unsaved documents and checks a workspace-relative `include=path` at the start of a line. A missing file produces a native error and a plan that creates it with `port=8080`. This sample policy does not interpret arbitrary project configuration formats.

独立检查器使用稳定 `editor.diagnostics 1.0`，无需 LSP。示例策略检查行首的 `include=path`；目标缺失时显示原生错误、关联来源和创建默认配置的修复。修复只写目标文件，源文档通过 `workspace.files 1.3` 的只读 `Plan.origin` 保持 revision 前提。

```powershell
.\target\debug\editor-app.exe --plugin-cargo plugins/config-check/Cargo.toml build --target wasm32-wasip2 --release
.\target\debug\editor-app.exe --plugin-package plugins/config-check --output target/community-language
```

Install into a trusted workspace and approve its declared editor read, diagnostics, workspace read/write and native-panel permissions. Invoke **Check configuration / 检查配置**, open Problems with `Ctrl+Shift+M`, and choose **Create default configuration / 创建默认配置**. Confirm only after reviewing the ordinary native plan. A changed or reopened source, changed target, cancelled preview or retired owner prevents stale work; the normal batch Undo removes a newly created file. Clear affects only this checker's collection.

接口、坐标、权限、限额和失败说明在 SDK Rust 源码注释中；本项目不附 SDK 副本或仓库路径依赖。
