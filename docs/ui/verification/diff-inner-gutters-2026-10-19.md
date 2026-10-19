# 差异比较内侧行号槽

日期：2026-10-19。状态：54 项相关回归、规定检查及最终宿主候选构建通过。

## 行为与边界

双栏比较的左源行号槽放在左编辑器右缘，右源行号槽保持在右编辑器左缘，两槽相邻。行号及折叠图标仍由现有原生编辑器绘制；横向滚动只移动正文，增删行的显示补白不增加源文件行号。宿主比较标签及显式采用原生对齐的通用插件代码组复用同一呈现，不按 Git 插件身份分支。

本地 `ui/controls/aligned_editor/gutter.rs` 从公开原生字形与输入边界测量真实槽宽，组合裁剪区域，把原生槽像素移到内侧。最后的原始编辑器挂载继续提供布局、文档、选择、输入和 Undo 权威；没有新建第二份可变文本，没有复制／fork 上游项目。原生行号与折叠开关不被改写，关闭比较直接恢复普通原生呈现。

垂直对齐暂停／预算回退／原生搜索打开时，水平槽位置继续保持。已有原生控制工厂同时用于可见副本和最后的权威挂载，搜索头部全宽保留；指针捕获排除该头部。鼠标、源菜单、IME 和弹出层使用同一已绘制帧投影；拖选保留按下时的正文／槽区域，栏宽或槽几何变化时释放旧原生拖动。回退期间同样设置绘制通知屏障，避免像素副本产生空闲重绘循环。

`project_range` 的可见性判断以 0.5px 探针检查零宽光标；返回的原始光标尺寸与源偏移不变。这修复行首位于物理最左缘后，被严格矩形相交误判为不可见的问题。

## 验证

| 范围 | 入口 | 结果 |
| --- | --- | --- |
| 原生内侧槽及对齐 | `cargo test -p editor-app ui::controls::aligned_ --jobs 1 -- --test-threads=1` | 最终 32 通过，6 ignored，3.66s；包含 4 项新增原生槽回归、两主题、150% 窄栏、真实滚动／选择／输入／Undo／IME 桥与可见头部；6 项随后显式运行 |
| 原生指针路由 | 显式运行 `ui::controls::aligned_ --ignored --test-threads=1` | 6 通过，0.07s；原点拖选、退休、嵌套取消和 Root 弹出层隔离 |
| 通用插件代码组 | 显式运行 `code_sync::alignment_tests` 的 ignored 测试 | 14 通过，58.16s；原生点击、三源增删行、输入／IME／Undo、栏调整、空闲停止绘制和生命周期 |
| 真实 Git ZIP 的历史比较 | `windows_tests::history_preview`，显式 `--ignored`，独立 0.1.14 | 1 通过，61.06s；真实文件点击、双／三源标签、只读、固定与回收 |
| 真实 Git ZIP 的工作区比较 | `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs`，显式 `--ignored` | 1 通过，59.89s；共享原文档、未保存编辑、比较切换和统一视图 |
| Rust 格式 | `cargo fmt --check` | 通过 |
| 非 UI workspace | `cargo test --workspace --exclude editor-app --jobs 2` | 78 组，295 通过，309 ignored，0 失败；忽略项不计通过 |
| workspace 编译 | `cargo check --workspace --jobs 1` | 最后窄栏边界调整后复查通过 |
| 差异检查 | `git diff --check` | 通过；已有行尾转换提示不是检查错误 |

首轮原生回归发现了上述零宽光标边界问题；字体上下文不同的槽宽断言改为同一主题、同一字体上下文的真实普通原生编辑器作对照，跨栏绝对坐标相减仅允许 0.01px 的浮点容差。搜索头部用稳定的本地原生输入实体验证可见像素和真实输入，不能用透明权威挂载的几何冒充可见头部。另检查 150% 缩放的 80px 窄栏：不能将槽宽限制为半栏，留下原生槽的一部分在外侧。

## 交付与范围

Git 包继续使用独立的 `target/git-inline-preview-2026-10-19/git-0.1.14.zip`，SHA-256 为 `486f81228ce163d899cf4f1def38f35f1f5ef08b04f93d8c0536baa0af99a9d9`，本轮已读回核对。访客源码、清单版本和公开协议均未修改；本次是通用宿主 UI 呈现改动。

新宿主为 `target/git-inner-gutters-2026-10-19/Nanobug.exe`（118602752 bytes），SHA-256 为 `5eba1ae4af4eee13767997ed3d8bcff3f318ade71a6aa37be5508f12e197960f`。同目录保存使用说明、`identity.json` 和 `SHA256SUMS`，可继续使用上述独立 Git 0.1.14。最终构建使用 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o <候选目录>/Nanobug.exe`，3m49s；先前中间候选构建后新增的窄栏修正已在最终构建重新纳入。未覆盖运行中的 `target/release/editor-app.exe`。

不自动升级用户安装、不覆盖运行中的程序，也不提交／推送当前工作区。已有及并发侧栏、停靠、终端等修改保留，不归入本次行号槽成果。完整多语言／缩放矩阵和系统 IME 候选弹窗仍须与本次原生 GPUI 输入桥回归区分。

日志：`target/git-inner-gutters-{aligned,tests,pointer-native,code-native,history-native,worktree-native,nonui,format,workspace-check,diff-check,build,build-final}.log`。
