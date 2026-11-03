# 宿主与官方插件统一发行验收

日期：2026-11-03。用户授权合并全部现有修改、普通提交并推送 GitHub，发布宿主和全部官方插件。13 个 Release 已公开，发行前构建、针对性验证、安装生命周期和附件摘要检查完成；保留下述 UIA 并发关闭异常及未覆盖范围。

## 集成与来源

宿主发行源码为 `5f6f58851db14815ef5cb585c6d335953318c0e6`，已合入并推送 `main`。该提交整合安装根目录存储、插件安装与启用、文件加载与语言结果保护、Markdown 长文档与双向滚动、GitHub 宿主更新及原生终端输入修复。`codex/editor-load-flicker`、`codex/plugin-message-routing`、`codex/host-updater-20261102`、`codex/markdown-scroll-fix` 均已被主线包含。已有工作区及分支保留，未重写历史。

宿主标签固定指向实际编译的上述源码；本验收文档是后续独立文档提交，不改变二进制来源。12 个插件使用各仓库 `main` 的普通提交，均已推送并读回远程 SHA。JavaScript、TypeScript、Vue 包含此前待发行的行为修改；其余插件本轮仅提升发行版本、使用现行公开入口重新打包，没有声称新增业务能力。

| 仓库 / 包 ID | 版本与发行入口 | 源码提交 | ZIP SHA-256 |
| --- | --- | --- | --- |
| Git / `git` | [0.1.55](https://github.com/Nanobug-Editor/Git/releases/tag/v0.1.55) | `ef8b3739e00ad2536f5277fffb0dbb4030585d37` | `e364292193541f28a38a5c9bd871157ded2306bc3af2838e7196e9efe7b3be6d` |
| HTML / `html` | [0.3.3](https://github.com/Nanobug-Editor/HTML/releases/tag/v0.3.3) | `fc2292a2040678ea420afae813bc8f12f2e53b0a` | `81c6d3f63ec661c8f4e27dda078885226950aac9753c9829e046affe3ebf5a3f` |
| Image / `svg` | [0.6.4](https://github.com/Nanobug-Editor/Image/releases/tag/v0.6.4) | `8e7859feb3c1edca16c1c6b64ac96d61b874732d` | `d34ef97165afb81333ed56581c19f3c3fb810676b2ca0be38c0da4b2468faf62` |
| Java / `java` | [0.1.36](https://github.com/Nanobug-Editor/Java/releases/tag/v0.1.36) | `503dfa46895300f6e7d7aa28b74f5e1666515b83` | `3fa63626f3f6d6cbe29d7c8e12e6611f44cb6104a8007d084e5f56af0881e55c` |
| JavaScript / `javascript` | [0.4.0](https://github.com/Nanobug-Editor/JavaScript/releases/tag/v0.4.0) | `430a0600159a97f356068d9f4da2297536dfc78e` | `1aaaac101b326ff4d77e77bb7e5a817d6ca018c6a364116a54ec006167b44d46` |
| Markdown / `markdown` | [0.18.8](https://github.com/Nanobug-Editor/Markdown/releases/tag/v0.18.8) | `f0fbb1415e26989d2235d68cb1df07e80fd2c8c2` | `9325a0f9c400e77f9cec3044c8e36fb1669744e6ca64b6fe61f023fd50d9b384` |
| Maven / `maven` | [0.1.20](https://github.com/Nanobug-Editor/Maven/releases/tag/v0.1.20) | `c4e08e4eca2a45fcbf4dfa2d86013c4121cc0b5c` | `8a9e31e9f6f0c8ff51aee00d46e0f5b5b6f17b7517d59f08e850415c371e314d` |
| Rust / `rust` | [0.6.5](https://github.com/Nanobug-Editor/Rust/releases/tag/v0.6.5) | `33b7ec569e53e6573a68f8fcf4e5b172ed3be813` | `b5e4f2fead33af0a11a1e9f8cbe6cb1b5c6a963ebbfed7a0e2af1f1747ce8fe7` |
| TOML / `toml` | [0.2.5](https://github.com/Nanobug-Editor/TOML/releases/tag/v0.2.5) | `55034f72267dd5605a3a2db8b7bd7cb71663edee` | `5d041fd9e431ba402685e69f869d18ef4578a312b9f89a57f3b0fdbd5b7c86d0` |
| TypeScript / `typescript` | [0.1.19](https://github.com/Nanobug-Editor/TypeScript/releases/tag/v0.1.19) | `a32df805030a8ed3efb332feb3e4af7dcc976e9b` | `9ea80e0850137b6ac0c7287ec47917b69da6dfb5d8f38a4e3eabca3994f0892d` |
| Vue / `vue` | [0.2.0](https://github.com/Nanobug-Editor/Vue/releases/tag/v0.2.0) | `0061aeb3e77b7bc0f31e0562d74a7dd288fda272` | `6f6a59b0e7b1d8ca2ab647b4e8bf32ef3e0c3b48415384c8bebdbc8c21c53b47` |
| XML / `xml` | [0.2.4](https://github.com/Nanobug-Editor/XML/releases/tag/v0.2.4) | `6d16fac67b0de969925d90c2c41c159e89def68d` | `a383f8b9e1b47d14f4afbcb3a88d9f9df6dcd1987b316bdb920d903880f7b2cf` |

## 构建与产物

宿主为 [v0.1.0-internal.20261103](https://github.com/Nanobug-Editor/Editor/releases/tag/v0.1.0-internal.20261103)，Windows x64 未签名内测版，发布为 Pre-release。每个插件提供 ZIP 与 `.sha256`；宿主提供安装器、便携 ZIP、两份 `.sha256` 和 `build-info.json`，总计 13 个 Release、29 个附件。旧标签与同版本附件未覆盖。

| 宿主产物 | SHA-256 |
| --- | --- |
| `Nanobug.exe` | `814e7036359ba7b52d709ba52ced1e2305fadbf7a7c8d86d58894a3e524e724e` |
| `Nanobug-Setup-0.1.0-internal.20261103-x64.exe` | `24a7da5e4772d803135dcb010ab3488562c046a35cf110898aab68254751ad7c` |
| `Nanobug-0.1.0-internal.20261103-windows-x64.zip` | `041b633027a659dbdb027205f4f5b0ef0be9a33e695efba8cd0c2c2342787a5c` |

构建前核对 `cargo metadata --no-deps --format-version 1`。集成测试使用 `C:/Users/Tmiracle/.codex/worktrees/markdown-scroll-fix/Editor/target/`；主线合并后 Release 使用主项目 `target/`，没有跨源码共享输出。最终宿主构建命令：

```powershell
$env:NANOBUG_RELEASE_VERSION = '0.1.0-internal.20261103'
cargo build -p editor-app --release --locked --jobs 1
```

插件在各自仓库通过宿主公开 `--plugin-cargo` 测试、`--plugin-package <workspace> --output C:/Projects/RustProjects/Editor/dist/<id>/` 打包。资源包无需空 WASM。最终 Release 宿主对 12 个成品再次 `--plugin-check` 全部通过；其内嵌 SDK 与插件构建使用的 SDK 共 94 个文件逐一 SHA-256 相同，SDK 摘要为 `4fcb7f89448e48854fd1281808277c3859aea83343322c9de46e088adc0e1c20`。未改变公开协议或为了发行重写 SDK 来源声明。

宿主载荷只有 EXE、图标、构建信息、内测说明及四份许可文件，共 8 个文件，不含开发者配置、缓存或插件。便携 ZIP 验证完整解包、文件集合和内含 EXE 摘要。安装器直接调用 Inno Setup `ISCC.exe`，参数为 `PayloadDir`、`InstallerDir`、`AppVersion=0.1.0`、`BuildLabel=0.1.0-internal.20261103` 与 `installer/windows/nanobug.iss`；未调用仓库外存档脚本或包装脚本。许可清单对应的 944 个 Cargo 包身份与已核对的上一版一致。

宿主为 Windows GUI 子系统、静态 CRT；宿主及 Git/Java/Maven/Rust 原生辅助程序的导入表没有动态 VC++ CRT 依赖。既有用户工具依赖仍按插件权限与依赖准备流程处理。

## 自动与真实包验证

| 检查 | 本轮结果 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked --jobs 2` | 128 个目标完成，888 passed、0 failed、447 ignored；包含 `editor-ui` |
| `cargo check --workspace --locked --jobs 2` | 通过 |
| 宿主相关普通测试 | 184 passed；更新 11、通用插件 UI 96、冷实例 2、市场 12、profile 1、会话 13、历史 29、插件开发 9、终端 11 |
| 真实插件包宿主测试，显式 `--ignored` | 14 passed；Git 首次安装/更新 2、Markdown 长文档 1、双向滚动 6、过期视口 1、JS/Vue 导航 1、TypeScript 1、Vue grammar 2 |
| 真实插件包运行时回归，显式 `--ignored` | 5 passed；Git 生命周期、Java 分析与进程回收、Maven 导入、Image 资源权限、XML 结构 |
| `host_update_native_navigation_and_cancelled_restart --ignored` | 1 passed；Windows/D3D、中英、深浅主题、100%/150%、空窗口和项目入口、取消未保存文档退出 |
| `host_update_github_release_download_smoke --ignored` | 1 passed；真实 GitHub 下载、SHA-256、缓存和临时便携替换 |
| 原生输入专项 | 6 passed；包含慢读子进程 FIFO、队列背压与关闭；已包含于 workspace 范围，不额外累加 |
| 插件 `--plugin-cargo test --lib` | 360 passed；仅有代码包运行对应测试，无测试或纯资源包不虚构用例 |
| 最终 EXE 的 Windows 分发检查 | 3 passed；GUI 子系统、CLI JSON 重定向、错误 stderr 与退出码 |

宿主测试先通过 `cargo test -p editor-app --bin editor-app --no-run --locked --jobs 1 --config 'profile.test.package.editor-app.debug=1'` 构建，再用生成的测试程序运行上述过滤词，设置 `RUST_MIN_STACK=16777216`。普通 ignored 不计为通过，真实包按新发行 ZIP 与摘要显式提供输入。测试没有安装到用户日常插件目录。

本轮最初的终端专项为 9 passed、2 failed。检查发现外层原生输入已接收后，底层容量更小的写队列返回暂满，导致输入丢失。修复保留单个最早阻塞输入、阻止后续输入越过它，并继续处理输出与独立撤销；新增真实慢读与永不读取子进程回归，确保有序交付且拥有者退出仍能关闭。终端断言也改为等待真实提示符，避免把命令回显误当成结果。修复后重跑终端 11 项、原生更新入口和最终 workspace 检查均通过，原失败日志保留。

## 安装与原生界面

实际安装器在任务自有目录 `C:/Users/Tmiracle/.codex/task-artifacts/nanobug-release-20261103/installed-host/` 完成首次安装、重装、卸载，三个退出码均为 0。测试前没有 Nanobug 安装注册项或菜单注册；只操作本轮自有安装，没有覆盖日常安装。

安装后的 EXE 与构建信息摘要匹配载荷。最终 EXE 使用显式隔离 profile 启动，关于页面真实显示 `0.1.0-internal.20261103` 和“安装版 · Windows x64”。只使用截图与键盘的普通设置关闭、主窗口退出返回 0。重装保留配置、插件、历史和更新目录中的四个测试数据文件；卸载移除程序与卸载器，保留四个文件且摘要不变，并清除 HKCU 安装注册。本轮 `/TASKS=` 禁用桌面及右键菜单任务，菜单键在安装前后均不存在，不把它计为菜单交互验收。完整日志与截图保留在任务资料目录。

### 未解决的无障碍并发关闭异常

首次通过 Windows UIA 读取设置窗口并关闭时，观察到一次 `accesskit_windows 0.34.0/src/util.rs:380` 的 `ClientToScreen` 无效 HWND 异常：`HRESULT(0x80070578)`，随后 `panic in a function that cannot unwind` 终止进程。该次尝试不计为通过。相同错误位置已记录于 [2026-10-19 控件验收](../../ui/verification/base-only-components-2026-10-19.md)，不能据此宣称本轮已修复或完全排除新触发方式。

本轮对上一版 `0.1.0-internal.20261102.2` 进行两次读取/关闭设置对照，未复现，正常退出码为 0；新版随后不并发读取 UIA 的普通关闭也为 0。本轮保留崩溃日志，未升级、fork 或关闭无障碍依赖。发行说明明确披露该限制，不将全量无障碍验收标为通过。

## 其余限制与证据

- [安装根目录存储](../../project/portable-data.md)中的完整历史 UI 矩阵仍受原有“2001 行应拒绝”断言与现行 `MAX_SOURCE_LINES=100_000` 不匹配阻断。已有真实保存/恢复切片有效，但不能把未运行的后续浅色、缩放及 OS IME 候选窗记为通过。
- 全目标宿主测试构建会遇到既有 `svg_preview_render` 示例编译问题；本轮针对应用使用 `--bin editor-app`，没有声称全部示例和全部 ignored 测试通过。既有警告未顺手修复。
- 本期仅提供 Windows x64 原生产物，未签名；macOS/Linux 原生安装与运行未验收。宿主不随附或自动恢复插件，JS/Vue 新权限仍需用户按正常更新流程确认。
- 原始 patch、各阶段失败与成功日志、构建信息、安装日志、截图及发行 API 记录保存在 `C:/Users/Tmiracle/.codex/task-artifacts/nanobug-release-20261103/`。重点文件为 `release-workspace-tests.log`、`integration-actual-results.json`、`actual-package-regressions.json`、`host-final-validation.json`、`installer-lifecycle-validation.json` 和 `release-staging-state.json`。

## 发布读回

12 个插件正式 Release 与宿主 Pre-release 均已公开，读回 `draft=false`；13 个标签逐一解析到上表插件提交和宿主 `5f6f5885`。29 个附件的 GitHub `digest`、大小和本地文件逐一匹配，没有覆盖旧版本附件。

随后不携带身份凭据访问全部 13 个公开发行页，均返回 HTTP 200；从公开附件直链完整下载 29 个文件，共 257,764,644 字节，重新计算 SHA-256 全部匹配。首次匿名 API 读回遇到 403 限额，改用公开 HTML 页面与附件直链完成校验，没有把限额请求计为成功。

公开状态、标签与附件账本见本机 `published-releases.json`；无凭据页面访问与完整下载摘要分别见 `public-release-validation.json`、`public-download-validation.json`。宿主发布后的说明正文已再次读回，确认包含无障碍异常和验收范围。最终文档链接、路径与 `git diff --check` 通过；纯文档收尾未重复运行无关 Rust 构建。

返回[安装与分发](../README.md)。
