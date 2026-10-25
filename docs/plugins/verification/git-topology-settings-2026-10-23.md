# Git 拓扑图深浅主题颜色设置

日期：2026-10-23。状态：已完成独立插件实现、两组合实际包原生验收及本地 ZIP 交付；未安装到用户配置或发布远端。全仓格式检查的并行工作例外见下文。

## 行为与公开能力

- “设置 → 插件 → Git”提供深色、浅色各八个颜色项，键为 `graph.colors.dark.1` 至 `.8`、`graph.colors.light.1` 至 `.8`。默认沿用 [0.1.31 配色](git-topology-colors-2026-10-23.md)，普通提交实心、合并提交空心的已确认规则保留。
- 输入六位 RGB（`#RRGGBB`）后应用，可逐项恢复默认。大小写均接受；无效值在保存前返回字段错误，不覆盖已保存颜色。设置仅用户作用域，由通用 Manager 持久化；不接受项目覆盖，不在 Git 仓库或插件快照另存一份。
- 使用现有 `configuration >=1.1,<2` 和 `settings_hook`，十六项均为 `hot_apply`。初始／重载 Apply 在激活前恢复完整设置，Validate 不产生业务副作用。Changed 只替换归一化调色板；活动主题颜色变化时仅更新已有历史场景，不依赖恢复时尚未到达的可见性通知，也不打开隐藏面板；非活动主题不重绘当前图。
- SVG 缓存按实际八个颜色值失效，拓扑几何与滚动 epoch 不变。不取消／泵送 Git 查询，不改变选择、滚动、提交草稿或提交窗口。颜色数组在设置变化时解析，滚动时不重复解析设置。
- 独立插件版本为 0.1.32；协议、权限保持原值，仅增加已有配置能力的协商声明。宿主生产代码不增加 Git 专属接口；宿主改动仅扩充原生设置验收。实现位于 `C:/Projects/RustProjects/Git-for-Nanobug/src/history/palette.rs`，声明位于该项目的 `manifest.json`。

## 验证结果与现场

证据目录：`C:/Projects/RustProjects/Editor/target/git-topology-settings-2026-10-23/`。本任务开始时旧宿主及插件构建缓存已被清理。重建途中收到“清理项目构建文件”会话的用户停编要求，已停止本会话构建；源码备份保留在证据目录的 `before/`，并另存到该聊天的本地可视化目录。闸门 `nanobug-full-cleanup-build-hold-20261023.json` 解除后，按原 Debug 入口重建。中断构建不计作通过。

- 插件回归覆盖十六项声明与默认值、合法／非法 RGB、项目覆盖拒绝、纯验证与原子应用、真实序列化 Changed 保留在途查询／草稿／滚动，以及主题数组和 SVG 缓存。
- `host-build-resumed-results.json`：原 Debug 宿主重建通过，使用现有 Rust 工具链和已安装 WASI target，没有安装新工具。`palette-tests-fixed.log` 的 7 项针对性测试及最终 `guest-tests-final.log` 的完整 171 项插件测试均通过。
- 首轮原生驱动误把普通导航 Div 和输入框包装层的 debug selector 当作 Base 原生控件 ID，未进入表单。修正为真实导航点击、实际 Apply/Reset 按钮 ID 和紧邻按钮上方的 Base 输入框，保存值断言不变；未新增测试专用生产接口。
- `restored-panel-red.log` 用最小序列化配置回归确认：已有历史场景但可见性通知尚未到达时，热配置不会刷新图。修正为刷新已有场景、不主动打开面板后，完整 171 项测试再次通过。这是独立回归发现，不能据此认定它就是原生失败的原因。
- 原生流程一度收到 Apply 成功回执但图未变化。补充输入框实际值及落盘值断言后，`native-values/` 确认活动主题输入仍为默认值；截图显示该输入已滚到固定标题栏下，快照仍保留可见元数据。驱动增加完整输入边界检查，避开被裁剪的控件。`native/`、`native-controls/`、`native-final/`、`native-values/` 的失败证据均保留，不计作最终通过。
- `native-clipping/` 已通过实际输入、两个独立用户值落盘和图中节点／连线即时改色；后续非法值回执等待超时，尚不计作整套通过。独立设置窗口不在原生 harness 的插件窗口绘制列表中，驱动补充该窗口的实际绘制与失败截图，继续核验拒绝和重置流程。
- `native-dialog/` 的功能流程通过后，驱动错误地等待已关闭窗口；改为观察主窗口。`native-complete/` 的深色功能、选择／滚动、资源 owner 及提交场景检查均通过，但原有总检查把主动构造的配置拒绝当成未知错误。驱动只将匹配本次错误回执、晚于操作前日志 ID 的同源同文错误逐条登记；最终审计按 ID／来源／消息精确核对，其余错误继续失败，不删除或改写日志。
- `native-audit/` 截图确认超时时界面仍在应用、按钮被禁用：实际冷 WASM 校验在并发构建负载下超过普通 UI 的 45 秒预算。仅配置回执观察改为 120 秒有界等待，不重发输入，也不修改生产超时。最终驱动为 `native-budget.exe`，深色中文 100%（225.68 秒）和浅色英文 150%（278.49 秒）均通过全部功能、状态保留、滚轮／拖条和关闭审计，外层用例通过（504.44 秒）。
- `package-final.log`：最终候选由宿主 `--plugin-package` 构建，十六项配置、`configuration >=1.1,<2` 与不变的安装权限已核对；最终 SHA-256 为 `7538ef88f05400af061b6636e37896a7c6582ba20c087949d35fb0e4979a5cc4`。早期候选 `9ce6ce425bccf65ebd5e88b11358934aa550c1ff6d0c119deebd4f3d28fb3486` 不作为最终交付包。
- 首次 `fmt.log` 报告其他会话正在修改的 `crates/editor-app/src/tests/mod.rs`、`window_geometry.rs` 存在格式差异，本任务未修改这些文件。后续会重查全仓格式；不将其他会话改动计入本任务成果。
- 较早的 `fmt-last-results.json` 格式检查通过；`check-results.json` 的 workspace 测试与编译检查通过，测试为 519 passed、383 ignored。ignored 不计作通过，原生实际包测试显式另行执行。编译保留现有警告及 LNK4217 警告，无编译错误。
- 后续其他会话新增 Maven 用例和语言上下文实现后，`final-style-results.json` 中全仓格式检查再次失败，涉及 `crates/plugin-runtime/tests/maven_package.rs`、`maven_package/` 及 `plugin-protocol`／`plugin-runtime` 的语言上下文文件，具体路径见 `final-style.log`。本任务原生验收文件的独立 rustfmt 检查通过，`git diff --check` 通过；未代其他任务修改格式。此前 workspace 测试与编译的通过结论对应当时构建快照，不涵盖后续其他会话新增的代码。
- `native-budget-results.json` 返回 0，`final-evidence-summary.json` 汇总两组实际证据。每组均通过七项配色检查、两种主题独立用户值落盘、一次明确关联的预期配置拒绝、零条非预期错误和完整关闭。已核对两组截图中的实际颜色、实心／空心节点及缩放结果。
- 交付包 `C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.32.zip`，979622 字节；与原生测试候选 SHA-256 完全一致：`7538ef88f05400af061b6636e37896a7c6582ba20c087949d35fb0e4979a5cc4`。`package-final-identity.json` 核对十六项设置、配置能力及不变的安装权限。过期驱动副本已清理，保留最终驱动、全部日志／截图与候选包，见 `probe-cleanup.json`。

```powershell
cargo build -p editor-app --jobs 4
$nanobug = 'C:/Projects/RustProjects/Editor/target/debug/editor-app.exe'
& $nanobug --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib
& $nanobug --plugin-package C:/Projects/RustProjects/Git-for-Nanobug --output target/git-topology-settings-2026-10-23/packages-final
cargo test -p editor-app git_native_history_scroll_probe --no-run --jobs 2
cargo fmt --check
cargo test --workspace --exclude editor-app --jobs 2
cargo check --workspace --jobs 2
```

原生流程设置 `NANOBUG_GIT_CANDIDATE`、`NANOBUG_GIT_NATIVE_OUTPUT`、`NANOBUG_GIT_NATIVE_HISTORY_ONLY=1`、`NANOBUG_GIT_NATIVE_SCROLL_ONLY=1`、`NANOBUG_GIT_NATIVE_GRAPH_COLORS=1`、`NANOBUG_GIT_NATIVE_GRAPH_SETTINGS=1`，显式执行 `extensions::worker::native_acceptance::git_native_history_scroll_probe --exact --ignored --nocapture --test-threads=1`。两组分别为深色中文 100%／浅色英文 150%，不改变用户系统缩放或用户配置。

收尾按新项目约定执行只读历史检索（关键词：Git 拓扑图／分支颜色／深浅主题／`configuration hot_apply`），返回 `status=unavailable`、`memories=[]`、`TimeoutError`，没有采用历史命中；本记录结论来自本轮源码与制品证据。
