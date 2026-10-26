# 统一 Editor 的实时换行与内部分隔线性能修复

日期：2026-10-25。状态：修复已在 `codex/unified-editor` 完成，并以未提交差异合并至本地 main；工作区与合并后主目录的原验收检查均通过。随后将补丁迁入[个人 gpui-kit Fork 的固定提交](https://github.com/T-miracle/gpui-kit/commit/4db6da3927224d2d856b5f3c21b9aa5103eac083)，Editor 改由该提交提供 Kit、Base 和 Assets 0.8.0。工作区为 `C:/Users/Tmiracle/.codex/worktrees/unified-editor/Editor`，主目录为 `C:/Projects/RustProjects/Editor`。实时换行保留，跨断点慢帧限制见下文。

## 授权与原因

用户明确允许“给锁定的 gpui-base 0.8.0 做本地补丁，接受维护依赖副本，保留实时换行”，并要求修复后合并至主分支。原修复阶段不升级依赖、不提交、不推送；随后用户另行要求在个人 GitHub 账号 Fork gpui-kit，把增量修复提交到 Fork 分支，并让 Editor 改用该 Fork。该后续授权仅用于 Fork 分支的普通提交与推送。

[统一前后诊断与公开接口实验](../../plugins/verification/unified-editor-2026-10-25.md)确认：原有布局/绘制优化仍保留；历史换行缓存补丁此前撤回，发布版 Base 每次改变宽度都会重测全文。公开文本系统缓存虽降低了塑形成本，仍未通过性能断言，实验产品代码已移除。

## 补丁边界

- 原修复先以发布的 `gpui-base-0.8.0.crate`（SHA256 `d5cd7de73d5b2d3d18b94a928abf343922091fa77bc472f77b7d2e7e8b1823a7`）建立本地副本，原始逐文件审计保存在 `target/resize-fix-20261025/vendor-audit.json`。随后将四个 Rust 增量文件按规范化换行逐字迁入 Fork 的 `crates/base`，基线为上游 0.8.0 发布提交 `1676826d9d49b90f4d7b6808b69924f58796f6f1`；本地依赖副本已移除。
- 唯一改变的原有上游源码为 `crates/base/src/input/editor/display_map/text_wrapper.rs`；新增 `measurements.rs`、`wrap_width.rs` 和 `wrap_width/tests.rs`，见[补丁提交](https://github.com/T-miracle/gpui-kit/commit/4db6da3927224d2d856b5f3c21b9aa5103eac083)。
- 在相同文本、字体、缩进和内联元素条件下，复用换行决策仍有效的宽度区间；跨越断点立即重算受影响行。优先检测合法 Unicode 单词断点，超长单词仍按完整 grapheme 换行。原算法保留为独立测试对照。
- 精确片段宽度缓存有 32,768 条、4 MiB 文本负载上限；不保留原生字形资源，不估算字宽。文本、字体、字号、缩进、内联元素及换行开关变动撤销相关缓存。
- `EditorState` 继续独占文本、选择、IME、布局与 Undo。没有拖动结束后才换行、宽度量化、第二份可变文档或宿主专用接口。
- 根 Cargo 以该 Fork 提交覆盖 Kit、Base、Assets 0.8.0；锁文件锁定同一 Git SHA，三者不再从 crates.io 或仓库内 `vendor` 加载。GPUI 的 `gpui-pre` 0.3.9 仍按原有锁定来源解析。依赖树无 `gpui-component`。

## 同一真实字体 CPU 基准

将 `source_resize_frame_cost` 从 NoopTextSystem 改为真实平台字体后端，其余 1500 行输入、36 次宽高变化、前四帧预热和断言不变。此数据不能与旧模拟字体结果直接作倍数比较。

| Release，1500 行 | 横向 p50 | 横向 p95 | 纵向 p50 | 原断言 |
| --- | ---: | ---: | ---: | --- |
| 无补丁、无实验缓存 | 178.3175 ms | 180.9059 ms | 0.4121 ms | 失败 |
| 本地 Base 换行补丁 | 0.3806 ms | 0.4155 ms | 0.4001 ms | 通过 |

断言仍为横向 p50 ≤ 纵向 p50 × 3 + 2 ms。日志为 `native-font-final-baseline.log` 和 `native-font-patched.log`，位于工作区 `target/resize-fix-20261025/`。

## 真实 Windows 分隔线

使用真实鼠标按键与移动、实际原生窗口、当前显示器报告的 144 Hz，按内容几何变化的有效绘制计数。Release 测试阈值仍为显示器刷新率的 90%，测试结束释放按键、恢复指针并关闭测试窗口。

| 场景 | 有效绘制/秒 | 间隔 p95 | 最长间隔 | 结果 |
| --- | ---: | ---: | ---: | --- |
| 1500 行，横向分栏 | 138.67 | 7.466 ms | 61.159 ms | 通过 |
| 1500 行，纵向分栏 | 142.82 | 7.502 ms | 14.471 ms | 通过 |
| 1500 行，浅色停靠分隔线 | 138.13 | 8.183 ms | 75.277 ms | 重试通过 |
| 当前实际 `main.rs`，横向分栏 | 143.43 | 7.761 ms | 14.004 ms | 通过 |

停靠场景首次没有产生实际几何变化（`changing=0`），该次失败不计为性能通过；同参数重试产生有效拖动并通过，截图 `dock-probe.png` 已检查实时换行与浅色 UI。重试时后台编译仍在运行。大量相似行同时跨越换行断点仍可能出现个别慢帧；以上结果证明持续全文重测瓶颈消除，不承诺所有文档每一帧都满足刷新周期。

## 回归与复现入口

目前已完成：根 `cargo fmt --check`、补丁文件独立 rustfmt 检查、依赖树审计；独立 Base 输入测试 361 passed、0 failed。新增 Windows 真实字体对照覆盖 Arial/Consolas、两种字号和缩进、中文、组合字符、Emoji、连字、阿拉伯文、超长词以及相邻浮点宽度；既有回归覆盖编辑、IME、选择、撤销、内联元素和换行映射。

```powershell
# 先核对当前工作区输出目录；其他源码副本不得复用这里的产物。
cargo metadata --no-deps --format-version 1
cargo test -p editor-ui --release source_resize_frame_cost --locked --offline -- --ignored --nocapture --test-threads=1
$env:NANOBUG_DRAG_SURFACE = 'split' # 也测试 split-vertical、dock-resize。
$env:NANOBUG_DRAG_SOURCE_ROWS = '1500'
cargo test -p editor-ui --release native_window_drag_tracks_display_refresh --locked --offline -- --ignored --nocapture --test-threads=1
Remove-Item Env:NANOBUG_DRAG_SURFACE,Env:NANOBUG_DRAG_SOURCE_ROWS

# Fork 的 Base 测试使用 Fork 工作区锁文件和独立输出目录。
$kit = 'C:/Projects/RustProjects/gpui-kit'
$env:CARGO_TARGET_DIR = "$kit/target/nanobug-live-wrap-20261025"
cargo metadata --manifest-path "$kit/Cargo.toml" --no-deps --format-version 1
cargo test --manifest-path "$kit/Cargo.toml" -p gpui-base --features test-support --lib input:: --locked -j 8 -- --nocapture
Remove-Item Env:CARGO_TARGET_DIR
```

工作区完整检查已完成：

| 命令 | 结果 | 日志（`target/resize-fix-20261025/`） |
| --- | --- | --- |
| `cargo fmt --check` | 通过 | `fmt.log` |
| 补丁文件 `rustfmt --edition 2024 --check` | 通过 | `vendor-fmt.log` |
| `cargo test --workspace --exclude editor-app --locked --offline -j 8 --no-fail-fast` | 113 组、782 passed、0 failed、409 ignored | `workspace-tests.log` |
| `cargo check --workspace --locked --offline -j 8` | 通过，保留既有警告 | `workspace-check.log` |
| `cargo test -p editor-app code_sync --locked --offline -j 8 -- --test-threads=1` | 5 passed，14 ignored 另行运行 | `app-code-sync.log` |
| 同上加 `--ignored --nocapture --test-threads=1` | 14 passed，含固定 Package/WASM 输入、三栏对齐、行边缘点击、滚动、IME/Undo、场景撤销和独立工作台缩放 | `app-code-sync-native.log` |
| `cargo test -p editor-app aligned_source_frame --locked --offline -j 8 -- --ignored --nocapture --test-threads=1` | 2 passed，普通/大文件补全锚点及原生编辑历史 | `app-aligned-source.log` |

普通 workspace 测试亦实际执行并通过 `source_resize_preserves_composition_and_reflows_immediately`、只读快照/行边缘配置回归。未运行的其他 ignored 测试不计为通过。

## 主分支合并与复核

以本次修复开始时的文件快照为共同基线做三方合并，保留 main 的其他任务改动。合并的人工变更涉及 15 个路径，另引入原发布依赖文件；`.gitignore` 显式保留依赖的 `Cargo.toml.orig`，副本 217 个文件全部可被 Git 收录。核对合并前 2,626 个已有文件：无关内容未变化，原暂存条目不变，main HEAD 仍为 `ffd9efbe5176a23d6caf4424c83903f6d409f64e`。本任务没有提交或推送。

主目录重新核对 `cargo metadata`，源码根为主目录、输出为其自身 `target/`，没有借用工作区构建产物。再次通过格式检查、workspace 编译检查、完整 workspace 测试（782 passed、0 failed、409 ignored）及 `cargo build -p editor-app --release --locked --offline -j 8`。日志与结果保存在主目录 `target/resize-fix-20261025-main/`。

合并后的真实字体 CPU 测试再次通过：横向 p50 **0.3874 ms**、p95 **0.4234 ms**，纵向 p50 **0.4074 ms**。使用主目录重新构建的 Release 测试程序，在后台构建全部结束后复测：

| 1500 行场景 | 有效绘制/秒 | 间隔 p95 | 最长间隔 | 结果 |
| --- | ---: | ---: | ---: | --- |
| 横向分栏 | 138.67 | 7.497 ms | 60.998 ms | 通过 |
| 纵向分栏 | 143.40 | 7.358 ms | 14.167 ms | 通过 |
| 浅色停靠分隔线 | 139.88 | 7.515 ms | 60.577 ms | 通过 |

三项均通过当前 144 Hz 显示器的原验收门槛；仍记录约 61 ms 的少数跨断点慢帧，不宣称完全没有慢帧。

实际主目录 Release 产物为 `C:/Projects/RustProjects/Editor/target/release/editor-app.exe`，89,101,312 字节，SHA256 `d6e45d3b8ca085da22e9fd95bcd1ba8bdeda70e5ae818e757e29b46ad277d474`；元数据见 `target/resize-fix-20261025-main/release-artifact.json`。未替换安装目录中的程序。

## 个人 Fork 迁移

Fork 位于 `https://github.com/T-miracle/gpui-kit`，本机仓库 `C:/Projects/RustProjects/gpui-kit` 与主 Editor 目录同级，Codex 本地项目 `gpui-kit` 指向该目录。补丁分支为 `fix-live-wrap-resize`，从上游 0.8.0 发布提交 `1676826d9d49b90f4d7b6808b69924f58796f6f1` 建立，补丁提交为 `4db6da3927224d2d856b5f3c21b9aa5103eac083`。该分支只用于锁定版本的修复，没有把更新的上游 main 当作依赖升级。

迁移前逐一比较 Fork 的四个 Rust 文件与本地 Base 补丁，规范化换行后内容相同。Fork 独立 `cargo fmt --check`、`git diff --cached --check` 和 `cargo test -p gpui-base --features test-support --lib input:: --locked -j 8 -- --nocapture` 通过，后者 361 passed、0 failed。Fork 提交已推送，新分支远端 SHA 已读回核对，旧分支引用已删除。

两个 Editor 目录的 `Cargo.toml`、`Cargo.lock` 与本节说明一致：`gpui-base`、`gpui-kit`、`gpui-kit-assets` 都解析到同一 Fork SHA，仍为 0.8.0；未引入 `gpui-component`，原 `vendor/gpui-base` 目录已移除。工作区和主目录各自使用独立的 `target/personal-gpui-kit-20261025*` 输出目录，`cargo fmt --check`、`cargo check --workspace --locked -j 8` 和 `cargo test --workspace --exclude editor-app --locked -j 8 --no-fail-fast` 均通过；两份完整测试各有 113 组、782 passed、0 failed、409 ignored。两个目录的 `cargo test -p editor-app code_sync --locked` 各有 5 passed；工作区另以 `--ignored --test-threads=1` 显式运行 14 项原生用例，全部通过。

工作区使用 Fork 依赖重新构建 Release 测试程序，真实字体 `source_resize_frame_cost` 通过：1500 行横向 p50 **0.3877 ms**、p95 **0.4106 ms**，纵向 p50 **0.4087 ms**。日志为 `target/personal-gpui-kit-20261025/release-source-resize.log`。

| Fork 依赖，1500 行 | 有效绘制/秒 | 间隔 p95 | 最长间隔 | 结果 |
| --- | ---: | ---: | ---: | --- |
| 横向分栏 | 139.82 | 7.504 ms | 59.872 ms | 通过 |
| 纵向分栏 | 143.41 | 7.398 ms | 13.883 ms | 通过 |
| 浅色停靠分隔线 | 139.88 | 7.443 ms | 59.739 ms | 通过 |

三项均使用真实鼠标拖动、144 Hz 显示器及原有 90% 帧率门槛。横向首跑 `changing=0`，没有产生实际几何变化，该次失败保留在 `release-native-split.log`，不计为性能通过；未修改源码或阈值的重试产生 278 次几何变化并通过，日志为 `release-native-split-retry1.log`。其他两项日志为 `release-native-split-vertical.log`、`release-native-dock-resize.log`。三份 `fork-native-*.png` 已检查实际分栏、实时换行及深浅主题；仍存在约 60 ms 的少量跨断点慢帧。

本次来源迁移已同步到本地 main，保留 Editor 的既有未提交状态，未提交或推送 Editor。前节主目录默认 Release 产物属于迁移前的修复验收；本次重新构建的是工作区 Fork 依赖的 Release 测试程序。
