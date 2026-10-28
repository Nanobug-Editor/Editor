# 主分支插件与 Rust 服务修复

日期：2026-10-27，Windows x86_64。状态：截图中的故障已修复，本机包更新、针对性回归、工作区门禁与原生复验通过。

用户授权在当前 `main` 修复并保留改动，不自动提交。宿主起点为 `2b043e2d74c91cbb10f44ff8e247cf677425fd48`，初始工作区干净；没有提交、推送或发布插件市场版本。

## 已复现的问题

1. 本机 Git 0.1.43、Markdown 0.18.5、Image 0.6.1 仍要求 `editor.layout ^1`，当前宿主按[统一 Editor 规格](../specs/unified-editor.md)提供 2.0。生产 `--plugin-check` 对旧 Image 包返回 `required_capability`，定位 `/api/required/editor.layout`，所需 `^1`、可用 `2.0.0`。
2. Rust 0.6.1 在该宿主工作区无条件链接根项目与 34 个独立插件夹具。相同 rust-analyzer、SDK 配置和实验能力下，35 项配置初始化成功但 135.062 秒仍未就绪；仅保留根项目后 14.219 秒收到 `quiescent=true`。不是 LSP 初始化协议失败，也不通过延长 120 秒期限处理。
3. 更新 Markdown 后，打开根 `README.md` 两次触发宿主崩溃。第二次采集的 stderr 为 `cannot update editor_app::ui::plugin::PluginView while it is already being updated`；调用链经过 `source_toolbar` 与嵌套原生 Editor 绘制。Windows Application 1000 记录异常码 `0xc000041d`，实际 Rust panic 日志比 Release 符号最近匹配更直接。

证据目录为 `target/plugin-language-repair-20261027/`。初始生产程序 SHA-256 为 `55b2f9c818f4e29516dfd22c5d16c3051d92c37e07b99812a9e555253b40dd39`，SDK 摘要为 `8d488db82a03cd433952171b085695ae015885a26c32f6d57f1ac4691d2bdf27`。

## 本机包更新

通过正在运行的宿主插件管理器，显式安装既有迁移候选；没有添加旧布局执行分支或自动升级机制。

| 插件 | 原安装版本 | 本轮候选 | ZIP SHA-256 | 当前验证 |
| --- | --- | --- | --- | --- |
| Git | 0.1.43 | 0.1.45 | `903d2ebf0b6e493dbee06c89f22c0e6e46998718d338f08825631deaea3ff505` | 已安装，分支标题、提交面板和历史图恢复 |
| Markdown | 0.18.5 | 0.18.6 | `6a84326bb3a0debcecd57838434fc63669254e140488c4212127c8188104f633` | 已安装，实机分栏、工具栏、预览模式及切换正常 |
| Image，安装 ID `svg` | 0.6.1 | 0.6.2 | `9a7b5a9da47a96512ba911823be531f0afcff04a3005ae84b7b4c59046976e72` | 已安装，实机打开 `nanobug.png` 正常渲染 |
| Rust | 0.6.1 | 0.6.2 | `89d4a20475bae56bb59084b7b90fb378069800b1408607552173eba605b98d92` | 已安装，实际服务就绪，根工作区 Rust 悬浮信息返回 |

前三份候选已记录在固定[输入账本](../../../crates/plugin-runtime/tests/fixtures/packages/inputs.json)，来源为独立插件仓库的既有迁移，没有修改历史包字节。Rust 修复保留在 `C:/Projects/RustProjects/Rust-for-Nanobug` 的 `main`，按请求文档选择相关 Cargo 项目，嵌套独立插件保留各自上下文及公开 SDK。详细记录为该仓库的 `docs/verification/document-scoped-language-startup-2026-10-27.md`。

四包均没有新增权限，安装 ID 保持不变。审计确认前三包的实际安装摘要与候选一致；七个当前安装包均通过生产静态准入。Git helper、HTML/JavaScript 的 Node 和服务文件共五项依赖缓存的完整标记、路径及 SHA-256 均正确。更新沿用公开事务路径及既有启用选择、授权和私有数据目录；审计记录的私有文件摘要采于更新后，不据此声称逐字验证过更新前后数据。

## 宿主调度修复

`extensions/canvas_frames.rs` 不再为空请求队列更新原生视图；有真实帧请求或需要隐藏暂停时，使用 `cx.defer` 等待嵌套绘制借用释放。延迟回调保留并重新检查原插件、面板、实例 epoch、原生实体及当前显示状态，不将旧隐藏操作施加到重新显示的表面。下层仍检查资源取消状态、文档／场景身份和帧代际，没有新增重绘通知、业务定时器或插件专属分支。

实际 Markdown 包回归在旧实现先出现相同 double-lease panic，0 passed、1 failed，运行 57.19 秒；修复后 1 passed，65.20 秒，覆盖首开绘制、真实加粗、Undo、切换普通文档隐藏及重开。

## 验证进度

工作目录和默认 Cargo 输出均为当前宿主 checkout；`cargo metadata --no-deps --format-version 1` 已核对 `workspace_root` 与 `target_directory`。独立 Rust 仓库使用自己的默认输出，通过宿主 `--plugin-cargo`、`--plugin-package` 构建，不复制 SDK 或添加源码路径依赖。

| 检查 | 实际结果 |
| --- | --- |
| `cargo test --workspace --exclude editor-app --locked --offline -j 4` | 126 组，831 passed、0 failed、441 ignored；ignored 未计为通过 |
| Rust：`--plugin-cargo Cargo.toml test --lib --locked --offline` | 10 passed、0 failed；项目发现回归已观察先失败后通过 |
| Rust：`cargo fmt --check`、`git diff --check`、公开打包和 `--plugin-check` | 通过，未把静态检查描述为真实 LSP 就绪 |
| `cargo test -p editor-app delivered_markdown_first_open_keeps_toolbar_and_native_body_live --locked --offline -j4 -- --ignored --test-threads=1 --nocapture` | 先失败后通过，输出见 `markdown-red.log`、`markdown-green.log`；设置 `RUST_MIN_STACK=16777216` |
| 既有 `delivered_markdown_source_fills_height_below_toolbar`、`delivered_source_height_survives_empty_documents_and_toolbar_free_previews` | 同一已编译测试程序显式执行 ignored，各 1 passed；81.23 秒、142.20 秒，含深浅主题、空文档和 Image 源码布局 |
| 同上入口过滤 `pending_canvas_frames_cross_nested_panel_borrows_and_hidden_surfaces` | 1 passed、0 failed、0 ignored，58.60 秒，见 `canvas-frame-routing.log`；实际 Manager 帧资源经过 ExtensionPanel 延迟路由及测试平台下一帧，隐藏保持 pending，重新显示后交付一次 |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo check --workspace --locked --offline -j 4` | 通过，保留既有警告 |
| `cargo build -p editor-app --release --locked --offline -j 4` | 通过，生产程序已重新启动；新增测试模块不进入此生产构建 |
| Windows 原生交互 | Rust 根项目与嵌套 SDK 项目同时分析、真实悬浮信息、Markdown 分栏与预览切换、Image PNG 显示通过 |

当前原生验收程序为 `target/release/editor-app.exe`，SHA-256 为 `8edb734d9f4ebb5dbc885c3ba985717de50fb769377ee84c4fa299a65a694b4b`。使用原用户 profile 和真实 Editor 工作区，通过插件管理器更新 Rust，未新增授权。13:48:55 语言服务启动，后续底栏启动提示正常消失；13:52 实际 `Ctrl+I` 返回 `editor_ui::aligned_pointer::for_window` 的签名、源码注释和定义位置。RA 保留了少量不存在监听路径的 warning；没有把这些 warning 作为服务就绪失败。

随后在同一工作区打开 `crates/plugin-runtime/tests/fixtures/sdk-completion/src/lib.rs`，根项目的原生服务仍保留，独立项目另启一个服务并正常就绪。对 `plugin_protocol::ui::Action` 执行 `Ctrl+I`，实际返回枚举成员及定义位置，路径指向宿主管理的 `MeEditor/plugin-sdk/8d488db82a03cd433952171b085695ae015885a26c32f6d57f1ac4691d2bdf27/src/ui.rs`；未给访客加入 SDK 副本或源码依赖。测试服务产生的夹具锁文件刷新在验收后还原，不纳入本次变更。

Markdown 实机先显示包含原生源码工具栏的分栏，右侧正确绘制标题、中文链接与本地图片；点击预览切为完整预览，再恢复分栏，切换图片与返回 Rust 均无崩溃。Image 实际打开 `crates/editor-app/assets/branding/nanobug.png` 并显示完整图片。Git 的当前分支标题、提交文件树与真实提交历史在同一 Release 窗口持续可用。没有执行实际 Git 写操作，也没有修改实际打开的 README、图片或访客源码。关闭 SDK 访客后，其独立语言服务器已退出，根项目服务保持可用。

本次实际窗口使用浅色中文；深浅主题、空文档、无工具栏 Image 源码等由上述既有自动回归覆盖。441 项 ignored 及未安装的其他语言生态不作为已通过的动态验收。当前 Release 窗口保留运行，改动留在宿主与独立 Rust 仓库的 `main`，未提交、推送或发布。

返回[插件文档目录](../README.md)。
