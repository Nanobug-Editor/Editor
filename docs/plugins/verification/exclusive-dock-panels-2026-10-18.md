# 内部窗口单独占据验证

日期：2026-10-18。状态：实现、专项验收及规定工作区门禁通过；扩展消息窗口回归保留 4 项失败。

需求与范围见[规格](../specs/exclusive-dock-panels.md)。实现采用公开 `PanelView.visible` 接缝过滤被替换窗口；Base 保留唯一布局、分割尺寸与生命周期，原窗口显隐意图独立保存。宿主增加通用 `Panel.exclusive` 与 `ui.panels 1.2`，没有插件 ID 特判，也没有改动独立 Git 插件或用户安装。

本轮开始时工作区已有 Git 刷新、构建缓存及其他文档改动，均保留，不算本任务成果。未提交、推送、发布或关闭工单。

## 验证入口

- 能力默认值、序列化省略、版本下限及非法位置：`cargo test -p plugin-runtime --lib capabilities::visibility_tests`。
- 原生布局绘制与指针交互：`cargo test -p editor-app exclusive_ -- --nocapture --test-threads=1`。
- 既有 Dock、侧栏恢复、可见性、窗口按钮相关回归，以及仓库规定的格式、非 UI workspace 测试与 workspace 编译检查。

## 专项结果

- `cargo test -p plugin-runtime --lib capabilities::visibility_tests`：3 passed，覆盖省略属性仍为 false、序列化不增加默认字段、1.2 下限、非法 editor/window 位置及与隐藏窗口入口组合。
- `cargo test -p editor-app exclusive_ --jobs 1 -- --nocapture --test-threads=1`：3 passed、1 ignored，2.72 秒。三个实际布局场景分别覆盖左、右、底部，含同区两个独占窗口、普通入口真实指针切换、原尺寸恢复、可见性采样、禁用撤销、重建窗口实体后的会话选择、旧布局缺失新窗口节点，以及移动至底部后的实际区域判定。日志 `target/exclusive-ui-tests-final.log`。ignored 的 Windows 专项另行显式执行，不计入这 3 项。
- 首轮借用冲突 RED：大纲可见性读取父 EditorApp，与布局采样时的父更新冲突。改为投影父会话的只读显隐值，未引入第二份文本或布局。
- 尺寸恢复 RED：第二个独占窗口延迟插入时，Base 会采用第一个独占窗口占满区域后的测量。改为在首个独占帧前注册保留叶；旧会话载入后也补齐缺失的隐藏独占叶。原尺寸断言保留，随后通过。

## 实际 Windows 窗口

通过已有生产 Worker／公共 Manager 安装隔离测试包，再以 WindowsPlatform/D3D11 绘制和生产 `Window.dispatch_event` 指针路径操作底部按钮。没有使用 TestAppContext 代替本专项，也没有改动用户安装。

测试 ZIP 从本地独立 Git 0.1.9 候选派生，仅将清单暂存面板设为 `exclusive:true`、能力下限设为 `>=1.2,<2`，测试版本为 `0.1.10-exclusive-fixture.1`。未变更或重编 WASM，未作为新插件版本交付；这是新增清单属性与旧组件共用既有 UI 回调的真实包验证。ZIP 位于 `target/exclusive-dock-native-fixture/git-exclusive-fixture.zip`，SHA-256 `4599061b19ac070571dd1531e3feb48962a409347f419c274e878faa3c57b461`。

首轮显式运行 `git_native_exclusive_dock_replaces_and_restores_peers --ignored`：父测试 1 passed、137.32 秒；深色中文 100% 与浅色英文 150% 两个独立原生进程分别通过，全部原始 HWND 退役，`shutdown_complete:true`，插件错误日志为空。证据位于 `target/exclusive-dock-native/{dark-zh-CN-100,light-en-150}`，父日志 `target/exclusive-native-tests.log`。逐图检查两组各 open/shared/reopened 共六张 PNG：独占侧栏只绘制暂存提交，组合显示只绘制资源管理器和大纲，底部 Git 窗口及编辑区保留。全部控件处于真实 clip 内。

补齐旧会话和实际位置回归后，冻结最终 runner `target/exclusive-dock-final-runner/editor_app.exe`，SHA-256 `2df73f65309a0aebe18e9a78a47bf0e6c361790319141ec9b19be3bd46ecc71f`。显式原生复验父测试 1 passed、144.38 秒，同样两组全部通过，均读回正确 workflow、ZIP 摘要、`exclusive_complete:true`、`shutdown_complete:true` 与空错误列表。证据 `target/exclusive-dock-native-final/{dark-zh-CN-100,light-en-150}`，父日志 `target/exclusive-native-tests-final.log`。最终深色组的 open/shared 两张 PNG 再次逐图核对通过。

原生专项不覆盖 OS IME 候选、完整 Git 操作、全部八种主题／语言／DPI 组合或原生重启；新窗口会话恢复由上述 GPUI 集成回归验证。

## 工作区及扩展回归

定向既有回归：Dock 集成 2、LocalDock 5、会话 8、可见性 2、窗口焦点 3、大纲模型 1 项通过；工具实际包 2 项及额外大纲场景 1 项 ignored，未宣称通过。

消息面板扩展回归为 6 passed／4 failed，分别涉及确认清空、运行拒绝提示、旧布局键盘及持久化重开；单项 Cargo 重复仍失败。已定位持久化场景的独立路径不一致：EditorApp 保存的 workspace 为 Windows canonical `\\?\C:\...`，测试却以 tempfile 的未规范化路径调用 `SessionState::load`，严格 workspace 比较拒绝该记录；实际落盘 `messages_visible:true` 且保存成功。其他三项仍不归因。本轮没有改变消息历史策略、放宽路径校验或弱化测试，不能宣称这组扩展回归通过。诊断日志 `target/exclusive-host-message-investigation.log`、`exclusive-message-diagnostic2.log`、`exclusive-host-messages-isolated.log` 保留，临时产品诊断打印已移除。

首轮 `cargo test --workspace --exclude editor-app`：291 passed、305 ignored，exit0。后续重复时并发 rustc 报 Windows 页面文件不足（1455，rlib mmap 失败），没有作为源码错误处理；改用 `--jobs 1` 串行复跑。中途格式检查曾遇到同期新增的 `extensions.rs` branch_menu_tests 模块声明、`plugin-runtime/tests/git_plugin/workbench.rs` 模块顺序差异，本轮未顺手格式化其他任务的改动。

最终门禁均已实际完成：

| 检查 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test --workspace --exclude editor-app --jobs 1` | 78 组、291 passed、0 failed、306 ignored；编译阶段 9 分 50 秒，exit0 | `target/exclusive-nonui-tests-serial.log` |
| `cargo check --workspace --jobs 1` | exit0；7 分 57 秒含等待 Cargo 目录锁 | `target/exclusive-workspace-check-final.log` |
| `cargo fmt --check` | 最终重新执行 exit0；同期格式差异已消失 | `target/exclusive-format-check-delivery.log` |
| `git diff --check` | exit0；新增规格与验证记录的本地链接检查通过 | `target/exclusive-diff-check-delivery.log` |

306 项 ignored 没有执行，不计为通过；Windows 独占专项是前述另行显式执行的真实结果。现有 unused 与 Wasmtime／Tree-sitter LNK4217 警告保留在日志中。没有执行发布构建或生成新的宿主交付安装包；本任务交付当前工作区源码、公开属性及验收记录。

返回[插件文档目录](../README.md)。
