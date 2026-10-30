# Rust 真实包语法高亮覆盖检查

日期：2026-10-30。候选为独立 Rust 仓库的 0.6.3；宿主保留通用 parser 和 capture 优先级。

用户报告 `///` 第三个斜杠颜色错误。用原 0.6.2 ZIP 经过 `Package::read`、真实 WASM 和原生主题回放，观察到 `/` 与 `//!` 的 `!` 被等范围运算符捕获覆盖。独立插件在通用运算符后显式捕获 doc marker，同时补齐 21 项已观察到的带路径／泛型调用、宏、生命周期、常量声明、路径关键字和复合运算符遗漏。普通注释、嵌套块注释、字符串家族、数字、中文和 CRLF 作为对照。

继续逐项解析插件声明的 capture，发现 `namespace` 没有宿主主题映射。`editor-ui` 现在将该通用类别映射到类型路径色板；所有插件均可使用，没有按 Rust 或插件 ID 增加生产分支。

## 验证

- 独立插件的 10 项业务单测和公开打包／静态准入通过。实际 ZIP 为 `C:/Projects/RustProjects/Editor/dist/rust/rust-0.6.3.zip`，SHA-256 `a872d5cce2c609e2d873eccbb3baf5c52138a7e4c0592908811da3d939934327`。
- `rust_package_comment_delimiters`：逐字符检查三斜杠、内外文档注释、普通与嵌套块注释在两种主题中的完整颜色。0.6.2 失败，0.6.3 通过。
- `rust_package_syntax_coverage`：实际原生语法与主题解析验证代表性语法，并要求插件每个声明 capture 都有可见颜色。原包在两种主题中分别漏掉 21 项；最终候选全部通过。
- `rust_package_readonly_highlights`：独立提供者实例经生产只读高亮入口保留注释颜色、模块／函数／复合运算符及精确 UTF-8 范围；通过。
- `namespace_capture_has_a_visible_color_in_both_themes`：修复前失败，修复后通过。

```powershell
$env:NANOBUG_TEST_PACKAGE_INPUTS = 'C:/Users/Tmiracle/.codex/task-artifacts/rust-warning-20261030/rust-candidate-inputs.json'
cargo test -p editor-app --bin editor-app rust_package_ -j 2 -- --ignored --nocapture --test-threads=1
```

上述包测试因需要独立构建的候选 ZIP 而默认 ignored，本轮显式执行后为 3 passed / 0 failed / 0 ignored。账本通过既有 SHA-256 夹具入口指定候选，不修改仓库内不可变历史 ZIP，也不增加测试专用生产 API。

## 原生范围与限制

公开 `--plugin-dev` 在独立 profile 中安装、激活实际包；原生浅色窗口已人工核对注释、调用、生命周期及运算符颜色。用户随后物理 Esc 停止桌面操作，未继续原生深色交互及新宿主模块名颜色截图。最终两种主题的颜色由真实 WASM／GPUI 样式解析回归覆盖；不把未进行的桌面操作计入通过。

用户原 profile 的宿主进程及 Rust 0.6.2 安装未替换。新插件包需导入；通用 namespace 色板与[重复警告浮窗去重](stderr-warning-coalescing-2026-10-30.md)需运行重新构建的宿主。

证据在项目外 `C:/Users/Tmiracle/.codex/task-artifacts/rust-warning-20261030/`：原包失败、候选通过、namespace 失败／通过、静态准入报告，以及公开开发实例日志。返回[原生 UI 目录](../README.md)。
