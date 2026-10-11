# Key-value Language / 键值语言

This independent `.nbkv` language packages a small controlled native stdio LSP. It provides Unicode identifier references, a `port=oops` to `port=8080` action, completion and `setting(key, value)` signature help through standard LSP messages. No full compiler or language SDK is installed.

独立 `.nbkv` 场景使用 `language.lsp 1.4` 和原有受控进程/服务依赖接口。服务负责键值策略，宿主负责同步、取消、超时、有效单 LSP 选择、共享原生结果界面和预览事务。参数提示与直接表达式语言采用同一原生消费者。

```powershell
.\target\debug\editor-app.exe --plugin-cargo plugins/keyvalue-language/Cargo.toml build --target wasm32-wasip2 --release
.\target\debug\editor-app.exe --plugin-package plugins/keyvalue-language --output target/community-language
```

Packaging directly invokes Cargo for the plugin-owned server and puts its actual binary hash in the resulting service artifact. Install the ZIP in a trusted Windows x64 workspace and approve its declared editor read/write, workspace read, dependency preparation and controlled analysis-process permissions. Use `Shift+F12`, `Ctrl+.` and `Ctrl+Shift+Space` in `.nbkv`. Actions require the normal native consent preview; the service's unrelated commands and external edits are not automatically executed.

The server keeps bounded synchronized text snapshots and accepts normal `$/cancelRequest`; cancelling a request does not kill the server. Close/reopen and owner retirement invalidate old results. Public SDK use and lifecycle boundaries are documented in source comments, with no repository path dependency or SDK copy.

The shipped optional WASM hook supplies dynamic initialization settings. Authors using fixed declarative settings can omit that component and set `hook: false`; the same granted and negotiated current-document code actions still use native preview and Undo. A missing `editor.write` grant or `workspace.files 1.3` contract rejects the effect, and disable or shutdown revokes the original native service seal immediately.
