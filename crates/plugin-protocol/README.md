# Plugin SDK

公开契约的用途、权限、错误、限额、兼容与释放义务维护在 [Rust 源码注释](src/lib.rs)，供宿主和独立访客使用。HTTP、密钥、私有状态与计时器分别见 `src/http.rs`、`src/secrets.rs`、`src/storage.rs` 与 `src/timers.rs`。

独立 SDK 只导出本 crate 的源码、Cargo 清单、WIT 与 JSON 协议资源，不内嵌或导出 Markdown。旧双语读者资料保留在 [documentation](../../documentation/README.md)，不替代当前源码契约。
