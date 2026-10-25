# 插件状态与外部服务启动分离验收

日期：2026-10-24。状态：本次功能、针对性回归、规定检查和 Windows 原生交互已验收；未提交、推送或发布。

## 范围与实现

依据[分离规格](../specs/plugin-service-status.md)，右下角原入口只表示插件恢复、语法资源加载和插件日志提醒。外部语言服务及项目语言引擎的准备、初始化在其左侧独立显示“外部服务启动中 N”，点击打开实时启动列表。

- `app/plugins/loading.rs` 从现有就绪状态投影两类加载列表。外部服务按公开声明标识分组：同一声明的多个上下文合并，同一插件的不同声明分别计数；不解析翻译消息，不设置插件或服务器名称白名单。
- `app/plugins.rs` 和 `status_popup.rs` 共用原有 Base 弹窗生命周期、按钮、滚动和焦点能力。服务弹窗不捕获或确认插件提醒，也不自动将插件日志标为已读；明确选择服务条目时可沿已有路由查看所属插件的完整日志。
- 同一插件的多个服务使用独立行键和焦点句柄。原插件弹窗保留异常严重程度、原子确认边界及可见摘要已读规则。
- `app/shell.rs` 将服务入口置于插件入口左侧；当前服务列表变空时，在绘制点击阻断层之前关闭服务弹窗并释放其状态。Ready 或 Failed 不再进入启动列表；取消、撤销沿用既有服务状态清理。
- 中英文文本使用 `editor-ui/locales/app.yml`，绘制使用既有主题、Spinner 和本地按钮。未修改公开协议、SDK 或官方插件，现有插件通过既有服务声明直接适用。

本轮开始时 HEAD 为 `561aa5b2006c38668a083482156646a16cd02b3e`，工作区已有 237 项改动。`target/plugin-service-status-2026-10-24/status-before.txt` 和 `baseline/` 保存任务前状态及相关原文件；这些已有改动不计作本次成果。

## 回归与构建

证据目录为 `target/plugin-service-status-2026-10-24/`。Cargo 复用同一 checkout 已有的独立输出目录 `target/exclusive-drag-replacement-2026-10-24/cargo/`；`metadata-isolated.json` 已核对 `workspace_root` 为当前 Editor，`target_directory` 为该目录。没有构建另一份源码快照或共用其他工作区缓存，复用前保存了原生产程序 `prior-editor-app.exe`。

最初默认 target 被其他构建占用，停止本任务等待链后改用上述输出目录；该等待尝试不计作功能失败。

| 检查 | 实际结果 | 证据文件 |
| --- | --- | --- |
| 分离入口的首项行为回归，改实现前 | 1 失败，断言缺少 `service-starting-indicator` | `red-separate-service-isolated.log` |
| 同一回归，改实现后 | 1 通过、0 失败、0 跳过 | `green-separate-service2.log` |
| `cargo test -p editor-app app::plugins::status_tests --offline --jobs 2 -- --test-threads=1` | 12 通过、0 失败、0 跳过 | `status-regressions.log` |
| `cargo test -p editor-app app::plugins::tests --offline --jobs 2 -- --test-threads=1` | 10 通过、0 失败、0 跳过 | `plugin-regressions.log` |
| `cargo test --workspace --exclude editor-app --offline --jobs 2` | 555 通过、0 失败、402 跳过 | `workspace-tests.log` |
| `cargo check --workspace --offline --jobs 2` | 退出 0 | `workspace-check.log` |
| `cargo build -p editor-app --offline --jobs 2` | 退出 0 | `host-build.log` |
| `cargo fmt --check`、`git diff --check` | 均退出 0 | `fmt-check.log`、`diff-check.log` |

多服务回归通过实际 ZIP Package / Manager 准入和已有发布接缝，操作真正的底栏按钮并检查原生控件标签及边界：插件加载 1 与服务启动 2 同时可见且顺序正确；打开服务列表后，另一插件的提醒及未读状态保留；一个服务就绪后只剩另一个，后者失败后入口和弹窗释放，插件提醒仍可独立确认。原有 12 项状态回归还覆盖并发新记录、卸载后的日志路由、长内容滚动、摘要可见性、Escape 与焦点归还。

初轮实现曾有原始 Div 的角色接口编译错误；多服务断言也曾依赖未进入 Base 控件快照的原始 ListItem。两次均保留失败日志，后续保留原有可交互日志条目、补独立服务行键，并以真实按钮标签重新验证。没有将编译失败或不可观察的语义标记作为成功验收。

402 项跳过包含需要额外包和外部环境的既有测试，未计作通过；本次未修改 WASM 包或 SDK，不将上述常规检查视为所有官方语言插件的完整功能验收。

## Windows 原生交互

实际生产程序：`target/exclusive-drag-replacement-2026-10-24/cargo/debug/editor-app.exe`，SHA-256 为 `b6b82ae44c48b33a66985156bd7e6737279bfd8b1c683f6224aa7e6bb606a027`，构建时间 UTC `2026-10-24T10:31:24.0761545Z`。身份另存 `native-host-identity.json`，开发生命周期报告中的宿主摘要一致。

通过生产 `--plugin-dev` 入口使用独立 profile、私有工作区及公开资源包 `status-startup-fixture 1.0.0`。只授予声明的 `process.service.analysis` 权限，既有 Python 运行时执行受控 stdio 服务；真实 initialize / initialized 握手完成后等待私有 gate 发出就绪通知。没有修改用户已安装插件、常用 profile 或系统显示设置。

```powershell
& $taskExe --plugin-dev "$native/project" --profile "$native/profile" --workspace "$native/service-status-workspace" --grant process.service.analysis --report "$native/report.json"
```

其中 `$taskExe` 是上述已核对的生产程序，`$native` 是当前 checkout 下的 `target/plugin-service-status-2026-10-24/native`。通过 computer-use 对这一个实际 Nanobug 窗口进行点击和键盘操作，浅色中文界面观察到：

1. 打开夹具文档启动真实外部进程。底栏左侧显示“外部服务启动中 1”，右侧显示夹具主动发出的“插件警告 1”；没有“插件加载中”冒充服务启动。
2. 点击服务入口，弹窗显示“正在启动的外部服务”、`status-startup-fixture/analysis`、所属插件和启动内容；插件警告仍保留。
3. Escape 关闭弹窗并将焦点归还服务按钮。点击插件警告打开原插件弹窗，只显示插件摘要，确认其提醒后服务 loading 继续存在。
4. 再打开服务列表，写入私有就绪 gate。实际 stdio 服务发送 `fixture/ready` 后，服务 loading 与弹窗自动消失，UIA 焦点回到文档编辑区。
5. 向本任务开发控制器发送 `stop` 正常退出。`native/report.json` 的 admission、startup、activation、cleanup、status 均为 passed，termination 为 normal；只读检查未发现夹具 Python 进程残留。

`native/wire.log` 实际记录 `initialize`、`initialized`、`ready_sent`、`textDocument/didOpen`。截图与 UIA 文本证据为 `native/services-popup.png`、`services-popup.a11y.txt`、`plugin-popup.a11y.txt`、`ready.png` 和 `ready.a11y.txt`。原生测试中的警告是夹具预设输入，不是启动失败；生命周期报告的 `business: not_exercised` 也不替代上述人工可观察交互记录。

本次自动 UI 回归覆盖浅色英文和深色中文；实际 Windows 点击采用浅色中文。没有单独实测 Rust-analyzer 下载、其他语言生态、系统 IME 候选框或多套 OS DPI 配置，这些不作为本次已验收范围。

2026-10-24 后续复查：用户报告 Rust 已能返回悬浮信息，但服务提示持续加载。上述首次启动检查漏掉了服务就绪后增加文档的重新需求路径；独立真实 stdio 服务也可复现。原因与补充回归见[保留服务实例的就绪状态](service-readiness-retained-adapter-2026-10-24.md)，不能将本记录的首次启动结果外推为多文档状态始终正确。

返回[插件文档目录](../README.md)。
