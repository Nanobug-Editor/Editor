# Git 比较：无差异区段折叠／展开

日期：2026-10-20。状态：本次功能与配套候选验收完成；未改动用户已安装插件、未自动重启、提交、推送或发布。实际包测试仅使用隔离的临时 Manager 配置。

## 范围与机制

- 用户要求比较的无差异部分可以折叠／展开，并明确允许将资源管理器的折叠／展开 SVG 复制到独立 Git 插件内。新增资源位于 `C:/Projects/RustProjects/Git-for-Nanobug/icons/explorer-collapse-all.svg` 与 `explorer-expand-all.svg`，通过包内 artwork 消费。
- Git 插件升至 0.1.19，使用面向所有插件的 `editor.diff 1.4` 可选 `folding` 描述。运行时复用 owned SVG 的规范路径、`ui.icons`、`assets.read`、64 KiB 单文件限制与 geometry-only SVG 校验，最多四个不同主题资源。不新增安装权限，不按 Git ID 分支。
- 比较默认展开；工具栏支持全部折叠／展开，折叠占位行提供单段展开入口。双／三源仅隐藏所有来源字节完全相同的连续行，区段两端各保留三行上下文，至少两行才可替换为一行占位。CRLF／LF 差异、增删改和任意父版本中的独有变化保持可见。
- 本地 `ui/controls/aligned_editor` 组合原有原生编辑器与零字节显示投影，隐藏行没有可点击的文本坐标。保留真实文本、原行号、选择、IME 与 Undo；未复制或 fork 上游工程。单段按钮 occlude 底层输入命中区域，避免根指针路由将其转换为编辑器手势。
- 每个保留标签持有自己的折叠选择。内容更新后若公共区段不变则保留单段选择，否则按最近一次全部折叠／展开动作重建；导航或真实光标移动揭示隐藏目标。原生搜索／代码操作打开时暂用完整原生来源，关闭后恢复折叠投影；既有预算回退规则保留。

## 现有工作区边界

在 `main` 的既有脏工作区继续本次需求；前轮箭头、整行底色、内侧行号槽、默认选择和树保留实现不属于本次新增成果。用户已有 `docs/窗口布局拖放规则.excalidraw` 改动保留。前轮记录中的旧比较辅助功能命名失败已有前后版本对照，本轮不将其记作已通过。

## 验证与候选身份

日志均保存在 `C:/Projects/RustProjects/Editor/target/`；独立访客构建日志位于 `C:/Projects/RustProjects/Git-for-Nanobug/target/`。

- 首轮编译发现本次 YAML 插入打断原有翻译项及缺少宏导入；修复后 `cargo check --workspace --jobs 2` 通过。失败与复验保留在 `unchanged-folds-check-first-2026-10-20.log`、`unchanged-folds-check-second-2026-10-20.log`。
- 首轮原生回归 2 通过／1 失败：单段按钮未隔离底层指针命中。增加占位区域 occlusion 后，同一行为复验 3/3 通过，覆盖两／三源、两主题、源文本／选择／事件及同步揭示；日志 `unchanged-folds-native-first-2026-10-20.log` 与 `unchanged-folds-native-second-2026-10-20.log`。
- 相关本地 UI／协议 SDK 回归 113 通过、23 ignored；`unchanged-folds-ui-2026-10-20.log`。原生输入／对齐 ignored 回归显式执行 20/20，通过；`unchanged-folds-native-input-2026-10-20.log`。ignored 未执行项不计为通过。
- `cargo fmt --check` 与 `git diff --check` 通过。最终门禁、独立插件测试、实际包流程与交付身份列于下方。

### 最终门禁与原生验收

| 实际入口 | 结果与范围 | 日志 |
| --- | --- | --- |
| `cargo fmt --check`、独立访客 `cargo fmt --check --manifest-path C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml` | 通过 | `unchanged-folds-fmt-delivery-2026-10-20.log`；访客检查直接返回 0 |
| `git diff --check` | 通过，既有换行符提示不属于空白错误 | `unchanged-folds-diffcheck-delivery-2026-10-20.log` |
| `cargo test --workspace --exclude editor-app --jobs 2` | 82 组、325 通过、322 ignored、0 失败；包含 1.4 协商／路径／上下文边界 | `unchanged-folds-nonui-2026-10-20.log` |
| `cargo check --workspace --jobs 2` | 通过，保留已有 warning | `unchanged-folds-check-delivery-2026-10-20.log` |
| 交付测试 EXE 的 `editor::comparison::render_tests:: sdk_export::tests:: ui::controls::aligned_ ui::plugin:: --test-threads=1` | 113 通过、23 ignored、0 失败，15.14 秒；未执行的 ignored 项不计通过 | `unchanged-folds-ui-delivery-2026-10-20.log` |
| 交付测试 EXE 的 `unchanged_folds --test-threads=1` | 3/3，7.92 秒；两／三栏、深浅主题、100%／150% 缩放、实际鼠标展开、源文本／选择／事件、源导航和原生光标揭示 | `unchanged-folds-native-delivery-2026-10-20.log` |
| 原生输入／对齐 `ui::controls::aligned_ code_sync::alignment_tests --ignored --test-threads=1` | 显式执行 20/20，54.94 秒，生产代码与最终候选相同；后续仅增强测试 | `unchanged-folds-native-input-2026-10-20.log` |
| 配套 `Nanobug.exe --plugin-cargo ./Cargo.toml test --lib` | 独立访客 115/115，5.89 秒；通过内嵌 SDK 私有缓存构建 | 访客 `folding-unit-2026-10-20.log` |
| `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs` 与 `git_inline_history_preview_opens_readonly_two_and_three_pane_tabs --ignored --test-threads=1` | 同一最终 0.1.19 ZIP，2/2，153.81 秒；实际包进入公开 Manager，覆盖工具栏折叠／展开、固定标签切换保留、草稿和 Undo、原提交文件箭头、根／普通双源与合并三源只读、关闭来源／禁用清理 | `unchanged-folds-real-package-2026-10-20.log` |

测试 EXE 为 `target/unchanged-folds-tests-delivery-2026-10-20.exe`，由 `cargo test -p editor-app unchanged_folds --no-run --jobs 2` 生成后复制，不覆盖运行中的测试程序。实际包测试统一设置 `NANOBUG_GIT_CANDIDATE=C:/Projects/RustProjects/Editor/target/unchanged-folds-2026-10-20/git-0.1.19.zip`。依赖树确认 Kit／Base 均为 0.7.0，无 gpui-component。

### 候选交付与身份

所有产物位于 [unchanged-folds-2026-10-20](../../../target/unchanged-folds-2026-10-20/)，该目录另含 `identity.json`、`SHA256SUMS` 与 `使用说明.txt`。32 个宿主相关输入及 65 个独立访客输入摘要已读回一致；这是当前脏工作区的选定输入记录，不是完整宿主源码快照。

| 产物 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `Nanobug.exe` | 78,075,904 | `7b5a40dd2983f0ad1f3663a636c2cfe5f8edd8ae3edefb0ec6fd8bcc5431a52b` |
| `git-0.1.19.zip` | 836,113 | `c43afe7a04d2ac69ca47c7a5e288cb6dae11ecb1c34cf87a1b99d1b90cb845de` |
| ZIP 内 `git.wasm` | 2,594,976 | `4555f49c832075f7a1c3e32be8c8dc36c902c92297bf6d63fdb13516382419e1` |

宿主通过 `cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/unchanged-folds-2026-10-20/Nanobug.exe` 构建，日志 `unchanged-folds-release-2026-10-20.log`。已先用 `cargo metadata --no-deps --format-version 1` 核对 workspace／target；访客使用自己的 `target/folding-2026-10-20`。

SDK 缓存摘要为 `0739a6f43e2faeef07ad4ee48a45fbc8c532191352cf8da04169a56d439e18da`，关键比较契约源字节已与宿主输入读回一致。访客不依赖仓库 `crates/`，也不附带 SDK／GPUI 项目副本。使用配套宿主的 `--plugin-package . --output .../unchanged-folds-2026-10-20` 直接打包，最终日志为访客 `folding-package-delivery-2026-10-20.log`；ZIP 共九项，两张 SVG 与资源管理器源文件及访客文件逐字节相同，原生 helper 的实际 SHA 与清单声明一致。
