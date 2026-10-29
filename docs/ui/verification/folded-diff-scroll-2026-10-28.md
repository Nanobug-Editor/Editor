# Git 差异折叠后的滚动修复

日期：2026-10-28～29。状态：任务工作区的实现、回归、workspace 门禁及真实 Windows 滚动验收结果见下文。

2026-10-29 集成：实现保存为提交 `289fb6c2`，与资源管理器筛选一同汇入本地主分支。本次按用户要求只做 Git 差异、冲突处理和提交归属检查，不重跑构建、测试或原生验收；下文保留原工作区当时的验证结果。

## 范围与原因

用户截图是三栏 Cargo.lock 比较，未修改区分别收起了 1,734、41、1,871 行。基线为 `7aa5ecfe0deb7a5f73aa637bd1ea29a67edb359f`，分支为 `codex/diff-fold-scroll`，工作区为 `C:/Users/Tmiracle/.codex/worktrees/diff-fold-scroll/Editor`。主目录原有的 Cargo.lock 修改未复制或改动。

当前代码及[此前折叠滚动记录](scroll-validation-folds-2026-10-22.md)核对出以下问题：

- 透明输入层按可见片段的完整来源包络分配高度。屏幕上相邻的差异块在源码中相隔数千行，这些隐藏行重新进入每帧原生布局。
- 折叠条使用 `occlude()`，同时挡住点击和滚轮；条带移到指针下方后，后续滚轮到不了共享视口，可能完全停滚。
- 底色缺失片段的补绘通知发给原始编辑器，被当作未知原生变化再次校验全文；校验又使补绘重新失效，形成循环。
- 不换行来源此前通过全文字形绘制确认横向范围。正常光标通知也会触发这条保守路径。

会话中曾成功检索到历史记忆 `cbabf57c-20e4-42e7-b873-eb5ab80a2b38`，文档 ID 为 `conversation:01a15c3e-b8d9-7482-accf-ac47046f662a:turn:01a16166-db77-7922-8317-2217f334c78e`。其关于此前完整字形缓存的记录已核对当前代码；后续续查返回 `unavailable`。最终结论以当前实现和测试为准，历史记录不扩大授权。

## 实现与输入边界

- `geometry_authority.rs`：输入层只分配物理视口、原生内边距及两行预留。路由在转发跨折叠事件前准备对应的有界原生窗口。
- `native_ranges.rs`、`input.rs`、`mod.rs`：保存可见副本真正绘制的只读原生光标矩形，补齐输入窗口外的 UTF-8／UTF-16 范围与命中查询。公开 display map 限定可见视觉行；COW 身份立即拒绝同长度编辑后的旧坐标。
- `folding.rs`：使用锁定 GPUI 的 `block_mouse_except_scroll()`，阻挡下层文本点击，同时保留共享滚轮命中。依赖版本未改变。
- `row_backgrounds/mod.rs`：补绘通知几何所属窗格，不再伪装成原生编辑器状态变化。租约退休后，旧回调不能访问或通知已撤销的所有者。
- `native_viewport.rs`、`geometry_display_map.rs`、`geometry_cache.rs`：对安全的折叠来源，读取原始原生控件的横向钳制边界，并在可见副本预绘制前恢复原位置。全部原生行元数据仍须连续、完整且匹配当前来源；不通过公开字段相等跳过未知通知，也不估算字形宽度。
- `aligned_pointer/mod.rs`、`pane.rs`：指针捕获期间暂停边界采样。选择或原生组合范围变化时先保留既有准入路径；定制工厂、原生隐藏行、掩码、非左对齐及无法取得有限边界的来源保留完整测量回退。原生范围变化同步更新横向缓存。

上述文件均位于 `crates/editor-ui/src/` 的对应模块。文本、选择、IME 和 Undo/Redo 仍由原始 EditorState／宿主输入委托管理，没有第二份可变文本或撤销栈。

## 可复现回归

证据目录：`C:/Users/Tmiracle/.codex/visualizations/2026/10/28/01a18312-b9b5-7a42-aebb-06d53cb89449/`。所有日志和 PNG 均在项目外。

| 场景 | 修复前／后与证据 |
| --- | --- |
| 4,200 行，差异位于 1,750／1,795／3,670 附近 | 原生布局 1,756 行，门槛失败；最终 14 行。`folded-red.log`、`aligned-final-complete.log` |
| 三次滚动及有限跟进绘制 | 原实现 debug 1,289.717 ms；最终 debug 176.061 ms，Release 37.261 ms。不同 profile 不计算优化比例；这不是单帧时间。 |
| 折叠条上的像素／行滚轮 | 新回归先因位置不变失败；最终两种输入均滚动，折叠、文本和选择不变。`disclosure-wheel-red.log`、`aligned-final-complete.log` |
| 原生通知后的底色补绘 | 修复前 1,281 行被重复测量 7 次，共 8,967 行；最终每次完整验证 1,281 行，后续补绘有限结束。`band-repaint-red.log`、`aligned-final-complete.log` |
| 隐藏长行的横向范围 | 新回归先记录 1,281 次隐藏字形行查询；最终无隐藏批次，横向终点与原生钳制一致，切回换行后 x 为 0。`native-width-red.log`、`aligned-final-complete.log` |
| 跨折叠平台范围、点击与拖选 | 旧坐标断言先失败；最终通过，包括同长度编辑后旧几何拒绝。`platform-query-red.log`、`aligned-final-complete.log` |

针对性集合还覆盖中文／emoji 预编辑、提交及撤销、宿主输入链、内侧行号槽、两／三栏、搜索暂停、缩放、字号、深浅主题、原生滚动请求和资源退休。

## 门禁

每次构建均核对 Cargo metadata，使用本 worktree 自己的 `target/`，未覆盖输出目录。

| 检查 | 最终结果 |
| --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test -p editor-ui --lib aligned_editor --locked --offline -j8 -- --nocapture --test-threads=1` | 38 通过、0 失败、2 ignored；`aligned-final-complete.log` |
| Release 内部行为集合 `aligned_editor::tests::` | 31 通过、0 失败、2 ignored；`aligned-release-complete.log` |
| `cargo test --workspace --exclude editor-app --locked --offline -j8 --no-fail-fast` | 正常 Windows 权限下 127 组、859 通过、0 失败、446 ignored；`workspace-native-permissions.log` |
| `cargo check --workspace --locked --offline -j8` | 通过；`check-complete.log`，保留既有警告 |

沙箱内相同 workspace 命令先有 81 项失败，包含临时文件 `PermissionDenied` 和 Windows 测试凭据设施不可用；日志为 `workspace-complete.log`。随后仅调整执行权限，保持源码、离线参数和 target 路径不变，完整复查通过。

此前整个 aligned_editor Release 集合的五个既有用例依赖 debug selector。锁定 GPUI 的 `elements/div.rs` 在 Release 下不记录这些 selector；相关交互由 debug 集合验证，Release 内部行为集合单独通过。ignored 项不计为通过，未执行无关的 WASM 夹具测试。

## 真实 Windows 验收

显式运行 ignored 测试 `scroll::tests::native_windows::smooth_scroll_real_windows_paints_intermediate_native_positions`，使用真实 Windows／D3D 窗口。夹具为 7,200 行，首个大间隔约 1,700 行，随后每隔 220 行有差异。保留默认光标闪烁、工作来源焦点、只读快照、底色与真实滚动求解器；换行开关明确应用到每个来源。

四组均设置 `NANOBUG_SCROLL_MIN_FPS=140`、`NANOBUG_SCROLL_MAX_FRAME_MS=30`、`NANOBUG_DIFF_NATIVE_SPARSE=1`，未降低或跳过门槛。每组超过 100 个真实前台移动帧，同时验证滚动中间位置、单调性、惯性尾段、最终停止及文本保持。

| 原生场景 | FPS | 最慢移动帧 | 子树 CPU p95 | 最终原生逻辑行上限 |
| --- | ---: | ---: | ---: | ---: |
| 两栏浅色，不换行 | 143.93 | 11.192 ms | 2.921 ms | 15 |
| 三栏深色，不换行 | 143.81 | 10.866 ms | 4.827 ms | 15 |
| 两栏浅色，换行 | 143.96 | 10.666 ms | 2.279 ms | 6 |
| 三栏深色，换行 | 143.94 | 11.892 ms | 2.714 ms | 6 |

日志和场景 PNG 使用 `native-complete-{two-light,three-dark}-{plain,wrapped}` 前缀，已检查文字、行号、底色及折叠条。早期前台帧无效、补绘循环、横向测量试验及被折叠条挡住滚轮的失败日志保留，不计作最终通过。

首次准入仍有秒级准备样本，不属于上述稳定滚动计时；未宣称首次打开性能已解决。截图是固定尺寸原生测试窗口，不是已安装程序截图。中文 IME 验证覆盖原生委托和预编辑状态，未做系统候选窗人工验收。

返回 [原生 UI 目录](../README.md)。
