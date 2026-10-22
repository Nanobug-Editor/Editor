# 比较视图整行底色、外侧边距与默认同步

日期：2026-10-20。状态：实现、129 项相关回归、规定检查与最终宿主候选构建通过。

## 本轮行为

- 左侧比较正文保留 12 个逻辑像素外侧边距；行号槽继续位于两编辑器之间。
- 改动行底色覆盖正文、软换行的行尾空白和行号槽。字符差异仍由原生范围装饰强调；文字、选择与光标由原生编辑器重绘，不在它们上方叠加半透明整行颜色。
- 删除比较工具栏的“同步滚动”开关及其可变状态；双／三栏默认共享滚动。原生搜索／代码操作期间的短暂暂停和布局预算回退继续保留。
- 通用比较标签、统一视图及声明式原生代码组的只读来源共用本地 `ui/controls/row_backgrounds.rs`。它只保存当前范围／颜色和弱来源身份，不复制上游源码，也不引入第二份文本或 Undo；完整原生挂载继续提供输入、IME 与可访问性。
- 公开协议、SDK 和独立 Git 插件源码／包未修改；继续使用独立 Git 0.1.17。

## 缺陷反馈循环

`cargo test -p editor-app editor::comparison::render_tests:: -- --test-threads=1`

修复前两项均失败：左首字符 x=0 贴边；改动行尾及行号槽采样点没有差异底色。日志为 `target/comparison-background-red-2026-10-20.log`。原有 `RangeDecorationStyle::Fill` 仅填充字符范围；上游没有整行且包含行号槽的公开样式。

本地组件按原生 caret／行首几何定位改动带，保留完整源宽重绘可见带。行号槽采用相同的不透明混合底色，保留原生字形、选择、折叠及遮挡关系，横向滚动的正文不会透进行号。呈现层通过弱拥有者独立撤销；相同范围／颜色不产生额外空闲通知。

## 最终验证

| 范围 | 实际结果 |
| --- | --- |
| 最终普通回归 | `editor::comparison::render_tests::`、`ui::controls::aligned_`、`ui::plugin::`：104 通过，23 ignored，8.03s；分别为新增 4 项、对齐 32 项与通用 UI 68 项，忽略项不计通过 |
| 最终显式原生回归 | `ui::controls::aligned_ code_sync::alignment_tests --ignored --test-threads=1`：20 通过，54.81s；含 6 项指针和 14 项三源、IME、原生鼠标、旧手势拒绝、尺寸调整与空闲停止绘制 |
| 最终真实 Git ZIP | `extensions::windows_tests::comparisons::`、`merge::`、`history_preview::` 显式 `--ignored --test-threads=1`：5 通过，336.61s；完整覆盖草稿与统一视图、来源更新／关闭、长软换行真实滚轮、双／三栏只读历史标签及冲突块手工审查；历史标签还断言没有同步开关 |
| 非 UI workspace | `cargo test --workspace --exclude editor-app --jobs 2`：82 组，321 通过，322 ignored，0 失败；后续调整仅涉及 `editor-app` 绘制及测试夹具 |
| 最终编译与格式 | `cargo check --workspace --jobs 1`、`cargo fmt --check`、`git diff --check` 均通过；现有编译／链接警告和行尾转换提示保留，不计为本轮修复 |

真实 ZIP 首轮 4 通过、1 失败，331.17s。失败位于长软换行夹具的滚轮前置条件：预设共同文本每行 42px，实际为 21px，五行只占 105px；改为根据实际视口放置锚点。继续运行后，历史断言还将位于正文左缘的零宽 caret 当作可相交矩形，导致可见性误报；改为测量实际 `shared` 字形范围。原来的两次真实滚轮、双侧可见／对齐、源文本与选择不变断言均保留。两次失败日志分别为 `target/comparison-background-package-2026-10-20.log` 与 `target/comparison-background-softwrap-green-2026-10-20.log`，不计为通过；修正定位后单项通过 65.16s，最终上述整组再次通过 5/5。

独立／统一来源还补充了“改动块末端超出可见区域”的真实绘制红绿回归：原生 `range_to_bounds` 不为未布局的行提供矩形，不能因为块尾不可见而忽略块内的可见行。本地组件按原生可见行裁剪范围；首次绘制或滚动发现新带时只安排有限的后续帧。新增单项先失败 0.07s，再与其他三项共同通过 0.71s；失败日志为 `target/comparison-background-independent-red-2026-10-20.log`。最终独立窗口夹具为左右来源提供完整文件变化范围，20 项原生回归确认带底色时仍能停止空闲绘制。

## 交付身份与范围

最终宿主为 `C:/Projects/RustProjects/Editor/target/diff-row-backgrounds-final-2026-10-20/Nanobug.exe`，77980672 bytes，SHA-256 为 `4c0190a097630fa0a5f5f72a23da7fcbeaf6c7fe465eff69869d61931b15d623`。命令为 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o <最终目录>/Nanobug.exe`，4m08s。与未包含最后可见行边界修正的中间目录 `target/diff-row-backgrounds-2026-10-20/` 区分，交付以最终目录为准。

使用既有独立 `target/git-default-selection-2026-10-19/git-0.1.17.zip`，830048 bytes，SHA-256 为 `5256c918fdbb918f87e9f71048d8b51bf16675b37b115f36ca21f76741746552`，本轮已读回核对。未修改插件包、安装权限、公开能力或 SDK；用户保存工作并退出旧实例后运行新宿主即可。最终目录保存使用说明、`identity.json` 和 `SHA256SUMS`。

`cargo metadata --no-deps --format-version 1` 核对 workspace 为本仓库、target 为本仓库 `target/`。当前分支为 main，基线 HEAD 为 `fc9dc531ef991315d73e21c8a092f12a0f1ff78b`；共享工作区原有修改保留，不归入本轮成果。测试程序复制到专用 `target/diff-row-backgrounds-tests-delivery-2026-10-20.exe` 后执行，以避免 Windows 重链接运行中的 EXE；不是另一份源码或测试专用宿主 API。`cargo tree -p editor-app --prefix none` 确认使用 gpui-base／Kit 0.7.0，没有引入 gpui-component。

本轮验证为 Windows GPUI 测试窗口、原生输入／几何和真实受控 Git 进程，未启动用户工作实例，不包含桌面 D3D 截图或系统 IME 候选弹窗。未提交、推送、自动安装或重启用户程序。

最终日志：`target/comparison-background-{ui-delivery,native-delivery,package-delivery,nonui,check-delivery,fmt-delivery,diffcheck-delivery,build-delivery}-2026-10-20.log`。新增四项回归与中间红绿记录另见同前缀的 `red`、`green-final`、`complete-green`、`independent-red` 以及 `softwrap-green-final` 日志。忽略项仅以上述显式运行记录计为通过。
