# Git 提交列表列宽与排版

日期：2026-10-22，2026-10-23 整理交付。状态：实现、最终候选包验证与原生交互验收完成；未安装到用户配置、提交或发布。

## 行为与边界

- 四个文字列仍为标题、分支标签、提交者、时间。标题占剩余空间，后三列默认宽度为 220/104/154 逻辑像素；标签靠右、提交者居中加粗、时间靠左，窄窗口按下限压缩。
- 根据用户追加要求，三条列边界透明，不绘制竖线；边界两侧各 5px 仍可发起左键拖动，只调整相邻列，限制过度拖动并允许反向恢复。松开、失焦或视口尺寸改变结束当前手势。
- 列表时间从 Git 的原始日期时间字段取 `YYYY-MM-DD HH:mm:ss`，不进行时区换算；无法识别的输入保留原文。列宽不足时沿用省略显示，用户可拖宽。
- 列宽在分支切换、滚动及插件热更新快照中保留；不承诺跨应用重启持久化。拖动不改变提交选择、滚动 offset 或内容 epoch，不启动 Git 查询，不刷新提交表单。
- 继续采用有界预绘和原生帧滚动。拖动复用公开 `CanvasEvent::Pointer` 捕获，生产宿主、协议、权限均未改变；宿主改动仅为现有原生验收增加本次交互检查。

## 验证

证据目录：`C:/Projects/RustProjects/Editor/target/git-history-columns-2026-10-22/`。

- `columns-tests.log`：4 项针对性测试通过，覆盖三条列边界、选中与滚动保持、过拖反向与失焦、窄窗口、字体／时间排版及快照数据边界。
- `guest-tests-transparent.log`：最终透明边界版本的 159 项插件测试全部通过，其中包含“不绘制分隔线”的断言。完整回归期间保留了原标题起点，确保无变化尺寸事件仍不重新发布界面。
- `large-history.log`：真实 8193 条历史、65 页加载、128 次滚动的有界绘图回归通过。随后仅移除分隔线绘制，命中几何与滚动处理没有改变。
- `check-results.json`：格式检查、workspace 非 `editor-app` 测试及 workspace 编译检查全部通过；测试合计 516 passed、382 ignored。ignored 不计作通过，原生用例另行显式执行。
- `native-transparent.log`、`native-transparent-results.json`：最终透明边界包的原生验收通过，覆盖深色中文 100%／浅色英文 150% 两种组合。实际鼠标拖动全部三条不可见边界，确认文字布局随列宽变化、提交选择与滚动位置保留、提交表单场景不刷新、提交者加粗、时间格式正确，并继续验证滚轮和滚动条交互。两组均正常退出。
- `native-transparent/<case>/evidence.json` 的 `observations.history_columns` 七项结果均为 `true`，`candidate_sha256` 与最终交付包一致。两组 `history-columns.png` 已人工检查，列间无竖线；窄窗口中内容按列宽省略。未把未覆盖的系统输入法候选窗口计作通过。

```powershell
$nanobug = 'C:/Projects/RustProjects/Editor/target/git-panel-isolation-2026-10-22/Nanobug.exe'
& $nanobug --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib
& $nanobug --plugin-package C:/Projects/RustProjects/Git-for-Nanobug --output target/git-history-columns-2026-10-22/packages
cargo fmt --manifest-path C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml --check
cargo test -p editor-app git_native_history_scroll_probe --no-run --jobs 2
cargo fmt --check
cargo test --workspace --exclude editor-app --jobs 2
cargo check --workspace --jobs 2
```

实际包测试设置 `NANOBUG_GIT_CANDIDATE`，显式运行 `git_package_large_history_keeps_scroll_scenes_bounded --ignored --nocapture --test-threads=1`。原生验收设置同一候选及 `NANOBUG_GIT_NATIVE_OUTPUT`、`NANOBUG_GIT_NATIVE_HISTORY_ONLY=1`、`NANOBUG_GIT_NATIVE_SCROLL_ONLY=1`，在编译后的 `editor-app` 测试二进制显式运行 `extensions::worker::native_acceptance::git_native_history_scroll_probe --exact --ignored --nocapture --test-threads=1`。所有操作使用临时配置和 Git 仓库。

交付包为 `C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.30.zip`，证据目录的 `packages/git-0.1.30.zip` 为同一产物，SHA-256 为 `cdbc4dcadbd48304e5ddf69ba78f14d18f7eef589770182ff0d7ac3d31bbc45f`。沿用 0.1.29 已要求的 `ui.canvas >=1.3` 宿主，无需因本次排版更新生产宿主。工作区同时存在其他任务改动，不归入本次成果。
