# Git 提交列表文字对齐

日期：2026-10-23。状态：功能、协议与控件回归、173 项插件回归、Release 构建及两组合实际包原生验收完成，已交付本地宿主与 ZIP；未安装到用户配置、提交或发布。全仓检查存在其他会话的失败，见下文。

## 本轮要求与依据

用户截图明确标注分支标签文字居右、时间文字居左；保留此前确认的提交者居中加粗、可拖动列宽、透明边界及时间格式。此前[列宽记录](git-history-columns-2026-10-22.md)描述了这些规则，但原生回归只检查坐标变化、粗体、时间格式及导航保持，未核对真实字体的字形右边缘与中心。

本轮按本机记忆约定先检索“Git 提交列表 分支标签居右 时间居左 提交者居中 列布局”，再补检“Git提交列表分支标签靠右、时间靠左的列对齐”。两次 `status=ok`，但只覆盖早期功能范围；采用的历史要点仅是四列顺序。相关 `memory_id=f1309303-977b-450c-9edf-23be216be557`，`document_id=conversation:01a13d8a-0c06-7782-9823-0efb47ad5d2e`，提及时间 2026-10-15。本次对齐规则以当前截图、已确认需求和源码为准。

## 反馈循环

证据目录：`C:/Projects/RustProjects/Editor/target/git-history-alignment-2026-10-23/`。修改前插件 0.1.33 及原生回归文件保存在 `before/`，保留其他会话的提交复选框、标签、顶栏和语言上下文改动。

- 在既有实际包原生提交列表流程中，拖动三条边界后，用同一个 GPUI TextSystem、字体、字号和粗细计算文字实际宽度，检查标签右边缘、提交者中心和时间左边缘；允许误差 1 个逻辑像素。
- 首次编译在其他会话正在修改的 `app/shell.rs` 和 `editor/tabs/view.rs` 失败。相关文件更新后重试原入口；这类编译失败不计作文字对齐缺陷的红灯。
- 重试在共享 Cargo 队列后编译通过。`native-red/` 使用未修复的 0.1.33 实际包稳定失败：标签右侧误差 `-4.375px`、提交者中心误差 `-2.9205017px`、时间左侧误差 `0px`。这才是对齐缺陷的行为红灯。

## 原因与实现

插件原来按 ASCII 7px、粗体 7.3px、其他字符 14px 估算字符串宽度，再计算绘制起点；真实绘制由原生字体整形完成，两种宽度不一致。拖列后的时间起点准确，实际原生字体度量复现标签与作者误差，确认了估算宽度参与对齐的问题。旧验证只检查绘图坐标，没有覆盖字形右边缘和中心。

- 公开 `Paint::TextBox` 提供矩形、水平对齐、字体、字号、粗细与颜色。`ui.canvas 1.4` 在能力协商与场景需求中显式声明；未按插件 ID 分支，不新增权限或改变协议 7。旧点定位 `Paint::Text` 语义保留。
- 通用本地绘制位于 `crates/editor-ui/src/text/canvas.rs`。GPUI 原生字形度量同时决定实际起点、超宽省略和裁剪；上下文整形的修正次数有界，UTF-8 边界安全。宿主适配位于 `crates/editor-app/src/ui/plugin/canvas/text.rs`，只投影公开矩形／字体，不解释 Git 数据。
- Git 的三项元数据改用文本区域，标签 End、作者 Center、时间 Start；主标题继续原来的绘制路径。既有保守截断只限制传输文本，不再参与对齐起点。列宽、透明边界、日期格式、颜色设置、复选框修复及提交表单保持。
- 独立插件为 0.1.34，需要匹配 1.4 能力的宿主。用户当前运行 `target/release/editor-app.exe`，本轮通过直接 Cargo Release 入口输出独立 `Nanobug.exe`；没有覆盖或停止正在运行的程序。临时输出位置先以无副作用的小 Cargo 工程验证，证据在 `output-path-probe/result.json`。

## 验证结果与交付

- `protocol-text-tests.log`：2 项通过，覆盖公开 JSON 往返、能力下限、非法矩形／字号／字体、多行文本和总量预算。
- `native-text-tests.log`：1 项通过，实际 GPUI 文本系统覆盖比例字体、粗体、中文、组合字符、Emoji、三种对齐及窄区域省略／裁剪。
- 首轮插件回归 172 项通过、1 项旧断言失败：该断言只把 `Paint::Text` 计为单元格，未计新文本区域。修正为检查四列及互不重叠的区域后，`guest-tests-final.log` 中 173 项全部通过；未放宽滚动、图形留白或内容检查。
- `host-release-results.json`、`guest-results-final.json`：Release 构建、通过宿主公开 `--plugin-cargo` 的插件测试与 `--plugin-package` 打包均返回 0。候选位于 `packages/git-0.1.34.zip`。
- 原生驱动使用共享 checkout 中已经包含本轮通用文本适配及边缘断言的测试程序，身份见 `native-green-identity.json`；另行针对性 `--no-run` 编译核验通过，见 `native-green-build-results.json`。`native-green-results.json` 返回 0：深色中文 100%（99.45 秒）和浅色英文 150%（99.72 秒）全部通过，三项原生对齐误差均为零，列宽手势、滚轮、拖条、提交区隔离及关闭清理均通过。
- 已核对两组 D3D11 截图。滚轮 32 次共 160px、边界反向后 25px、原矢量图片复用和选中保持均通过；132ms／100ms 只是这一批回包时间，不作为屏幕帧率结论。
- `workspace-results.json`：`cargo check --workspace` 通过；`cargo test --workspace --exclude editor-app` 在 `language_declarative.rs:239` 停止，实际 `language.lsp=1.8.0` 而旧断言为 `1.7.0`。本次只把 `ui.canvas` 从 1.3 提升至 1.4，语言版本升级是其他会话的既有改动，未修改该断言。停止前 487 passed、1 failed、253 ignored；后续测试未运行，ignored 不计作通过。
- 全仓 `cargo fmt --check` 失败，差异位于其他会话的语言准备／角色及 Maven 测试文件，具体见 `fmt.log`。本次修改的文件独立检查，未代其他会话格式化；`git diff --check` 通过。
- 插件交付：`C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.34.zip`，978881 字节，SHA-256 `dee89f55461147b29e3404bab63365561489f186ff712c7b6afc74268b599e47`。与两组原生验收候选完全一致，十六项颜色设置、协议 7 和安装权限已核对。
- 配套 Release 宿主：`C:/Projects/RustProjects/Editor/target/git-history-alignment-2026-10-23/Nanobug.exe`，SHA-256 `6d56d625257e2c0858c0f3ef784ca14596e09c2142c104a3b468989ab36dd267`。关闭原宿主后使用此程序，再通过插件管理器安装 0.1.34；旧 1.3 宿主会按能力声明拒绝新包。程序来自本轮共享 checkout，不把其他会话代码的验证状态算作本次成果。
- 汇总身份、逐组断言及实际计数见 `final-evidence.json`。所有失败日志与截图保留，不把旧包红灯或共享工作区异常改记为通过。

```powershell
$ev = 'C:/Projects/RustProjects/Editor/target/git-history-alignment-2026-10-23'
cargo rustc -p editor-app --release --jobs 2 -- -o "$ev/Nanobug.exe"
& "$ev/Nanobug.exe" --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib
& "$ev/Nanobug.exe" --plugin-package C:/Projects/RustProjects/Git-for-Nanobug --output "$ev/packages"
cargo test -p plugin-protocol text_box --lib --jobs 2
cargo test -p editor-ui canvas_text_regions --lib --jobs 2
cargo fmt --check
cargo test --workspace --exclude editor-app --jobs 2
cargo check --workspace --jobs 2
```

```powershell
cargo test -p editor-app git_native_history_scroll_probe --no-run --jobs 2
```

原生流程使用实际 Git ZIP、临时仓库与配置，设置 `NANOBUG_GIT_NATIVE_HISTORY_ONLY=1`、`NANOBUG_GIT_NATIVE_SCROLL_ONLY=1`，显式运行 `extensions::worker::native_acceptance::git_native_history_scroll_probe --exact --ignored --nocapture --test-threads=1`。
