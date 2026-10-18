# Git 独立工作窗口标题栏点击回归

用户在提交详情已显示提交文件和双栏比较后，点击最小化、最大化、关闭三个按钮均无反应或卡死。该反馈与周期刷新修复分开记录，独立 Git 插件当前交付候选为 0.1.8。

## 验证缺口

此前 `WM_CLOSE`／`WM_SYSCOMMAND(SC_CLOSE)` 回归只验证关闭请求、关闭门禁及资源退役，没有覆盖 Windows 原生鼠标在标题栏按钮上的命中与按下/释放路径；不能用那些通过记录宣称这三个按钮正常。

当前公开依赖仍为 gpui-kit 0.7.0、gpui-pre-windows 0.3.7，根工作区和任务源码快照一致。工作区已有大量其他任务改动，通用窗口控件也有既有外观变更，均保留，不归为本次成果。

## 本次反馈循环

准备两层真实 Windows 回归：不加载 WASM 的通用工作窗口分别验证三个按钮；真实 Package／Worker 的已加载提交详情窗口再验证同一鼠标路径、内容保留与主窗口响应。通过系统鼠标输入进入非客户区命中/点击流程，使用窗口状态与正常退役作为结果，不以直接发送最小化/关闭命令代替点击。

每个原生 child 都由父进程限制等待时间，只有私有 profile、临时仓库和自身 HWND；不修改用户测试项目或终止用户程序。

## RED 与原因

- 冻结未修复 runner：`target/git-caption-mouse-red-runner-2026-10-18/editor_app.exe`，SHA-256 `197635a6c6de0580367cc4e20658fb7c3368032542798c7f85b505a7a65c5965`。
- 通用最小窗口的三张独立 child 均在实际 OS 移动后发现 `WM_NCHITTEST=HTCAPTION(2)`；最小化、最大化、关闭预期分别为 8、9、20。总计 3.14 秒失败，窗口仍能处理旧关闭请求并正常退役，未触发 12 秒超时。
- 真实 Git 0.1.8 包的提交详情已加载同一文件的非空不同父/结果文本；截图核对最小化中心 `(1188,14)` 与 helper 的逻辑坐标一致，viewport、图片均为 1273×796、scale=1。实际鼠标位置同步与自身 HWND 门禁通过，但仍命中 2、预期 8，64.34 秒失败。
- 两类 RED 的命中门禁均阻止了随后的 SendInput。因此它们证明按钮区域被错误识别，不能写成实际按下后整窗卡死；已加载场景也没有执行后续最大化、还原和关闭。

原因是 `WorkWindow` 将完整标题行登记为 Drag，覆盖其子按钮。锁定版本 GPUI 的 `window_control_hitboxes` 按登记顺序取第一个命中；父级先登记，按钮的原生动作区域无法生效。主窗口标题拖动区本来就与按钮分离。

修复仅将该 Drag 区域移到标题文字的 `flex_1`、满高容器，按钮仍使用本地控件及上游公开 `window_control_area`；不改原生关闭门禁、Git 策略、协议、依赖版本或上游源码。标题一侧保持满高垂直居中。`ui/controls/window.rs` 中此前已有外观修改不属于本次修复。

## 支架前提与 Generic GREEN

首个修复版 runner `4e0385…` 的三个 child 均在“必须是前台窗口”前提失败，没有执行命中或点击，不能判断产品改动。后续 `19b431…` 已完成标题真实拖动、最小化命中 8 和真实点击后 `IsIconic=true`；另两张 child 的坐标被外部前台遮挡，安全门禁阻止输入，没有冒充按钮失败或通过。两次中间记录均保留，文件目录中的 green/final 字样不代表通过。

最终支架只对经 PID 校验的私有窗口使用 `SetWindowPos(HWND_TOPMOST, SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE)`，保证测试目标可见。该前置步骤不移动或缩放窗口，不能满足拖动回执；不调整其他应用。前台归属只记录为布尔值，保留鼠标同步、自有窗口 `WindowFromPoint`、正确 `WM_NCHITTEST`、完整 SendInput 数量及原生状态硬门禁。测试结束销毁自己的窗口撤销层级，父/child guard 释放残余左键并还原光标；没有纯重试或放开遮挡检查。

最终冻结 runner：`target/git-caption-final2-runner-2026-10-18/editor_app.exe`，SHA-256 `c988b6a64143e8c9c2765a50447247e19f853ddc6ee695a36e119360ff0f24ff`，匹配 PDB 保留。

通用三张独立 child **3/3 GREEN，3.28 秒**，日志 `target/git-caption-mouse-final2-2026-10-18.log` 和同名目录下的 `minimize.log`、`maximize.log`、`close.log`：

- 每张 child 均先命中标题 Drag=2，通过真实 down／relative move／up 移动自身窗口，矩形位置改变、宽高不变，始终正常状态。
- 最小化命中 8 → 真实 down/up → `alive=true, iconic=true, zoomed=false`；仅为后续清理使用自身 `SW_RESTORE`。
- 最大化命中 9 → 真实 down/up → `IsZoomed=true`；再实际点击还原，恢复正常状态。
- 关闭命中 20 → 真实 down/up → 原 HWND 不再存在；此项不使用 WM_CLOSE 代替点击。
- 三张 child 均正常退出、父窗继续响应、全部自有 HWND 退役，无 12 秒超时。

## 已加载提交详情 GREEN

同一最终 runner 和独立 Git 0.1.8 ZIP 的 caption-only 原生父用例 **1/1 GREEN，两种组合均通过，138.02 秒**，没有重试。日志 `target/git-caption-loaded-final2-2026-10-18/parent.log`，每种组合下保留 `native.log`、`evidence.json` 与前后 PNG：

| 组合 | child 耗时 | 实际绘制尺寸与 scale | 单次关闭后刷新回执 | evidence SHA-256 |
| --- | --- | --- | --- | --- |
| 深色／中文／100% | 67.10 秒 | 1273×796，1.0 | 101 → 102 busy → 114 enabled/idle | `fa945b885d232a674405a75f7fbce40892be3f4db2abd50cc5cd3a3740e6b628` |
| 浅色／英文／150% | 70.65 秒 | 1920×1200，1.5 | 100 → 101 busy → 113 enabled/idle | `43c8dcc93d08bb8513dcb25c93e05362785d96134472a1e1a961bcc15d465997` |

两种组合均完成最小化、最大化、还原、关闭四次真正的系统鼠标点击：分别命中 8／9／9／20，SendInput 完整入队，原生状态依次 iconic／zoomed／normal／dead。最小化后恢复仅为自身 HWND 的 `SW_RESTORE`，不冒充第五次按钮点击。每次恢复核对同一窗口、精确提交及两份真实只读字节；不重建来源或放宽文档权限。

关闭详情后，主窗口既有按钮路径只发一次新刷新，观察比原 revision 更晚的 disabled/真实进程 busy，再等更晚 revision 回到 enabled/idle，不以旧空闲界面或无变化 timer 冒充完成。互审修复了旧断言的误报缺口；忙碌瞬态轮询仍可能导致假红，不会导致假绿，本次两种组合均实际捕获该状态。所有 `unexpected_error_logs=[]`，`shutdown_complete=true`，最终 GPUI 窗口与原生 HWND 都为空。四张 D3D11 前后实际截图均已逐张查看，标题栏、比较布局、来源和内容无新增视觉问题。

## 其他检查

修复后的隔离源码 `cargo fmt --check`、`cargo check --workspace` 已通过；非 UI 工作区测试 77 组、278 passed、0 failed、302 ignored，ignored 不计通过。根格式检查仍仅在此前已有的 `extensions/preview/toolbar.rs:65`、`extensions/surface.rs:86` 两处失败，未扩大修改。

窗口关闭/退出常规测试 2/2、焦点测试 3/3；另显式运行真实 0.1.8 ZIP 的五项 ignored 窗口集成，5/5 通过（418.72 秒），覆盖比较刷新、同步滚动、未保存来源/标签、合并审查及隐藏草稿保留。日志分别为 `target/git-caption-editor-windows-tests.log`、`target/git-caption-editor-focus-tests.log`、`target/git-caption-editor-window-package-tests.log`。

最终任务源码快照与根仓库的十个产品/测试/清单输入摘要一致；`Cargo.lock` 未改变依赖版本。最终编译检查日志为 `target/git-caption-final-workspace-check.log`，格式检查为 `target/git-caption-final-canonical-fmt.log`，非 UI 测试为 `target/git-caption-nonui-workspace.log`，根既存格式偏差为 `target/git-caption-root-fmt.log`。产品和测试交叉审查未解决项为零。

## 本地交付与重现

便携包 `target/git-caption-fix-release-2026-10-18/Nanobug-caption-fix-2026-10-18-x64.zip`，32,737,891 bytes，SHA-256 `264e5d5cab1fc047089e6d2de7db6346c331136d2be69ef04aaa0256f136f289`。逐个解读 ZIP entry 并核对摘要：仅包含 `Nanobug.exe`、品牌 ICO、Alacritty 许可和使用说明，不携带插件、SDK、编译器或上游副本。

其中宿主 EXE 为 117,529,600 bytes，SHA-256 `d43f2c19d15ec9dea8798989e7a0d69a1badc787114a9ce8730f1db01636bdd6`；Windows ProductName=Nanobug、FileVersion=0.1.0、PE32+ x64、8 MiB stack reserve、133,912 bytes 品牌资源均已核验，匹配 PDB 另存。直接 Cargo release 构建 4 分 49 秒；临时独立 bin alias 只用于避免覆盖运行入口，构建后 Cargo/build.rs 已恢复到之前摘要，根构建入口没有修改。CLI `--export-plugin-sdk` exit 0，50 文件、0 Markdown，校验输出放在运行载荷之外。

独立 Git 仍为此前的 `target/git-auto-refresh-fix-2026-10-18/git-0.1.8.zip`，SHA-256 `5c6c483fd60b9fc02277aadfe800408fbc324eab1b99ffac7e1ddf0acb9b3292`，本次没有改包或提升其版本。标题栏需要使用新宿主，周期刷新修复由单独安装的 0.1.8 提供。便携包使用说明明确先退出旧进程再解压运行，避免单实例路由仍交给旧程序；没有自动安装、覆盖用户配置或终止用户程序。

```powershell
$env:NANOBUG_CAPTION_MOUSE_OUTPUT = 'C:/Projects/RustProjects/Editor/target/git-caption-mouse-recheck'
& ./target/git-caption-final2-runner-2026-10-18/editor_app.exe --exact extensions::worker::native_close_tests::caption_mouse::native_caption_mouse_controls_remain_responsive --ignored --nocapture --test-threads=1
$env:NANOBUG_GIT_CANDIDATE = 'C:/Projects/RustProjects/Editor/target/git-auto-refresh-fix-2026-10-18/git-0.1.8.zip'
$env:NANOBUG_GIT_NATIVE_CAPTION_CONTROLS = '1'
$env:NANOBUG_GIT_NATIVE_OUTPUT = 'C:/Projects/RustProjects/Editor/target/git-caption-loaded-recheck'
& ./target/git-caption-final2-runner-2026-10-18/editor_app.exe --exact extensions::worker::native_acceptance::caption_controls::git_native_loaded_commit_caption_controls --ignored --nocapture --test-threads=1
```

原生用例顺序执行；运行前清除无关的 `NANOBUG_GIT_NATIVE_HISTORY_*`、`NANOBUG_GIT_NATIVE_IDLE_RETENTION`、`NANOBUG_GIT_NATIVE_SCENE_PROBE` 等诊断标志。结构化摘要、逐文件 hash 和本地交付身份见便携包同目录的 `verification.json`，ZIP 校验另附 `.sha256`。

本次没有重跑完整工作台、八组合或系统 IME 候选窗口验收，不继承那些场景的通过结论。没有修改用户测试项目的分支、暂存区、合并或变基现场，没有提交、推送、关闭议题或发布在线版本。

返回[插件文档目录](../README.md)。
