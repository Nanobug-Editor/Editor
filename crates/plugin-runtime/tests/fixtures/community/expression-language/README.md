# Expression Language / 表达式语言

This independent `.nbexpr` language uses experimental `language.direct 0.1` pure WASM callbacks for references, actions and signature help. It runs no LSP or native service. Unicode identifier references, an `add(` to `sum(` proposal, and `sum(left, right)` parameter data are plugin policy; the host supplies shared native presentation and normal edit transactions.

独立 `.nbexpr` 场景通过实验能力 `language.direct >=0.1,<0.2` 提供引用、代码动作及函数参数提示。能力缺失时 required 声明明确拒绝安装；使用 optional 的其他作者必须省略未协商调用。实验 minor 不承诺稳定兼容。

```powershell
.\target\debug\editor-app.exe --plugin-cargo crates/plugin-runtime/tests/fixtures/community/expression-language/Cargo.toml build --target wasm32-wasip2 --release
.\target\debug\editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/community/expression-language --output target/community-language
```

Install into a trusted workspace and approve the declared read/write, workspace read and native-panel permissions. In `.nbexpr`, use `Shift+F12` for references, `Ctrl+.` for actions, and `Ctrl+Shift+Space` for parameter help. A code action enters the ordinary native preview; it never edits from a pure callback. Switching tabs/providers, close/reopen, cancelling or disabling prevents old results from being delivered or selected. Valid saved provider choices survive later installations.

Callbacks return bounded immutable plain data. Ownership, minimum capability versions, source revisions, cancellation and quotas are documented in the exported SDK Rust comments; this project includes no SDK copy.
