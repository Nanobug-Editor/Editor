# Hover 内容层级与统一外观

日期：2026-10-24。接续[拖动交互修复](editor-hover-gestures-2026-10-24.md)，调整定义说明和诊断说明的外观与内容排版。

## 外观和内容

两类说明统一使用 `editor-ui::definition_popup::details_card`（绘制位于该组件目录的 `surface.rs`）：主题浮层背景、主题大圆角、降低不透明度的中性边框、两层柔和阴影，以及按圆角裁剪的内容区域。诊断严重级别不再染红整个外框，而用标题行的彩色圆点、级别文字和淡色错误码背景保留提示。颜色来自现有 Error／Warning／Info／Hint 主题角色。

定义说明按语言服务提供的内容顺序显示模块路径、签名、文档、布局信息等。普通说明使用界面字体，代码块使用编辑器等宽字体；紧凑段落、同字号粗体标题、行内代码和分隔线区分层级。代码块通过 Base 的公开样式和 actions 接口去掉额外边框及语言角标。现有定义源码补充预览继续显示。没有自行推导类型大小、补写文档或新增宿主语言白名单。

诊断说明依次显示：

1. 级别标记、级别名称及可选错误码。
2. 完整消息正文和语言服务提供的相关说明；纯文本中的换行保留，不把特殊字符解释成 Markdown 语法。
3. 使用较小、较淡文字的底部来源及行列位置。

普通 Hover 的 `MarkupKind::PlainText` 同样按纯文本转义，包括追加现有源码预览之前的内容组合路径。Markdown 内容保留语言服务原有的代码块、标题、列表和分隔线。

参考 Zed 官方 [Hover 实现](https://github.com/zed-industries/zed/blob/main/crates/editor/src/hover_popover.rs) 中的 UI 正文字体、代码字体、紧凑 Markdown 和卡片 elevation 分层。实现留在现有本地 UI 层，不引入 `gpui-component` 或内建语言 grammar。

## 验证

沿用此前的真实分隔条、延迟 Hover、滚动、键盘、复制和缩放回归。Windows/D3D 夹具改用包括代码签名、段落、分隔线和中文的类型说明，以及带来源、错误码和帮助行的 Error 诊断。

```powershell
cargo test -p editor-app hover_ -- --nocapture
$env:NANOBUG_HOVER_OUTPUT = "$PWD/target/hover-appearance-native"
cargo test -p editor-app hover_native_windows_acceptance -- --ignored --nocapture --test-threads=1
Remove-Item Env:NANOBUG_HOVER_OUTPUT
cargo fmt --check
cargo test --workspace --exclude editor-app -j 1
cargo check --workspace -j 1
git diff --check
```

## 换行高度修正

首次原生图片暴露了长正文换行后首屏过早裁切。新增结构化文档用例后，针对性测试在旧测量方式上失败：卡片为 `500 × 190.5`，内部正文为 `477 × 188.5`，加上内边距后无法放入卡片；该用例执行约 0.32 秒。记录在 `target/hover-appearance-wrapping.log`。

`DefinitionPopup` 现在先测首选宽度，再使用该确定宽度重新计算高度；保留原有高度上限和用户拖动指定尺寸。修复后同一断言以及其他 Hover 回归全部通过。最终图片中中文首屏完整，长内容仍可滚动。

## 实际结果

| 检查 | 结果 | 日志（`target/`） |
| --- | --- | --- |
| Hover 回归 | 15 passed、0 failed、2 ignored；包含新增长文档的首屏高度断言，以及拖动、键盘、复制、缩放与主题对比度 | `hover-appearance-tests.log` |
| Windows/D3D 原生验收 | 父入口 1 passed，子进程 1 passed；深浅主题、类型说明及 Error 诊断，分隔条实际位移后等待仍无弹框 | `hover-appearance-native.log`、`hover-appearance-native/native.log` |
| `cargo fmt --check` | 通过 | `hover-appearance-fmt.log` |
| workspace 测试（不含 editor-app） | 555 passed、0 failed、402 ignored；110 组含空 doctest | `hover-appearance-workspace-tests.log` |
| `cargo check --workspace -j 1` | 通过；保留已有警告 | `hover-appearance-check.log` |
| `git diff --check` | 通过 | `hover-appearance-diff-check.log` |

最终原生执行使用本次成功 Cargo 测试后立即复制的 `target/hover-appearance-tests/editor-app-tests.exe`，显式运行 `editor::hover_hit_test::native::hover_native_windows_acceptance --ignored --nocapture --test-threads=1`。两个默认跳过的原生入口已单独执行，不计入普通 15 项通过数。图片为 `target/hover-appearance-native/{light,dark}-{definition,diagnostic}{,-drag}.png`，已查看本轮图片中的圆角、阴影、首屏中文、换行正文、来源页脚及拖动后状态。

验证期间，早期两次直接调用共享 `target/debug/deps/` 中的测试程序出现 `native definition did not paint`，当时尚未保存断言前截图。后续重新编译并固定测试二进制后，原生验收连续通过；未证实早期失效原因。此后截图发现的高度问题有独立失败断言和修复后通过结果。完整 workspace 检查曾为定向反馈主动暂停一次，上表为恢复后完成的结果。

## 可运行产物

Release 源码编译和链接完成，但 Cargo 最后复制到 `target/release/editor-app.exe` 时因运行中的程序占用而返回拒绝访问（退出码 101）。日志为 `target/hover-appearance-release-build.log`；该命令不计为完整成功。

本次链接产物 `target/release/deps/editor_app.exe` 已复制为 `target/hover-appearance-release/editor-app.exe`，两份 SHA-256 校验一致：

```text
58F8937C75B21818D05E10C3B4089E28C3F84D6CA397AFBDEACD314C9EE1516B
```

文件大小为 86,709,760 字节，产物时间为 2026-10-24 18:37:57（Asia/Shanghai）。保存工作并关闭旧程序后，可以运行这份新版。原生外观与交互验证使用上文固定的 Debug 测试程序，没有把 Release 文件校验当作运行验收。

未提交或推送。记录和截图只计本次执行，不复用此前外观的截图作为通过依据。

返回[原生 UI 文档目录](../README.md)。
