# Markdown 0.18.0 内嵌 HTML 原生预览

日期：2026-10-14。状态：插件实现、单元测试、打包与定向原生验收通过；本地候选尚未发布。

## 范围与依据

用户要求“Markdown 插件允许渲染 HTML 元素”，替代独立插件 `docs/spec.md` 中首版排除内嵌 HTML 的条款。业务源码位于 `C:/Projects/RustProjects/Markdown-for-Nanobug`；宿主只新增真实包回归与固定测试输入，未增加 Markdown 专属 API 或渲染分支。

HTML5 tokenizer 将作者 HTML 转换为只读原生富文本及受控图片节点。支持标题、段落、行内格式、换行、表格、图片与链接；过滤脚本、CSS、事件属性、嵌入页面与交互表单。图片与链接仍走既有授权、路径检查、revision 和资源清理入口。代码中的 HTML 保持字面内容；HTML 图片与片段共享其包含块的 UTF-8 源码范围，链接只授予实际包含链接的叶节点。

原生渲染支持的 HTML 子集不等同于浏览器布局；不支持 HTML `id` 锚点、CSS 尺寸或交互表单，Markdown 标题锚点继续可用。默认市场夹具仍是已审核的 0.17.2，新候选按 `markdown-0.18.0` 明确选择，旧 ZIP 和市场记录不变。

## 实际输入与制品

- 插件版本：Cargo、manifest.json 和 plugin.toml 均为 0.18.0；protocol 7，公开能力与权限不变。
- 构建入口：本项目现有 `target/debug/editor-app.exe --plugin-cargo` 与 `--plugin-package`，未使用旧脚本或复制 SDK。
- 实际 SDK 摘要：`b02e41e9b1a7f92e8e745885df2388baab68cb28f487a009d3505b363b6d730f`。插件 `host-sdk.json` 保留其历史固定宿主来源；本次验证使用当前本机公开宿主，未宣称运行了历史固定二进制。源码与宿主的未提交状态在账本中如实使用 null，不推断宿主二进制的 Git commit。
- 生成 ZIP：`C:/Projects/RustProjects/Markdown-for-Nanobug/dist/markdown-0.18.0.zip`。
- 完整 SHA-256：`274877f66055ea95751714f9f758ef5e51bc5bfef87adec634e215e48e6db33b`，与宿主[冻结输入账本](../../../crates/plugin-runtime/tests/fixtures/packages/inputs.json)对应候选逐字节一致。
- 包含组件、grammar、查询、图标、README 与许可证；无 src、Cargo 或 target 缓存入口。

## 验证

- `target/debug/editor-app.exe --plugin-cargo ../Markdown-for-Nanobug/Cargo.toml test --lib`：81 passed，0 failed，0 ignored；覆盖 HTML 子集、中文实体和属性、脚本/嵌入/表单过滤、代码字面量、混合图片、精确链接归属、局部重解析及深度限制。
- `target/debug/editor-app.exe --plugin-package ../Markdown-for-Nanobug --output ../Markdown-for-Nanobug/dist`：成功生成实际 WASM ZIP；补充过滤交互表单后再次生成并固定上述最终摘要。
- `cargo test -p editor-app embedded_html -- --ignored --test-threads=1`：2 passed，0 failed；通过真实 Manager 安装、原生布局与像素加载、深浅主题、未保存输入及真实指针点击打开文档。最终候选又通过下列定向测试，不重复累加相同场景。
- `cargo test --workspace --exclude editor-app` 与 `cargo check --workspace`：均通过；ignored 项未在该命令执行，不计为通过。
- 独立插件 `cargo fmt --manifest-path ../Markdown-for-Nanobug/Cargo.toml --check`、两仓库 `git diff --check`：通过。主 workspace 格式检查最初通过，随后其他任务新增未格式化文件，未修改这些并发内容。
- UI 测试重编译一度因其他任务拆分中的 `run_ui_tests/toolbar` 未就绪而失败（E0583），后又遇到其 `plugin_preview_activation.rs` 缺少 `ClipboardItem` 导入（E0433）。这些并发问题随后恢复；最终 `cargo test -p editor-app embedded_html --no-run` 和 `cargo check --workspace` 均通过。
- 最终 ZIP 在同一 deps 目录的已编译测试二进制副本上执行，避免锁住其他任务的链接输出：`editor_app-markdown-html-acceptance.exe <filter> --include-ignored --test-threads=1`。过滤词为 `embedded_html`（2 passed）、`delivered_markdown_preview_tracks_unsaved_native_edits_and_reclaims_split`（1 passed）与 `delivered_markdown_relative_link_opens_native_document`（1 passed），合计 4 项定向原生验收，0 failed、0 ignored。除 HTML 场景外，确认普通 Markdown 的任务框、代码块、未保存输入、Undo/Redo、关闭清理和文档导航保持正常。
- 可选的完整 90 项 Markdown 组曾启动，3 项代码高亮回归打印 ok 后改为上述定向验收；完整组未运行结束，不将其报告为通过。

日志在本机宿主 target 中：`markdown-html-plugin-tests.log`、`markdown-html-package.log`、`markdown-html-native-tests.log`、`markdown-html-targeted-final.log`、`markdown-html-all-native-final.log`（中止的完整组）、`markdown-html-workspace-tests.log`、`markdown-html-workspace-check-final.log`、`markdown-html-native-build-latest.log`、`markdown-html-fmt-latest.log`。本次三个 Rust 测试文件另经 `rustfmt --edition 2024 --check --config skip_children=true` 定向检查通过；最新全 workspace 格式检查仅报并发 `plugin_preview_activation.rs` 的格式差异。

## 交付限制

未提交、推送、发布 Release、提交市场审核或替换用户已安装插件。当前源码及生成 ZIP 可供本地导入。未执行人工 OS IME 候选窗口或 macOS/Linux 验收；工作区其他任务的改动不计入本次成果。
