# 通用画布连续滚动与绘制复用

日期：2026-10-21。状态：本轮滚动行为、实际包及两组原生验收完成，候选已生成；全仓格式检查仍有下述并行文件例外。未安装到用户环境。

## 范围与契约

用户授权先修复滚轮丢失，再优化刷新频率及重复绘制；宿主能力保持通用。原现场是 Release 宿主与独立 Git 0.1.17，运行日志出现同一秒多条 `host.ui.stale / UI document was replaced`。本轮工作区与独立项目已有的 0.1.20 增量刷新等改动保留，不计为本轮新增成果。

`ui.canvas 1.2` 新增可选非零 `Canvas.scroll_epoch` 与 `CanvasEvent::ScrollInput { epoch, change }`。只有仍有效的同一内容可接收旧绘制 revision 的增量／绝对位置；内容替换（包括 A → B → A）必须更换 epoch。未来版本、旧点击／键盘／普通滚轮、弹窗、禁用／删除节点、源绑定动作继续拒绝。实例、权限、工作区与资源生命周期门禁保持现有语义。未启用字段的插件保持现有契约。

通用原生画布投递该事件，内容更换清除待处理滚动条位置；worker 合并连续同方向增量或绝对位置，反向保留顺序以维持边界裁剪。UI 轮询从 33ms 改为 16ms，未变化场景继续复用。矢量源相同、大小和相对裁剪不变时，平移复用原像素；尺寸／裁剪改变重新绘制。

独立 Git 0.1.21 通过该公开能力接入，使用缓存拓扑、真实视口文字和稳定坐标 SVG 分块。空闲滚动只发布历史面板，查询／错误／忙闲变化继续完整发布，保留暂存及比较面板的输入版本。宿主没有 Git ID、提交或分支策略。包版本、能力版本与协议版本分别处理；协议仍为 7，权限不增加。

## 已运行的反馈

- `cargo test -p plugin-protocol scroll_stream_survives_repaint_but_not_content_replacement -- --nocapture` 首次失败于未知 `scroll_epoch`；新增公开契约后通过，并覆盖旧普通滚轮／点击、未来版本、弹窗、禁用及非法数值。
- `cargo test -p editor-app scrolling_vector_tiles -- --nocapture` 先失败于平移重新生成像素，测试本身 0.16 秒；通用复用实现后通过，0.03 秒。这是缓存行为信号，不是原生帧率测量。
- 新宿主通过 `--plugin-cargo ./Cargo.toml test --lib` 执行独立访客 122 项，122 passed、0 failed；第二次日志为独立项目 `target/scroll-unit-second.log`。首轮因移动绘制方法后的内部可见性编译失败，修正为与原来相同的 crate 可见范围；该失败不计作性能缺陷复现。
- 实际包经公开 Manager 接收同一帧捕获的 64 次增量后，448px 全部保留；初次随后失败于暂存面板也被替换，日志 `target/scroll-burst-red.log`。缩小发布范围后，最终包复验通过，同时确认旧点击继续拒绝、禁用后资源归零；`target/scroll-burst-delivery.log`，62.86 秒，包含隔离进程及冷启动，不作为输入延迟。
- 能力下限回归曾失败于 `required_capabilities()` 仍返回 1.1；修正为统一的 1.2 下限后通过，日志 `target/scroll-capability-floor-red.log` 与 `target/scroll-capability-floor-green.log`。沿用现有通用能力检查，没有重复增加专属门禁。

## 最终验证与交付

使用独立 Cargo 输出目录 `target/scroll-fix-2026-10-20` 与独立访客 `target/scroll-fix-2026-10-21`，均已通过 metadata 核对 workspace root／target directory。构建入口为 Cargo、宿主 `--plugin-cargo` 和 `--plugin-package`；未调用归档的 scripts。

- `cargo test --workspace --exclude editor-app --jobs 2` 最终通过：82 组、332 passed、324 ignored、0 failed；`target/scroll-nonui-final.log`。ignored 未计作通过；本轮的实际包及原生测试另行显式执行。
- `cargo check --workspace --jobs 2` 通过；`target/scroll-check-final.log`。配套开发宿主构建通过；`target/scroll-host-final.log`。已有编译和链接告警保留，未升级依赖。
- `cargo test -p editor-app ui::plugin:: -- --nocapture`：69 passed、17 ignored、0 failed；`target/scroll-ui-tests.log`，包含实际位移复用及裁剪变化重新绘制的检查。
- 最终独立访客 122/122 通过，格式检查通过；独立项目 `target/scroll-unit-formatted.log`、`target/scroll-guest-format-final.log`。最终 `--plugin-package` 构建通过，日志 `target/scroll-package-formatted.log`。
- `cargo fmt --check` 首次在本次文件上修正后通过；交付前因并行新增的 `crates/editor-app/src/ui/controls/aligned_editor/tests/performance.rs` 格式差异失败，完整输出 `target/scroll-format-delivery.log`。该文件不属于本任务，未代改。本次 21 个 Rust 文件单独 `rustfmt --check --config skip_children=true` 通过，`target/scroll-scoped-format.log`；`git diff --check` 通过，`target/scroll-diff-check.log`。

原生回归显式构建并执行 `extensions::worker::native_acceptance::git_native_history_scroll_probe`（`--ignored --exact --nocapture --test-threads=1`），设置 `NANOBUG_GIT_CANDIDATE`、`NANOBUG_GIT_NATIVE_OUTPUT`、`NANOBUG_GIT_NATIVE_HISTORY_ONLY=1`、`NANOBUG_GIT_NATIVE_SCROLL_ONLY=1`。最后一次直接运行同一 test binary，日志 `target/scroll-native-delivery.log`：两组均通过，116.84 秒，均读回最终 ZIP 相同 SHA-256 并完成私有窗口／项目清理。

| 原生组合 | 32 次滚轮实际位移 | 输入到发布样本 | 平移复用图像 | 边界反向后位置 | 选择／清理 |
| --- | --- | --- | --- | --- | --- |
| 深色、中文、100% | 160px | 19ms | 1 | 25px | 保留／完成 |
| 浅色、英文、150% | 160px | 22ms | 1 | 25px | 保留／完成 |

证据为 `target/canvas-scroll-final-2026-10-21/<case>/evidence.json` 和 `history-scroll.png`，已人工检查深浅两组像素呈现。时间是这两次突发输入到宿主发布的观测样本，不能推导持续 FPS；WPR 在分析阶段因系统性能采样权限失败，未获取 CPU/GPU 分项。

旧原生夹具先后仍等待已移除的刷新按钮、将提交选择视为独立详情窗口流程，未触达滚动断言；窄探针初版还假设画布具有命名快照节点。相关失败保留在 `target/scroll-native.log`、`scroll-native-second.log`、`scroll-native-range-final.log`，不作为滚轮缺陷复现。修正通用就绪观察后，新滚动探针使用实际树／停靠几何，覆盖当前内嵌预览，不放宽位移、反向顺序或像素复用断言。保存的通用 Harness `workflow` 标签仍沿用 `history-keyboard-only`；本次范围以 `history_scroll` 和 `diagnostic_only=true` 为准，**没有把它计作键盘、IME 或完整 Git 工作台验收**。

## 候选身份

- 开发宿主：`C:/Projects/RustProjects/Editor/target/scroll-fix-2026-10-20/debug/editor-app.exe`，153135104 bytes，SHA-256 `ac420773dee3e66f67d024a6874c3fc65a78c5d9e873614502a3da3a355e7f21`。这是配套开发构建，不是 Release 安装器。
- Git ZIP：`C:/Projects/RustProjects/Git-for-Nanobug/target/scroll-candidate/git-0.1.21.zip`，869726 bytes，SHA-256 `59d546219f7d11f7c188e4247a1cf09e90070f67f5b3273e9aa6cd5aadf35aae`。清单要求 `ui.canvas >=1.2,<2`；需要配套新版宿主。
- 独立源码：同目录 `git-for-Nanobug-0.1.21-source.zip`，197070 bytes，SHA-256 `cb164c1388a7cc06da7eb7cb7fa45e1fada5c08e1a3ca43b82e8741ca7e0d4f2`；68 文件逐项 SHA-256 与当前源码一致，不含 target、dist、SDK 副本或上游源码。

未修改用户已安装包，未重启用户程序、提交、推送或发布版本。

返回[插件目录](../README.md)。
