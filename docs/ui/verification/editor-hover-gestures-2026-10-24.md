# Hover 与边框拖动的交互边界

日期：2026-10-24。范围为编辑器的定义／类型说明与诊断弹框；保留 Base `EditorState`、现有 LSP 提供者、主题与本地卡片绘制。

## 复现与原因

通过生产 `EditorApp`、实际停靠分隔条和 GPUI 鼠标事件复现，未调用私有鼠标处理函数绕过路由：

- `hover_dock_resize_does_not_show_diagnostic`：断言分隔条确实移动，再断言诊断卡片不应出现。修复前失败，测试执行约 0.18 秒。
- `hover_splitter_press_retires_pending_details`：先通过真实悬浮发起请求，再按住分隔条，最后放行延迟响应。修复前失败，测试执行约 0.19 秒。

第一条路径中，Base 已在移动事件上写入诊断状态，宿主看到鼠标按钮按下却直接返回，没有撤销误写入的状态。第二条路径中，分隔条属于编辑区的兄弟控件，按下事件没有取消此前的宿主 Hover 请求，晚到结果仍通过 generation/revision 检查并显示。

仅在移动时判断 GPUI drag payload 能修复第一条路径，但不足以修复第二条。因此同时覆盖按下、拖动与异步返回的生命周期。

## 实现与参考

参考 Zed 的 [hover_popover.rs](https://github.com/zed-industries/zed/blob/main/crates/editor/src/hover_popover.rs) 对卡片实际边界、独立交互和状态生命周期的组织，以及 [element/mouse.rs](https://github.com/zed-industries/zed/blob/main/crates/editor/src/element/mouse.rs) 对实际文字命中的判断。仅借鉴交互职责，没有复制其源码或引入 Zed 编辑器、`gpui-component`。

- 在源编辑器挂载时注册窗口范围的鼠标按下观察，因此停靠分隔条、工具区等兄弟控件也能使旧请求失效。
- 弹框外按下关闭说明，清理 Base Hover/诊断状态、宿主缓存、待处理 generation 以及延迟焦点恢复。
- 按键拖动或 GPUI 拖动期间，不允许 Base 临时生成的诊断或 Hover 留在显示状态。
- 弹框内开始的文本选择与边缘缩放保留到松手，即使指针随后离开弹框。
- 本地 `DefinitionPopup` 在最终定位后报告完整外框边界；定义框与诊断框共用该接口。边界不从带内边距的 Markdown 子控件估计，包含可点击空白和缩放边缘。
- 普通悬浮延时、键盘打开、Escape、滚动命中、主题、复制与现有定位规则保持原有行为。

## 验证入口

```powershell
cargo test -p editor-app hover_ -- --nocapture
$env:NANOBUG_HOVER_OUTPUT = "$PWD/target/hover-native"
cargo test -p editor-app hover_native_windows_acceptance -- --ignored --nocapture --test-threads=1
Remove-Item Env:NANOBUG_HOVER_OUTPUT
cargo fmt --check
cargo test --workspace --exclude editor-app -j 1
cargo check --workspace -j 1
cargo build -p editor-app --bin editor-app --release -j 1
git diff --check
```

原生验收使用隔离临时工作区和 profile，在独立进程打开生产 `EditorApp` 的 Windows/D3D 窗口，通过 `Window::dispatch_event` 发送普通鼠标事件，检查实际文本区几何、卡片外框及状态，并保存 D3D 截图。它没有使用 TestPlatform 或全局鼠标注入，也没有启动用户安装的语言服务器。类型与错误内容通过公开 HoverProvider／诊断接口供给。

## 实际结果

| 检查 | 结果 | 日志（`target/`） |
| --- | --- | --- |
| Hover 回归 | 15 passed、0 failed、2 ignored；包括分隔条静止悬停、真实拖动、松手后重新悬浮、晚到响应、滚动定位、键盘、复制与弹框缩放 | `hover-final-regressions.log` |
| Windows/D3D 原生验收 | 父入口 1 passed，子进程 1 passed；深浅主题、类型说明与 Error 级别诊断，分隔条实际位移后等待 1.2 秒均无弹框 | `hover-native-final-run.log`、`hover-native/native.log` |
| `cargo fmt --check` | 通过 | `hover-fmt.log` |
| workspace 测试（不含 editor-app） | 551 passed、0 failed、401 ignored，110 组含空 doctest | `hover-workspace-tests.log` |
| `cargo check --workspace -j 1` | 通过；保留已有警告 | `hover-workspace-check.log` |
| `git diff --check` | 通过 | 本轮命令输出 |

原生图片已人工查看，150% 缩放下错误框、文字与分栏正常，拖动截图中卡片已关闭。图片位于 `target/hover-native/{light,dark}-{definition,diagnostic}{,-drag}.png`。默认跳过的两个原生入口已单独显式运行，不计入普通 15 项通过数。

最后一次 `cargo test -p editor-app hover_` 成功后，本次范围外的工作区文件 `extensions/worker/native_acceptance/scenario/history/scroll/columns.rs:206` 出现 `windows::Win32` 导入解析错误，导致紧接着的 Cargo 原生验收命令重新编译失败（`hover-native-run.log`）。本任务没有改动该文件。Error 级别复验使用刚由前一条成功 Cargo 命令生成的 `target/debug/deps/editor_app-29c966e531ac687b.exe`，显式运行同一 `hover_native_windows_acceptance`，父子进程均成功；该二进制包含本次最终 Hover 源码与 Error 级别夹具。此记录区分已验证源码与随后发生的无关测试编译问题，不声称最后一次 Cargo 原生命令通过。

## Release 产物

`cargo build -p editor-app --bin editor-app --release -j 1` 完成源码编译和链接，但最后复制到 `target/release/editor-app.exe` 时失败：现有进程 PID 38476 正在使用该文件，Windows 返回拒绝访问。保留用户运行中的会话，没有终止进程或替换其程序。

已把该次链接产物 `target/release/deps/editor_app.exe` 复制到 `target/hover-release/editor-app.exe`，两份 SHA-256 一致：

```text
5F7C2F23E805C4A0E609ECCEE22043E19F56F62D5240CA9E90C923702A89DD5F
```

新版大小为 86,766,592 字节，链接产物时间为 2026-10-24 16:43:36（Asia/Shanghai）。用户保存工作后可关闭旧程序并运行此独立产物。Cargo 命令退出码为 101，原因是最后的占用文件替换，不能记为完整构建命令成功；新产物已生成并校验复制完整性。原生行为验收使用上述 Debug 测试程序，没有把 Release 产物哈希校验描述成原生运行验收。

未提交或推送，本记录不把工作区其他改动算作本次成果。

返回[原生 UI 文档目录](../README.md)。
