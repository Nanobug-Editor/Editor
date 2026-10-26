# 统一默认 Editor 验证记录

日期：2026-10-25。状态：统一功能、针对性协议、实际插件包及下列功能回归通过；同日追加复查发现长文件横向拖动性能未通过，尚未修复，不能据功能测试宣称全部验收完成。用户更新工作流要求后，收尾迁入 `codex/unified-editor` 工作区分支，不提交、推送或发布。

## 变更与范围

按[已确认方案](../specs/unified-editor.md)，宿主普通编辑区及插件文档、文字快照、行边缘控件共用 `editor_ui::source_editor::Editor`。快照准备、原生行边缘绘制和只读挂载限制位于该组件目录；宿主保留文档身份、权限和会话集成。

插件协议只有 `Kind::Editor(Editor)`，以 `EditorSource`、`read_only`、`line_actions`、`changes`、`sync_group` 配置。`line_actions: None` 关闭边缘功能，`Some(empty)` 保留左右栏宽度。`ui.code_editor` 和 `editor.layout` 为 2.0，基础协议仍为 7；启用边缘控件另外需要 `editor.actions` 1.0。

旧节点不再执行。历史 ZIP 保留原字节；旧能力声明要求更新，但不清空启用选择、权限或私有数据。文档状态、中文 IME、选择、保存和 Undo/Redo 继续由原来的 `EditorState` / `DocumentSession` 管理。

## 独立消费者与候选

构建使用 `target/debug/editor-app.exe --plugin-package`，测试使用其 `--plugin-cargo`；所有消费者使用公开内嵌 SDK，没有宿主源码路径依赖或 SDK 副本。SDK 的 79 个输入逐字节与当前源码核对，SHA-256 为 `6907bc7111ec587048b8483ba4197e3e93a1099197c374da6f304169e2391f25`。

| 消费者 | 本机源码 | 候选版本 | ZIP SHA-256 |
| --- | --- | --- | --- |
| Git | `C:/Projects/RustProjects/Git-for-Nanobug` | 0.1.45 | `903d2ebf0b6e493dbee06c89f22c0e6e46998718d338f08825631deaea3ff505` |
| Markdown | `C:/Projects/RustProjects/Markdown-for-Nanobug` | 0.18.6 | `6a84326bb3a0debcecd57838434fc63669254e140488c4212127c8188104f633` |
| Image，安装 ID `svg` | `C:/Projects/RustProjects/Image-for-Nanobug` | 0.6.2 | `9a7b5a9da47a96512ba911823be531f0afcff04a3005ae84b7b4c59046976e72` |
| 布局测试访客 | `crates/plugin-runtime/tests/fixtures/layout-editor` | 0.4.0 | `0d130a3db62ea3b083d80d3dd26a0586a8726d32c9d00cb371493ec0e6faa7ac` |
| SDK 画布/文档夹具 | `crates/plugin-runtime/tests/fixtures/sdk-canvas` | 1.1.0 | `c4df42029dff925d03a0816279bb8c5eab4265f68d35eed1b2832ca89b7bc483` |

候选输出在 `target/unified-editor-20261025/packages-final/`；该目录在测试期间不再覆盖。五份相同字节的包已冻结到 `crates/plugin-runtime/tests/fixtures/packages/`，账本记录实际 SDK、源码指纹和未提交状态。`NANOBUG_TEST_PACKAGE_INPUTS` 用于冻结前的显式带摘要验证，不影响生产宿主；冻结后普通测试通过默认账本读取。

Image 原先没有本机源码，因此从官方 `Nanobug-Editor/Image` 克隆到上述目录后迁移；未 fork 仓库。Markdown/Image 的历史 `host-sdk.json` 发布 commit 保留，README 已说明本地候选必须使用本记录配套开发宿主。当前没有可供发布固定的新宿主 commit；本次不提交、推送或发布。

## 已运行的针对性检查

- 先增加统一节点的协议回归，观察旧实现无法识别 `Editor`，再迁移实现。快照边缘嵌套文档的新增回归先失败，再修复 root-only 限制。
- `cargo test -p plugin-protocol --locked --lib configurable_editor -- --test-threads=1`：6 项通过，覆盖只读快照、来源权限、独立同步配置、旧节点拒绝、UI revision 及空边缘配置。
- 通过公开宿主 SDK 运行三个独立消费者的 `test --locked --offline`：Git 199、Markdown 83、Image 8 项通过。
- `cargo test -p editor-app code_sync --locked -- --test-threads=1`：5 项通过；14 个 ignored 测试另行显式运行，未计入这 5 项。
- 同一已构建测试二进制运行 `code_sync --ignored --test-threads=1`：14 项全部通过，覆盖实际 IME、Undo、可见行、补白对齐、边缘按钮、滚轮、拖动撤销及双窗口共享会话。
- 实际 Git 0.1.45 包运行 `git_merge_edges_keep_nonconflicting_changes_and_require_manual_block_review --ignored --test-threads=1`：通过；独立临时 Git 环境和两原生窗口验证非冲突更改、手工冲突块及原 Undo。
- `cargo test -p plugin-runtime --test compatibility unified_editor --locked -- --test-threads=1`：通过；两种旧能力分别拒绝并提示更新，恢复安装后保留权限、启用选择、项目启用记录及私有字节。
- `composable_layouts --ignored`：实际独立 WASM 布局的协商、辅助面板限制、精确来源和过期事件验证通过。
- `file_views --ignored --test-threads=1`：2 项通过，Image 实际包保留文件专属资源、视口协商、撤销和作用域校验。
- `ui_migrations --ignored --test-threads=1`：3 项通过，覆盖既有控件数据恢复、Markdown 超限保留文档并恢复，以及 SVG 精确内存来源与缩放意图。
- `delivered_markdown_source_fills_height_below_toolbar --ignored`：通过，多尺寸、深浅主题、源代码/分栏模式、指针选择及滚轮均保留完整原生输入区域和文本。
- `delivered_source_height_survives_empty_documents_and_toolbar_free_previews --ignored`：通过，实际 Markdown 空文档、Image 无工具栏源代码及普通文档切换保持完整输入区域。
- `markdown_sdk_adoption_hot_update_retains_document_undo_and_retires_old_resources --ignored`：通过，失败候选保留当前视图，成功更新保留原生实体、未保存中文、更新前后 Undo/Redo 及私有数据，并撤销旧实例资源。旧 0.18.2 字节先明确拒绝；可执行热替换基线是同一迁移后真实访客的较低版本清单变体，不冒充已发布旧包。
- `composed_wasm_preview_follows_memory_and_reclaims_the_editor_split --ignored`：通过，SDK 画布夹具使用真实包、公开视图激活、原生输入与缩放，撤销后释放布局。旧测试的单轮 publication 不再足以等待冷启动，现通过现有公开 demand 接缝等待，再检查真实布局。
- `aligned_source_frame --ignored --test-threads=1`：2 项通过，默认 Editor 的小/大源文件补全锚点、上下文菜单与原生编辑历史保留。
- `delivered_markdown_source_scroll_reveals_the_corresponding_native_block --ignored`：通过，真实源代码滚轮在实际插件预览中显示对应段落。

上述显式原生 ignored 检查共 22 项；另外 5 项 `code_sync` 普通测试独立计数。未执行的其他 ignored 测试不计为通过。构建后的测试二进制副本保留在同一 `target/debug/deps` 目录中，避免与其他任务链接时争用运行中的 EXE，并保留 examples 相对查找。

首次文件预览预跑遇到重建正在覆盖候选路径，固定摘要校验拒绝该输入。随后改为上述固定候选目录和新摘要重跑通过；不能将首次失败计为功能通过。

## 工作区门禁与限制

`cargo build -p editor-app --locked`、`cargo check --workspace --locked`、`cargo fmt --check`、`git diff --check` 已通过。`cargo tree --workspace --target x86_64-pc-windows-msvc --locked --offline` 和 `Cargo.lock` 检查没有 `gpui-component` 及宏库；根 `Cargo.lock` 与任务开始的工作区快照完全相同。

本次修改的 Rust 文件另通过独立 `rustfmt --check --edition 2024 --config skip_children=true` 检查。最初共享输出目录遇到测试 EXE 被占用的 LNK1104；重跑普通测试 782 项通过，但两个 doctest 目标遭遇 E0463 依赖产物读取错误。两个目标单独重跑成功后，最终将完整验证切至独立输出目录，保留先前失败日志，不用子集成功冒充原命令成功。

最终完整命令与结果：

```powershell
$env:CARGO_TARGET_DIR = 'C:/Projects/RustProjects/Editor/target/unified-editor-validation-20261025'
cargo test --workspace --exclude editor-app --locked --offline --jobs 8 --no-fail-fast
cargo check --workspace --locked --offline --jobs 8
```

metadata 已核对源码根为当前主工程，输出目录为上述独立目录。完整测试 exit0，113 组、782 passed、0 failed、409 ignored；doctest 目标均成功。ignored 不计入通过数；受影响的真实 WASM 和 22 项原生 ignored 回归按前文另行显式运行。workspace 编译检查 exit0，保留既有编译警告。日志为 `C:/Projects/RustProjects/Editor/target/unified-editor-20261025/workspace-test-isolated.log` 和 `workspace-check-isolated.log`。

## 工作区分支与合并方式

用户在功能验收完成后要求后续工作使用新工作区分支。创建 `C:/Users/Tmiracle/.codex/worktrees/unified-editor/Editor`、分支 `codex/unified-editor`，以 `56cb99aeada229aefc9c17374175a2d03c72454b` 为基点；迁移前完整备份主工作区，逐字节核对 2,624 个文件并保留 8 项已有跟踪文件删除。迁移不改写主目录原有 361 条未提交状态。

主工程原有未提交实现作为迁移基线保留；工作流更新后的说明、验收收尾在该工作区修改。源码与先前已验证的 79 个 SDK 输入逐字节相同，功能回归复用同一份源码结果。新工作区的 metadata 路径核对、`cargo fmt --check` 和 `cargo check --workspace --locked --offline --jobs 8` 全部通过，输出为该工作区自身 `target/`，编译日志在 `%TEMP%/nanobug-unified-editor-20261025/worktree-check.log`。

合并采用迁移快照与当前 main 的内容对比，只包含迁移基线之后的三个文档差异，不重放其他未提交状态；保持未提交、不推送，保留其他任务改动。原功能代码在工作流更新前已位于 main，复制到工作区后未再改变；本记录不将此前的主分支实现表述为在新分支重新实现。

## 内部分隔线性能复查（修复前诊断）

后续状态：下述失败促成用户明确授权的 Base 0.8.0 本地补丁。修复与当前验收见[2026-10-25 性能修复记录](../../ui/verification/internal-resize-performance-2026-10-25.md)，本节保留诊断当时的证据，不代表修复后的状态。

用户在统一 Editor 后报告内部面板、编辑区分隔线重新卡顿。原验收运行了功能测试，但漏跑了 `source_resize_frame_cost` 及带长文件的原生拖动性能测试；这两项均为 `ignored`，782 项普通测试及 22 项其他原生回归不能覆盖它们。先前对“全部完成”的结论未披露这一性能缺口，现予以更正。

### 优化代码与源码对照

- 对照任务开始的 `baseline.zip`、迁入工作区前的 `before-worktree-main.zip`、当前工作区及 main，12 个相关文件字节相同：依赖清单/锁文件、窗口几何保存、图标筛选、目录索引/主题复用、后台发布锁、对齐几何缓存、源编辑器缩放测试及原性能记录。此次合并没有删掉这些已有优化。
- [原横向拖动性能记录](../../ui/verification/internal-resize-performance-2026-10-23.md)在本任务开始前已注明：2026-10-24 按禁止整包复制或 fork 上游的要求，撤回了 `gpui-base` 换行缓存补丁。依赖恢复为 crates.io `gpui-base 0.8.0`，该补丁不在统一前源码中。
- 当前注册表 `text_wrapper.rs` 与本机原始 `.crate` 内容相同。`TextWrapper::set_wrap_width` 在宽度改变时调用 `update_all`；现有 `EditorState::display_map()` 只读接口并不能替换内部全文换行计算。仍保留的原生行映射复用与该宽度缓存是不同优化。

### Release CPU 对照

在当前工作区及任务开始源码快照中分别运行同一原有测试，两个快照使用各自独立的 `target/`，均先核对 `cargo metadata --no-deps --format-version 1 --locked --offline`。测试使用同一份 1500 行中英文/emoji 文本，在同一 EditorState 上连续改变宽度或高度；以下是每帧 CPU 耗时中位数，不是屏幕 FPS。

| 源码 / 配置 | 横向 p50 | 纵向 p50 | 性能断言 |
| --- | ---: | ---: | --- |
| 统一前源码，软换行 | 6.3429 ms | 0.2274 ms | 失败 |
| 统一后源码，软换行，首次 | 6.2103 ms | 0.2226 ms | 失败 |
| 统一后源码，软换行，重复 | 6.2581 ms | 0.2259 ms | 失败 |
| 统一后源码，临时关闭软换行的定位对照 | 0.2299 ms | 0.2266 ms | 通过 |

```powershell
cargo test -p editor-ui source_resize_frame_cost --release --locked --offline --jobs 8 -- --ignored --nocapture
```

定位对照仅给测试进程设置 `NANOBUG_DRAG_NO_WRAP=1`，结束后清除，没有修改产品默认软换行。统一前源码从 `%TEMP%/nanobug-unified-editor-20261025/baseline.zip` 的 `Editor/` 恢复到同目录 `before-editor-resize/`。该早期文本备份不含 WIT 与图标，前两次构建因此失败；补齐 1 个 WIT、22 个静态资源后构建成功，并执行到上述性能断言失败。补齐内容来自迁移备份，逐个核对与 HEAD 仅可能有行尾差异，未用当前 Editor 实现替代旧源码。两个输入的 Cargo 清单、锁文件与缩放测试完全相同。

### 真实原生分隔线

使用当前工作区的 Release 测试程序，保持 800×500 逻辑像素窗口、同一原生分隔控件及原测试门槛。本轮显示器实际报告 144 Hz；仅统计前台窗口实际几何变化的绘制。

| 场景 | 结果 |
| --- | --- |
| 1500 行，横向分隔线 | 失败：全部采样仅 6 次几何变化，未达到有效运动样本门槛；不据此计算或宣称 FPS |
| 空内容，横向分隔线 | 通过：142.27 次有效位移绘制/秒，间隔 p95 7.428 ms |
| 1500 行，纵向分隔线 | 通过：142.22 次有效位移绘制/秒，间隔 p95 7.487 ms |

```powershell
$env:NANOBUG_DRAG_SURFACE = 'split'
$env:NANOBUG_DRAG_SOURCE_ROWS = '1500'
cargo test -p editor-ui native_window_drag_tracks_display_refresh --release --locked --offline -- --ignored --nocapture --test-threads=1
# 纵向对照使用 split-vertical；空内容对照移除 NANOBUG_DRAG_SOURCE_ROWS。
Remove-Item Env:NANOBUG_DRAG_SURFACE
Remove-Item Env:NANOBUG_DRAG_SOURCE_ROWS
```

`source_resize_preserves_composition_and_reflows_immediately` 另行通过，覆盖实时换行、深浅主题、中文 IME 与原 Undo。CPU 与原生对照共同支持：当前横向卡顿的主要瓶颈是原版 Base 宽度变化时的全文换行计算，统一前后都存在；不能将本次结果表述为已修复，亦不能把关闭软换行的定位对照作为产品修复。

全部日志及 `preservation-audit.json` 保存在当前工作区 `target/unified-editor-resize-20261025/`。本轮只修正文档状态与补记诊断，不修改产品代码、依赖或测试断言，不恢复已撤回的上游补丁；在 `codex/unified-editor` 完成文档检查后按既有授权以未提交差异合并回 main。此前功能测试结果仍有效，但没有重复运行无关的完整构建。

## 不修改上游的文字测量缓存实验

用户随后要求修复后再合并主分支。本阶段仍在 `codex/unified-editor`，沿用禁止 fork、升级依赖、提交和推送的限制；未将未达标的实验合并至 main。

现有 CPU 测试默认使用 GPUI 的 NoopTextSystem，低估真实字体塑形成本。因此将同一 `source_resize_frame_cost` 接到 `current_platform(false).text_system()`，保留同一 Editor、1500 行输入、36 帧宽高变化、前四帧预热及原性能断言，只替换测试字体后端。使用独立工作区 Release 输出，先确认该真实路径失败。

在 GPUI 的公开 `PlatformTextSystem` 接口上尝试跨短生命周期 `WindowTextSystem` 复用精确塑形结果：完整比较原始文本、字号和字体 run，保留原字形位置与索引，限制缓存容量，并在字体安装时撤销。实验未改变 Base 的换行算法或 EditorState。结果如下；本表与上节 Noop 字体 CPU 数据不作直接倍数比较。

| 同一真实字体测试 | 横向 p50 | 纵向 p50 | 原性能断言 |
| --- | ---: | ---: | --- |
| 缓存前 | 176.9936 ms | 0.4022 ms | 失败 |
| 精确原生测量缓存 | 8.6723 ms | 0.4099 ms | 失败 |
| 避免对已散列键重复散列 | 8.2211 ms | 0.4209 ms | 失败 |

计数探针显示，从第 50,000 次到第 200,000 次查询，新增查询均命中；条目数保持 6,042、估算文本/字形负载保持 8,419,713 字节。扩大缓存不能消除此时的全文遍历、临时映射和布局复制成本。这一公开接口方案显著降低测量成本，但不足以通过当前性能验收，不作为已完成修复。

实验代码已从产品模块移除，临时探针全部撤下；供复查的源码与日志保留在工作区 `target/resize-fix-20261025/experiments/native-measurement-cache/` 及 `target/resize-fix-20261025/native-font-*.log`。保留真实字体性能测试改进。用户随后明确允许给锁定 Base 0.8.0 做本地补丁并维护依赖副本、保留实时换行；按该授权实施的最终补丁及验证见[性能修复记录](../../ui/verification/internal-resize-performance-2026-10-25.md)，未延迟产品换行或放松断言。
