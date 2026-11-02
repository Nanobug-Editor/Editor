# 文件加载时正文闪烁修复

状态：修复与针对性回归、Windows 原生绘制、workspace 测试及编译检查通过；扩展高亮集合存在一项既有夹具失败，详见下文。

## 复现与原因

用户录屏 `C:/Users/Tmiracle/Desktop/PixPin_2026-11-02_15-41-41.mp4` 为 60 FPS。
逐帧检查确认正文消失而行号仍保留，5 段空白分别位于
5.400–5.467、6.067–6.200、7.750–7.817、8.517–8.617、9.283–9.383 秒。

新文件首先绘制普通文本，随后安装语言高亮器。`outline::folding::Adapter`
在后台准备语法解析器；此时 `SyntaxHighlighter::deferred` 尚无源文本快照。
`styles` 原先把可见范围裁到旧快照长度，返回空列表或不完整的文本段。
Base 依赖这些段绘制正文，因此短暂清空正文，而行号继续独立绘制。

修复让样式段覆盖当前编辑器请求的完整范围，未解析部分使用默认前景。
解析完成后的捕获优先级、动态语法来源、后台任务与 revision 检查保持原有机制。

## 回归证据

新增普通测试覆盖首次加载、旧快照后新增文本、视口完全超出旧快照及空范围。
修改生产代码前，前两项失败，分别返回 `[]` 与 `0..4`，而实际请求是 `0..54` 与 `0..32`。

原生测试使用生产 `Controller::attach` 与本地 `source_editor::Editor`，通过隔离的
语法提供者工厂控制后台准备期间，检查真实 Windows/D3D 帧中的正文像素。
工厂最后返回不可用错误，检查普通文本回退；不安装或启动语言服务。
源文本包含中文、Emoji 和多行内容，行号与光标不计入正文像素。

| 版本及主题 | 准备前文字像素 | 等待解析的连续 8 帧 | 完成后文字像素 |
| --- | ---: | --- | ---: |
| 修复前，浅色 | 7336 | 全部为 0，断言失败 | 7336 |
| 修复后，浅色 | 7336 | 每帧均为 7336 | 7336 |
| 修复后，深色 | 8055 | 每帧均为 8055 | 8055 |

已查看导出的深浅主题画面；等待解析期间正文、中文和 Emoji 均可见。
此测试验证宿主绘制及语法不可用回退，不把它等同于真实 WASM 语法加载成功或系统 IME 候选窗验收。

## 检查命令

工作区：`C:/Users/Tmiracle/.codex/worktrees/editor-load-flicker/Editor`。
构建前已核对 Cargo metadata，所有产物使用该 worktree 自身的 `target/`。

| 命令 | 结果 |
| --- | --- |
| `cargo test -p editor-app --bin editor-app deferred_highlighter -- --include-ignored --nocapture --test-threads=1` | 4 passed，包含显式执行的原生测试 |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test --workspace --exclude editor-app -j 2` | 127 组，877 passed、0 failed、446 ignored；未运行的 ignored 不计为通过 |
| `cargo check --workspace -j 2` | 通过，保留仓库已有警告 |
| 同一 editor-app 测试程序，过滤 `highlight` | 21 passed、1 failed、6 ignored；失败为既有 Markdown 夹具校验，见下文 |
| 同一 editor-app 测试程序，过滤 `preview_tabs` | 13 passed |
| 同一 editor-app 测试程序，过滤 `rust_package_ --ignored` | 3 passed，真实 Rust 0.6.4 ZIP，使用外部 SHA256 账本 |
| 同一 editor-app 测试程序，过滤 `vue_editor_native_highlights_themes_keyboard_and_ime --ignored` | 1 passed，Vue 0.1.13、TypeScript 0.1.18；深浅主题、中文输入、Undo 与 IME 桥 |

普通高亮、文件切换及真实插件测试直接运行同一次 Cargo 生成的
`target/debug/deps/editor_app-f9e58d58fdbe3923.exe`，均带 `--nocapture --test-threads=1`。
真实包来自主项目 `dist/<id>/`，未改写包及用户安装。
Vue 的 IME 验证是 `EntityInputHandler` 标记／提交回调，不包含系统候选窗。

`highlight` 集合中的 Markdown 夹具失败于 `extensions/markdown_tests.rs:61` 的重新打包，
错误为 `Structure requires WASM, language.structure and editor.read`。
该夹具移除了 component、API 能力与权限，却保留了结构提供者；校验先于编辑器创建及高亮调用。
夹具与对应校验代码相对任务起点 `60412e95` 均未改变，此次不扩大范围修改它们。

workspace 首次默认并发构建遇到 Windows `os error 1455`（页面文件不足）及 LLVM
out-of-memory；保留失败日志，使用相同 worktree/target 加 `-j 2` 重试。

原始录屏分析、红绿测试日志及截图集中保存于项目外
`C:/Users/Tmiracle/.codex/task-data/editor-load-flicker-20261102/`。
原生截图可通过 `NANOBUG_LOAD_CAPTURE` 指定已有输出目录。
