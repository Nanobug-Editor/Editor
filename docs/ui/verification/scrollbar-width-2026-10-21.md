# 宿主与插件统一滚动条宽度动画

日期：2026-10-21。状态：滚动条实现、专项行为验收和候选构建完成。扩展终端回归有两项失败，见下文，未宣称完整终端验收通过。

## 范围与行为

用户截图指向独立 Git 插件提交拓扑图右缘。该画布已经通过公开 `Canvas.scroll` 提供完整内容长度和偏移，但原来的尾部绝对定位覆盖层没有固定起点，使用布局视口时可被放到满高画布的下方。通用覆盖层现在明确锚定视口左上角；轨道仍在右缘，Base 继续负责比例、最小滑块、命中、拖动与边界。

宿主和插件统一使用本地 `crates/editor-ui/src/scrollbar/`：新视口／实际滚动后保留两秒，随后以约 180 毫秒将可见滑块宽度收缩到 0，颜色透明度不随隐藏改变。常态宽 6、滑块悬停／拖动宽 8 逻辑像素，边缘留白及点击区域固定；横向滚动条按相同规则改变厚度。轨道任意位置可以揭示隐藏滑块，移出原生窗口后释放悬停保留。隐藏或展开不会重排文本、改变换行宽度或移动内容。系统减少动态效果偏好继续由 Base 执行。

移除终端的一秒计时直接卸载和标签栏直接显隐指示条，统一接入原生控件。菜单、弹窗、补全及语言结果、安装／设置、运行配置、问题动作、插件容器／代码块等滚动区域使用共享控件；已有外部滚动句柄继续复用，以保留键盘定位及恢复位置。停靠根的结构性 overflow 不产生另一份内容或滚动条，面板内视口负责实际滚动。

原生 Editor／Textarea 没有每实例滚动条样式工厂。本地透明呈现包装在该输入 prepaint 期间同步投影 Base 的公开滚动条主题，随后恢复，绘制读取 Base 已解析的几何；不读取私有字段、不估算文档长度、不复制输入源码。只记录原滚动偏移、活动时间和一个弱引用计时器，Base 的宽度动画支持反向过渡；文档、选择、IME 和 Undo 仍归原有输入实体。

本轮没有修改独立 Git 源码／包、协议、SDK、能力或安装权限，不添加按插件 ID 的生产分支。Git 业务和画布内容 epoch 继续在独立插件内。

实施期间，用户在并行聊天“UI组件配置”中确认并执行独立 `editor-ui` crate 迁移。本轮沿用迁移后的实际目录；目录移动、公共 UI 构造接口、Cargo 测试依赖和测试输入适配属于该并行工作，不计为本轮滚动条成果。宿主画布回归仍在 `crates/editor-app/src/ui/plugin/scrollbar_tests.rs`，控件绘制回归随组件位于 `crates/editor-ui/src/scrollbar/tests/animation.rs`。

## 原生观察与夹具修订

初轮原生几何探针错误报告滑块一直为 6px。检查 D3D 截图及同一轨道像素后，确认实际滑块已经隐藏：GPUI 场景缓冲中仍可存在被裁剪或覆盖的旧绘制项，不能把第一个匹配矩形直接当作可见结果。另一个原因是普通原生验收默认调用 `set_reduce_motion(true)`，该设置本来就应跳过过渡。

最终探针按实际裁剪和绘制顺序选择候选，再通过当前 D3D 读回像素确认可见性；只在隔离的动画专项中开启运动。此前关于停靠缓存阻断动画的判断已撤回，临时尝试的局部续帧和窗口级刷新代码均已移除，生产动画完全由 Base 负责，不增加全窗口重绘循环。减少动态效果的独立回归检查隐藏直接到达零宽度且不请求动画帧。

原生按下／释放及窗口移出事件只更新外观保留，不消费输入、不代替 Base 的捕获或修改滚动偏移；隐藏宽度优先于过期的原生 hover 外观。专项日志保留首次活动策略复验、错误可见性探针、减少运动设置检查，以及拖动释放后等待异步范围回执的夹具修订。此前失败记录不计为验收通过。

## 验证与交付

记录目录为 `target/scrollbar-unification-2026-10-21/`。`baseline/source.zip`、`baseline/inputs.json` 保存开工时 1,000 项宿主源码／文档输入，`baseline/workspace.patch` 和 `baseline/status.txt` 保留既有改动；本轮不将它们重新计作成果。

针对性回归直接读取原生滑块像素几何，检查宽度中间值、最终为零、颜色透明度不变、滚动后反向展开、悬停及真实鼠标拖动；原生 Editor／Textarea 的独立活动与主题恢复；带标题画布的覆盖层定位及内容 epoch 回执。Windows 专项从实际独立 Git 包进入公开管理器，保留原来的连续滚轮、矢量复用及边界顺序验证，并追加真实 D3D11 窗口的轨道拖动、收缩帧和截图。

### 实际结果

| 检查 | 结果与证据 |
| --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过；`fmt-final.log`、`diff-check-final.log`。 |
| `cargo check --workspace --jobs 2` | 通过；`check-final.log`。保留工作区既有的 dead-code／可见性等警告，没有为本轮扩大清理范围。 |
| 组件依赖边界 | `cargo tree --workspace --edges normal,build,dev --all-features` 通过且无组件运行时；`dependency-tree-final.log`。源码中无 `gpui_kit::component`／`gpui_component` 调用，`Cargo.lock` 无 `gpui-component` 及宏库。边界回归中的禁止包名字符串属于断言。 |
| `cargo test --workspace --exclude editor-app --jobs 2` | 83 组，411 通过、0 失败、334 忽略；`nonui-final.log`。迁移后该命令包含 `editor-ui`，其中 79 项通过、10 项忽略。忽略项未计为通过。 |
| 控件专项及完整 `editor-ui` 回归 | `ui-library-final3.log`：79 通过、0 失败、10 忽略，包含两轴收缩、反向展开、轨道悬停、窗口移出、原生拖动、独立输入活动、主题恢复、减少动态效果，以及原有输入／Undo／IME／对齐行为。 |
| 宿主 UI | `host-tests.exe 'ui::' --test-threads=1`：76 通过、0 失败、28 忽略；`host-ui-current.log`。包含画布覆盖层定位和 content epoch 回执。较早一次并行迁移中的表单草稿测试失败，在此最终构建中已通过；较早结果没有覆盖或冒充最终结果。 |
| 对比编辑器 | `host-tests.exe 'editor::comparison::' --test-threads=1`：11 通过；`host-comparison-final.log`。包含一条共享竖向滚动条、换行宽度、原生输入、偏移和资源清理。 |
| 真实 Git 包集成 | 显式 `--exact --ignored` 执行 `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs`、`git_inline_history_preview_opens_readonly_two_and_three_pane_tabs`，各 1 项通过；`git-comparison-final.log`、`git-preview-final.log`。 |
| Windows 原生滚动专项 | 显式 `--exact --ignored` 执行 `extensions::worker::native_acceptance::git_native_history_scroll_probe`：1 项通过，内部深色中文 100%／浅色英文 150% 两例；`native-pixels-final.log` 和 `native-pixels-final/<case>/evidence.json`。 |
| 扩展终端回归 | `host-tests.exe 'terminal::' --test-threads=1`：30 通过、2 失败、2 忽略；`host-terminal-final.log`。失败为 `terminal::tests::lifecycle::shortcuts_and_closing_active_restored_tab_keep_remaining_shell_usable`（PTY 输入队列满／命令漏字）和 `terminal::tests::native_shell_tabs_input_output_clear_and_trust`（等待 Shell 响应超时）。本轮没有修改进程输入投递链路；未认定这两项已修复，也未用滚动条专项代替终端验收。 |

原生两例都观察到 6px 展开、多个中间宽度和最终不可见，alpha 全程为 0.55：深色观测到 `6 → 5 → 4 → 3 → 2 → 1 → 0`，浅色 150% 观测到按物理像素对齐的 `6 → 5.33 → 3.33 → 2.67 → 2 → 1.33 → 0.67 → 0` 逻辑像素。最终不可见同时经过 D3D 像素验证；原生拖动改变公开画布偏移，闲置动画保持偏移和提交选择。两例普通窗口都完成关闭与资源清理，错误日志为空。每例的 `history-scrollbar-visible.png`、`history-scrollbar-contracting.png`、`history-scrollbar-hidden.png` 保存实际帧。

测试宿主通过 `cargo test -p editor-app --release scrollbar --no-run --jobs 2 --config 'profile.release.package.editor-app.opt-level=0' --config 'profile.release.package.editor-ui.opt-level=0'` 构建，再复制为该记录目录的 `host-tests.exe`；控件测试采用相同依赖 profile 的独立测试程序。测试程序用于行为验证，不能当成普通 Release 发行程序。原生专项使用已构建的独立 `git-0.1.22.zip`，SHA256 为 `239a0033145048333fff43528287b1cf74c8dd2cba6ec3af7484cc0e5c30ff25`；没有重新打包或安装插件。

### 候选与输入来源

普通 Release 候选为 `target/scrollbar-unification-2026-10-21/Nanobug.exe`，79,099,392 字节，SHA256 为 `e69e2e1b39eaa4c316d9f9021f63b84b9cb66c6ab530fe7165cc3e1e1779b825`。最终构建命令如下，没有测试程序的 opt-level 覆盖：

```powershell
cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/scrollbar-unification-2026-10-21/Nanobug.exe
```

`release-final.log` 记录构建通过，耗时 2 分 23 秒，保留既有 Rust／Wasmtime 链接警告。`metadata-final.json` 确认 `workspace_root` 为当前 Editor 仓库、输出为其 `target`。`production-inputs-before.json`／`production-inputs-after.json` 对比 Cargo 清单、锁文件和 crates 下共 1,365 项输入；`production-changed-during-build.json` 为空。`production-artifact.json` 保存命令、路径、长度、最终哈希与测试宿主身份。最后一次重建前只移除了三个受影响控件中已不需要的导入，避免把中间构建当成交付候选。

候选来自当前工作区，包含开工前既有改动和用户另行授权的 UI 迁移；源码账本用于说明构建来源，不表示这些改动全由本轮生成。用户既有 `docs/窗口布局拖放规则.excalidraw` 仍为开工时的 SHA256 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6`。

不将以上观察解释为帧率或输入延迟基准；没有逐项手动遍历所有菜单，也没有验收操作系统中文 IME 候选窗。没有替换用户运行中的程序、安装插件、生成安装器、重启、提交、推送或发布。

返回 [UI 目录](../README.md)。
