# Markdown 滚动与通用插件 UI 性能验收

状态：修复、专项行为验收及可运行宿主构建完成，改动保留供审阅；尚未提交、合并或安装到日常配置。

## 范围与基线

- 宿主基线：`60412e95cc339b42d3289d4636d0b4e7a4be8cdc`，任务分支 `codex/markdown-scroll-fix`。
- 用户要求：Markdown 加载卡顿、双向滚动失效、过期场景错误与滚动信息刷屏，以及长文档导致整个窗口拖动卡顿；通用 UI 优化适用于所有插件。
- 实际包回归使用已有的 Markdown `0.18.7` ZIP，并通过 `NANOBUG_MARKDOWN_SDK_SHA256` 校验摘要。worker 的固定包回归沿用仓库夹具。
- 长文件取仓库实际 `AGENTS.md`，本次基线为 17,075 字节。通用绘制压力场景使用协议配额内的 400 个映射富文本段落，并让根 Scroll 填满可用高度。
- 宿主构建使用本任务 worktree 自己的 `target/`；深路径夹具的 Cargo 构建显式使用同一 worktree 根下的 `target/`，未使用主项目编译输出。日志与截图保存在项目外的 `C:/Users/Tmiracle/.codex/task-artifacts/markdown-scroll-fix-20261102/`。

## 已定位的问题与改动

1. worker 只将部分原生回调识别为可正常过期的输入，遗漏了原生工具栏事件；源滚动虽被安全拒绝，却逐条写入 `host.ui.stale`。真实 worker 回归在修复前产生错误状态及 32 条滚动信息。现在对过期工具栏、源滚动和预览滚动回调静默丢弃，仍保留 revision、权限和输入有效性校验；其他错误继续报告。
2. 每个普通叶节点都完整查找编辑器操作区。绘制现在先查询当前场景已有的操作区节点集合，只对命中的节点继续核对活动页、模态状态和文档来源。集合在更新保留的树控件手势前同步，不作为授权凭据。
3. 旧虚拟化仍为每个屏外段落创建空文本控件和测量回调。现在连续屏外块共用一个空布局占位，累计原高度、外边距和内部间距；显式定位目标提前实体化，交互节点继续保留原生焦点遍历。未变化的高度缓存无需复制整个节点再比较。
4. 原 dev/test 未优化第三方依赖，使 WASM 编译和 GPUI 布局显著变慢。根清单对非 workspace 依赖设置 `opt-level = 2`，保留项目自身的未优化代码与调试信息。没有升级依赖或更改 release 配置；首次编译这些依赖会更久，本机首次测试构建约 17 分 42 秒。

## 性能结果

以下均为本机开发／测试构建实测，不作为 release 前后对比或所有硬件的帧率保证。

| 场景 | 对照 | 最终结果 |
| --- | --- | --- |
| 17,075 字节 AGENTS.md 首次打开 | 未优化依赖约 41.7 秒，其中 WASM 编译约 40.2 秒 | 3.17 秒；主组件编译约 2.78 秒 |
| 400 段通用 UI 的重绘中位数 | 已复用索引、未合并占位约 46.2 ms；合并占位后约 16.6 ms | 优化依赖后约 3.5 ms |
| 完整 Markdown 编辑窗口重绘中位数 | 单独观测整个窗口绘制 | 2.02 ms |
| Windows/D3D 真实拖窗 | 合并占位、未优化依赖约 52.5 FPS | 浅色 120.95 FPS、深色 119.63 FPS，显示器报告 121 Hz |

拖窗采样真实前台窗口移动时的绘制帧，不将输入定时器当作 FPS；通过显示刷新率 90% 的验收线。深色首次在并发 Rust 编译期间测得约 95 FPS，编译结束后复测通过。最初无高度约束的 400 段压力场景不纳入上表比较。

关键证据：`final-markdown-long_document.log`、`optimized-ui-performance.log`、`native-drag-final-light.log`、`native-drag-final-dark.log` 及同名 PNG；深浅截图均已检查。

## 验证结果与复现入口

- `cargo fmt --check`：通过。
- `cargo test --workspace --exclude editor-app -j2`：877 项通过，446 项按原定义 ignored，未计作通过。
- `cargo check --workspace -j2`：通过。
- `cargo build -p editor-app --bin editor-app -j1`：通过，首次普通构建约 11 分 56 秒；产物为本 worktree 的 `target/debug/editor-app.exe`，已通过 CLI 启动与固定包准入检查。
- `cargo test -p editor-app --bin editor-app --no-run -j1 --config 'profile.test.package.editor-app.debug=1'`：通过；`RUST_MIN_STACK=16777216`。
- 上述实际测试可执行文件的 `ui::plugin:: --test-threads=1`：96 项通过，23 项默认 ignored。
- 显式执行实际 WASM／原生测试：`extensions::markdown_tests::long_document` 1 项、`synchronized_scroll` 6 项、`small_scroll` 3 项、`first_open` 2 项均通过。覆盖源码与预览双向滚动、末次手势归属、连续滚动期间重排、工具栏开关、首屏和小幅表格滚动。
- `stale_source_viewport_is_discarded_without_plugin_error --ignored`：真实 worker 中旧工具栏事件及 96 个旧源视口、预览视口和旧 epoch 回调不产生错误状态或普通消息；非法零 origin 仍报告错误。
- 6 项操作区按压归属测试显式执行通过，保留场景替换、遮挡、来源释放的凭据校验。
- `long_plugin_window_drag_tracks_display_refresh --ignored --nocapture --test-threads=1`：浅色、`NANOBUG_DRAG_DARK=1` 深色各通过；测试结束恢复鼠标位置并退出隔离进程。

实际包回归通过以下环境选择已有产物，测试会核对 SHA-256；测试二进制始终在对应 worktree 的 `target/debug/deps/` 中运行：

```powershell
$env:NANOBUG_MARKDOWN_SDK_PACKAGE = 'C:/Projects/RustProjects/Editor/dist/markdown/markdown-0.18.7.zip'
$env:NANOBUG_MARKDOWN_SDK_SHA256 = 'df534bf48812b7abdd49c926e9a70d56241e9c9e378c09168da7c6516e276329'
cargo test -p editor-app --bin editor-app --config 'profile.test.package.editor-app.debug=1' extensions::markdown_tests::long_document -- --ignored --nocapture --test-threads=1
```

## 固定输入修复

独立视口用例原先使用的 `capability-example` 和 `0.19.1-main-sdk` 均只能解码旧 `native_editor`，无法读取当前 `editor` 节点。使用基线宿主的公开内嵌 SDK 重建测试专用 `0.19.2`，同时更新清单、Cargo 版本和锁文件；生产插件未升级。

先以公开 `--plugin-cargo ... build --release --target wasm32-wasip2 --target-dir <本 worktree>/target` 编译，再将编译结果与声明资源暂存于 `target/capability-example-package/`，通过 `--plugin-package ... --output C:/Projects/RustProjects/Editor/dist/capability-example` 生成 ZIP。静态 Package 准入及实际 Manager／原生视口测试通过后，新增固定 ZIP 和摘要账本；该用例显式选取 `0.19.2`，并移除与最小权限场景无关的命令贡献。旧 ZIP、旧摘要和其他用例的裸 ID 选择不变。

SDK 摘要：`d5128a04bb5b83c61feffc3a9b4ec2fc1d738ea6822cebad147616062ad45f71`；源树和归档摘要见 `crates/plugin-runtime/tests/fixtures/packages/inputs.json`。证据为 `example-sdk-build.log`、`example-package-check.json`、`independent-viewport-rebuilt.log` 和最终完整 `final-markdown-synchronized_scroll.log`。

## 构建注意事项

- 默认完整 `cargo test -p editor-app --no-run` 被既有 `svg_preview_render` 示例编译问题阻断；本任务未修改该示例。UI 测试使用 `--bin editor-app` 明确目标。
- 此机器编译大型 app 测试时出现过编译器资源崩溃；测试命令临时使用 `--config 'profile.test.package.editor-app.debug=1'` 减少调试信息，并限定并行度。它不改变测试行为或依赖版本。
- 深路径夹具默认 `target/` 曾触发 MSVC `LNK1181`；改用所属 worktree 根下的短 `target/` 后，重复同一 SDK 编译入口通过。
- 本次没有完成全量 app ignored 测试或 release 性能验收；上述未执行项均不计作通过。Markdown 独立 worktree 保持干净，修复集中在通用宿主。
