# 已完成改动的主分支整合

日期：2026-10-25（Asia/Shanghai）。状态：完成隔离构造，按下列验证边界交付本地主分支；未推送远端。

## 范围和边界

用户要求将已完成改动合入主分支，并明确选择保留进行中改动。开始时主分支及 `origin/main` 为 `56cb99ae`，所有远端分支已纳入该提交；本地 `codex/canvas-protocol-extension` 的新阶段仍在审查，不在本次范围内。

本次整合此前已验收的文件标签和窗口按钮排序、布局与拖动、Hover、消息等级、插件服务启动状态、Maven 所需通用语言上下文、Git 工作台所需公开能力与宿主回归，以及独立原生终端库和原生文字网格。共享 UI 的 S0/S1 设计及本地工单仅作为设计资料保存，不表示相关功能已实施。独立插件仓库、用户安装、发布和远端推送不在本次操作范围内。

统一 Editor 迁移、Canvas 场景图形阶段和终端首次启动／恢复显示修复留在原任务。候选依据统一 Editor 任务开始前保存的源码快照构造，并排除快照中的首个迁移失败测试；终端保持此前已完成的核心融入版本。Canvas 阶段按已提交候选与主分支做三方差异分离，保留主分支最新的列宽取消修复和回归。

内部拖动性能的上游缓存补丁已被原任务撤回，本次仅保留测量入口和历史说明，不宣称性能优化已交付；依赖仍为锁定的 crates.io 版本，没有增加 vendor 或路径覆盖。

## 隔离和整合处理

- 独立 checkout 为 `C:/Users/Tmiracle/.codex/worktrees/merge-completed-20261025/Editor`，Cargo 输出为该目录的 `target/`；`cargo metadata --no-deps --format-version 1` 已核对路径。未复用其他源码快照的输出目录。
- 原工作区的进行中源码不参与候选构建。主分支更新时仅接纳经验证的提交，保留原工作文件中的后续修改。
- 去除快照与主分支重复的三个 SDK 导出登记，保留新增 Textarea 契约测试输入。
- 为 45 份终端录制字节流设置二进制 Git 属性；逐份核对暂存 blob 与原文件 SHA-256，防止 CR/LF 和空格转换破坏录制回放。没有修改录制内容。
- 历史验收文档保留当时状态和限制；本记录仅证明本次候选的检查，不将历史 skipped 项当作通过。
- 整合复查修复两处交互回归：仅把带定义跳转修饰键的 Hover 鼠标按下清理延后到原生点击处理之后，保留 Ctrl+点击所需的定义缓存，普通手势仍立即清理；新显示的右侧列按 Base 的最小可见宽度分配空间，避免继承隐藏零宽度后把插件按钮和输入框挤到窗口外。现有回归增加实际窗口边界断言。
- 旧布局恢复测试同步断言新增的隐藏 Open Files 和 Problems 叶子，仍逐项核对旧布局、尺寸和原插件身份。
- 相关 SDK、宿主 UI 与固定测试输入存在共同依赖，本次使用一份经过隔离验证的整合提交，未把需要后续进行中源码才能构建的中间版本写入主分支。

## 本次验证

命令在独立 checkout 执行，Cargo 固定 `--offline --locked --jobs 2`，没有升级依赖或安装工具链。

- `cargo fmt --check`、`git diff --cached --check`：通过。
- `cargo test --workspace --exclude editor-app --offline --locked --jobs 2`：通过，760 passed、407 ignored（包括文档测试）；宽度修正后复跑通过。
- `cargo check --workspace --offline --locked --jobs 2`：通过；最终复跑通过。
- 固定 WASM 安装测试 `wasm_install_respects_global_opt_in --ignored --exact --test-threads=1`：1 passed，实际执行约 31 秒。
- `cargo tree --workspace --target all --locked --prefix none`：通过，没有 `gpui-component`；原先离线树检查缺少 `gpui-pre-apple 0.3.9` 元数据，补齐锁定依赖后宿主架构测试通过。
- `cargo test -p editor-app --bin editor-app --offline --locked --jobs 2 --no-run`：通过。最终串行运行结果与原生截图复验见本记录末尾。

## 已知验证限制

- 未限定 `--bin editor-app` 的宿主测试命令会编译既有示例 `crates/editor-app/examples/svg_preview_render.rs`，其 `Paint` 匹配缺少主分支已有的 `TextBox` 分支而失败；该示例与整合前主分支一致，本次未扩大修改。
- 完整宿主串行运行发现文件监视保存测试和后台文件保存门禁测试失败，`shortcuts_open_and_restore_focus` 长时间不结束。相同三个测试也在另一份已有的 Canvas 候选测试程序（clean checkout HEAD `d12c4d21`，非纯主分支构建）复现失败／超时；本次未修改这些路径。最终串行复验显式排除这三项，不计为通过。
- 宿主并发运行还出现只在并发时出现的测试失败，最终采用 `--test-threads=1`。历史或本轮未显式执行的 ignored 测试不属于本次通过范围。

日志与字节摘要位于 `C:/Users/Tmiracle/.codex/merge-completed-evidence-20261025/`。首次暂存检查发现录制数据被当作文本，以及终端 Cargo 清单多余空行；设置属性并重新规范化暂存、移除空行后检查通过。独立 worktree 的旧空 index 锁被保留到证据目录；候选使用专属 index，未删除其他任务的锁或修改其暂存内容。

## 最终复验结果

- 宿主测试程序串行运行，使用三个 `--skip` 排除上述文件监视、后台保存及快捷键问题：**775 passed、0 failed、373 ignored、3 filtered out**，约 291 秒，见 `app-final.log`。
- 原先对所有点击延后 Hover 清理的中间修正通过了 mock 测试，却使原生诊断 Hover／长按拖动复验失败；最终仅对 `Modifiers::secondary()` 延后清理，普通手势保持原逻辑。缩小后的修改追加定义跳转 2 项和 Hover 16 项定向通过，未重复无关的完整宿主测试。
- 最终 Windows 原生验收：`hover_native_windows_acceptance --ignored --test-threads=1`、`window_toolbar_native_windows_acceptance --ignored --test-threads=1` 各 **1 passed**，分别约 14 秒和 12 秒。截图在 `hover-scoped/`、`toolbar-scoped/`，已查看实际深浅主题及拖动、诊断结果；诊断文字为测试夹具。
- 最后再次执行 `cargo fmt --check` 和 `cargo check --workspace --offline --locked --jobs 2` 通过。忽略项、三项显式排除以及未编译通过的既有示例仍按前述限制记录。
- 主分支采用祖先校验和旧值比较后的快进，仅替换原先干净的 index 基线；不 checkout、不 stash、不改写工作文件。最终快进前后的文件摘要及 `main-fast-forward.json` 留在外部证据目录，进行中源码仍作为未提交修改保留。
