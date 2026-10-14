# Markdown HTML 图片尺寸修复

日期：2026-10-14。状态：实现、83 项插件单元、匹配宿主/包构建与 3 项定向原生验收通过；未发布或安装到用户 profile。

## 复现和根因

用户截图将 README 中 128×128 的 HTML 图片显示为大图。独立插件最小测试 `html_image_dimensions_reach_native_layout` 连续两次出现 `None != Some(128)`。过滤器未传递 width/height。通用宿主像素测试又实际得到 128×64，而非声明的 128×128；像素比例修复后，说明仍受整个 128 高节点限制，测试再次失败。

业务修正在 `C:/Projects/RustProjects/Markdown-for-Nanobug/src/preview/embedded_html.rs`，将 HTML 整数像素尺寸传给已有公开 Layout 字段。宿主 `ui/plugin/leaf.rs` 按两个声明值决定显示比例、按单边和原图比例计算缺失边，并保留窄栏缩小与无尺寸时不放大的行为；`render.rs` 让图片说明独立占用高度，`virtual_blocks.rs` 的高度估计同步考虑说明。所有逻辑面向普通 Image 节点，不按插件 ID 分支。

`plugin-protocol/src/ui.rs` 补充现有公开图片尺寸语义的源码注释，未新增字段、权限或能力版本。视觉 viewport 图像仍由其整个绘制表面尺寸控制，不采用说明高度例外。

## 固定输入和制品

- 本地插件版本 0.18.1，协议 7，公开能力/权限保持原值。
- 匹配宿主：`target/debug/editor-app.exe`（Debug 开发程序，包含本机其他任务的已有改动，不将那些改动计为本次成果）。SHA-256：`522862a1a6a5d2f8d1163fae83529d8a51dcb9d91ff37ec4b82a7b35b894d480`。
- 实际宿主 SDK 摘要：`a7a66395ad257bb31cd68706568ebecd6c6d62359cca86b155e149607ee0186c`。本机匹配二进制构建来源存在未提交变更，账本中的源码/宿主 revision 使用 null；历史 host-sdk.json 不冒充本轮输入。
- 插件 ZIP：`C:/Projects/RustProjects/Markdown-for-Nanobug/dist/markdown-0.18.1.zip`，SHA-256：`4ae2aa098d2d9f5fca4db4a4e55b2dd19b7d49bb246161aaa630b75de1997934`；与[固定测试账本](../../../crates/plugin-runtime/tests/fixtures/packages/inputs.json)一致。
- 普通市场读取仍使用已审核 0.17.2；原生 Markdown 测试明确选择当前 0.18.1 候选，0.18.0 历史本地候选仍保留。

## 实际验证

- `target/debug/editor-app.exe --plugin-cargo ../Markdown-for-Nanobug/Cargo.toml test --lib`：83 passed，0 failed，0 ignored。
- `cargo build -p editor-app`、`cargo check --workspace`：通过。
- `target/debug/editor-app.exe --plugin-package ../Markdown-for-Nanobug --output ../Markdown-for-Nanobug/dist`：成功生成实际 WASM ZIP，未调用旧脚本或复制 SDK 源码。
- 因并发构建存在旧测试产物，首次 Cargo 过滤返回 0 tests，未计为通过。使用 `cargo rustc -p editor-app --tests --message-format=json -- -C debuginfo=0` 重编译，并从 compiler-artifact 的 profile.test/executable 核对实际测试二进制；执行前以 --list 核对三个指定场景确实存在。
- 在同一 deps 目录的二进制副本 `editor_app-markdown-size-acceptance.exe` 上，以 `--include-ignored --test-threads=1` 分别执行 `explicit_image_dimensions_size_pixels`、`delivered_markdown_html_image_dimensions_match_pixels`、`delivered_markdown_wide_image_height_follows`：各 1 passed，合计 3 passed，0 failed、0 ignored。覆盖非正方形原图、单边、放大/原始/零值、说明布局、实际 128×128 图片和图片链接，以及既有分栏缩放。
- `cargo test --workspace --exclude editor-app` 最初因 `optional_language_dependencies` 链接时无法打开 msvcrt.lib（LNK1104）失败；`cargo test --workspace --exclude editor-app -j 1` 重试通过。普通命令中的 ignored 仍未运行，不计为通过。
- 独立插件 fmt、改动宿主文件的定向 rustfmt 与两仓库 `git diff --check` 通过。完整 `cargo fmt --check` 仍报并发 `tests/plugin_preview_activation.rs` 的格式差异，未修改该文件。

日志位于宿主 target：`markdown-html-size-red.log`、`markdown-html-size-host-real-red.log`、`markdown-html-size-caption-red.log`、`markdown-html-size-host-green.log`、`markdown-html-size-all-plugin.log`、`markdown-html-size-package.log`、`markdown-html-size-native-final.log`、`markdown-html-size-workspace-retry.log`、`markdown-html-size-workspace-check.log`。

## 交付边界

图片 width/height 取值为 0～10000 的整数像素；无效值逐项忽略，CSS/百分比仍不在本次范围。必须使用包含本次通用图片几何修复的宿主，不能仅凭新 ZIP 让旧宿主获得完整单边/双边尺寸语义。

未关闭用户当前运行程序、替换其安装、Git 提交/推送、发布 Release 或修改市场。用户使用新 Debug 宿主前应退出旧宿主，再导入 0.18.1 包。Windows 原生自动化为本次行为证据，未执行人工 OS IME 候选窗口或跨平台验证。
