# Spelling Checker / 拼写检查

An independent WASM checker publishes native diagnostics without an LSP. It reads the current unsaved document, marks the word `teh`, shows related source information, and proposes `the` through the normal native plan preview. This small policy is an example for authors, not a full dictionary.

独立 WASM 检查器通过稳定 `editor.diagnostics 1.0` 发布原生诊断，无需 LSP。当前示例策略检查单词 `teh`，定位关联来源并提出 `the` 修复；预览、确认、冲突和撤销由宿主现有事务完成。

From a built host, use its public SDK and package entry points:

```powershell
.\target\debug\editor-app.exe --plugin-cargo crates/plugin-runtime/tests/fixtures/community/spell-check/Cargo.toml build --target wasm32-wasip2 --release
.\target\debug\editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/community/spell-check --output target/community-language
```

Install the ZIP into a trusted workspace and approve its declared editor read/write, diagnostic and native-panel permissions. Invoke **Check spelling / 检查拼写**, then open **Problems / 问题** with `Ctrl+Shift+M`. Select a row to reveal its location or related source; choosing its fix opens a normal consent preview. **Clear spelling problems / 清除拼写问题** clears only this checker's collection. Changes, close/reopen and disable retire old revisions and actions.

SDK use, UTF-8 ranges, ownership, quotas and failures are documented in exported Rust source comments. No copied SDK or repository crate path is included in this project.
