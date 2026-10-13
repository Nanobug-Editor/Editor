# 插件管理器共用布局验收

日期：2026-10-13。依据：[已确认方案](../specs/plugin-manager-shared-layout.md)。本次为一个连续的宿主 UI 改动，按 execute-plan → implement 直接执行，没有新建或关闭 GitHub 工单。

实现和 2026-10-13 的 Windows 自动化原生交互验收已完成。2026-10-14 用户追加授权普通提交并推送到 main，保留其他未提交改动；Git 交付按末节执行。

## 比较点与改动归属

用户指定比较最新上游主分支。2026-10-13 验收及 2026-10-14 提交前读回的 `git ls-remote origin refs/heads/main`、本地 `origin/main` 与 HEAD 均为 `5b8eaae50ebbefe8b4874c8c11fdb544bf57b561`，Standards / Spec 均按该提交复审全部本次差异及新增文件。实施开始时本地为 `d640d50991d7f5454efb8c2017ad7ed7c46d760f`；其间其他任务提交的项目窗口改动不计为本次成果。

实施开始时工作区已清洁。本次没有改动 `Cargo.lock`，交付读回 SHA-256 为 `a0076c4bccb22b08b6b74b082440dd08e8b6798937e101e9c302b2670f07cffb`。没有协议、SDK、依赖或插件包版本变更，也没有修改冻结 ZIP 或输入账本。

## 实际行为与验收入口

生产管理窗口现在通过 `extensions/management.rs` 共用侧栏、输入、模式 Tab、列表、固定详情头、操作区、详情 Tab 和正文滚动区；`actions` 与 `details` 分别承担本地实例操作和详情呈现。原市场独立整页布局已移除。列表行复用本地 Base Button，输入前缀和筛选浮层复用 Base 的输入、焦点及 Popup / PopoverState 行为。

入口为用户选定的真实 ZIP → 公开 Package / Manager / Worker → 生产 GPUI 管理窗口。没有新增测试专用宿主 API；HTTP 只替换传输端，下载、摘要核对、授权和安装仍走实际路径。

| 场景 | 可观察结果 |
| --- | --- |
| 两种模式与长正文 | 切换已安装/市场后侧栏及详情边界相同；标题、动作和 Tab 保持固定，README/日志在正文内滚动 |
| 搜索与筛选 | 图标在市场搜索框左端，与输入同框；浮层在下方左对齐；分类、清除和非默认标记的实际点击通过；标签/排序保留现有搜索逻辑与本地操作入口，本轮未单独点击；Escape 关闭后完整 Enter 按下/抬起可以重新打开 |
| 键盘与详情 | 列表可由实际焦点路径选择下一项；概览、变化、版本、信息按数据启用；评价保持禁用；本地日志入口及原共享已读状态继续有效 |
| 浏览版本与安装实例 | 本地 `Legacy Guide 1.0.0` 搜索结果进入市场 `Journal 1.1.0` 历史时保留插件 ID、清除旧名称搜索；头部同时显示浏览版与安装版 |
| 市场页管理本地实例 | 全局禁用、本项目启用、重启、日志、取消卸载及保留私有数据的确认卸载都作用于实际安装的 1.0.0；重启 epoch 改变，管理操作不下载 1.1.0，完成后仍在市场页 |
| 安装与更新 | 原更新标记、新增权限确认、真实 TOML 旧版到新版安装及撤回目录场景通过；同版、降级、过期授权和离线安装限制沿用原入口 |
| 未安装时失败 | 实际下载错误摘要字节后，首次安装仍为空，市场页明确显示失败原因；没有因为缺少本地日志归属而吞掉错误 |
| 语言、主题、窗口 | 两个原生市场用例分别覆盖中文浅色和英文深色；覆盖 850×600 窗口、20px 字体、1.5 显示缩放、真实指针/键盘/文本/滚轮输入与边界断言 |
| Markdown 与输入 | 市场外部图片保持惰性、不发起外部请求；链接行为沿现有安全路径；旧回显及显式重置回归通过；实际 WASM 交互的中文 IME 确认回归通过 |

两个市场原生用例为 `marketplace_native_consent_and_install_in_both_locales` 和 `marketplace_browsing_version_preserves_local_lifecycle_and_fixed_layout`。新布局测试在临时目录生成合法资源 ZIP，版本与名称变化是明确的测试输入；现有冻结档案保持原字节。

## 测试与检查

使用现有 VS 18 x64 开发环境、锁定依赖和共享 Cargo 构建缓存；`CARGO_TARGET_DIR=C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/target01`，`CARGO_BUILD_JOBS=2`，`RUST_MIN_STACK=16777216`。不安装工具，不调用历史构建脚本。

| 实际命令/过滤词 | 结果 | 本机日志（仓库根 target） |
| --- | --- | --- |
| `cargo fmt --check` | exit 0 | `plugin-layout-fmt.log` |
| `cargo test -p editor-app marketplace_ -- --test-threads=1 --nocapture` | 5 passed、0 failed、4 ignored；运行 5.73s | `plugin-layout-marketplace-final.log` |
| `cargo test -p editor-app extensions::management_tests -- --test-threads=1` | 6 passed、0 failed、0 ignored；运行 3.80s | `plugin-layout-management.log` |
| `cargo test -p editor-app market_update_dot_and_permission_delta_follow_real_native_actions -- --test-threads=1 --nocapture` | 1 passed；运行 1.16s | `plugin-layout-regression-1.log` |
| `cargo test -p editor-app unbundled_official_market_install_and_existing_user_upgrade -- --test-threads=1 --nocapture` | 1 passed；运行 1.97s | `plugin-layout-regression-2.log` |
| `cargo test -p editor-app ui::plugin::tests::native_input_survives_stale_echo_and_explicit_reset_does_not_echo -- --test-threads=1 --nocapture` | 1 passed；运行 0.02s | `plugin-layout-regression-3.log` |
| 下述同一测试二进制的显式 IME `--ignored` 命令 | 1 passed、0 failed、0 ignored；运行 62.09s | `plugin-layout-ime.log` |
| `cargo test --workspace --exclude editor-app` | exit 0；267 passed、0 failed、255 ignored | `plugin-layout-workspace-test.log` |
| `cargo check --workspace` | exit 0；Cargo 结束耗时 10.39s | `plugin-layout-check.log` |
| `git diff --check` | exit 0；文档补链后再次核对 | 终端读回 |

本次 editor-app 共 15 项不同用例实际通过。默认市场组的四项 ignored 包括 WASM 更新事务、额外官方安装和线上 Pages，用例没有在本轮执行，不能计为通过；既有验收仅作为复用边界。非 UI 的 255 项 ignored 同样不计为通过。编译仍报告现有未使用代码和可见性等警告，不宣称零警告。

IME 用例初次默认命令只有 1 ignored（`plugin-layout-regression-4.log`）。随后读取冻结夹具说明，核对 `capability-example-0.19.0.zip` 与账本均为 `bdfc5d3327b5b7a4f3d25ba1ae95e2a6a47b62634fb040e17fb4430c9aa46773`，使用相同最终测试二进制显式执行：

```powershell
& 'C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/target01/debug/deps/editor_app-d4e1859b30e4cb6a.exe' native_plugin_input_does_not_confirm_marked_ime_text --ignored --nocapture --test-threads=1
```

测试经公开 Manager 安装该实际 WASM，按命令需求激活，再通过 Base Input 使用 Windows IME 的文本标记接口验证：组合期间 Enter 不完成插件请求；提交“你好”后 Enter 返回该文本。没有改变超时预算或替换历史包。

最终 app 测试二进制 SHA-256：`d7230260964fd752d0d9f79d0cc2769e01ee2768bc63ca503dff5bcfa9c7e439`。最后通过的市场、管理、更新、官方包、输入和 IME 结果均对应这份二进制；之后只补充文档。

## 失败与修复证据

- 首次共用框架用例在旧市场缺少共同侧栏时失败；完成布局收敛后通过。筛选左对齐先捕获 8px、再捕获 1px 偏移，最终补偿输入前缀内边距与边框后通过。
- Enter 重开浮层的中途失败来自测试仅发送 KeyDown；核对锁定 GPUI Base 源码后补全 KeyUp，保留既有 NoAction 边界。没有通过改写产品按键语义让用例通过。
- 原共用裸列表行的键盘选择红灯通过本地 Base Button 修复。旧名称到新名称历史导航红灯通过明确清除旧搜索、恢复选定插件 ID/版本修复。首次下载失败不可见红灯通过共用错误区域修复。
- 生命周期测试曾在异步状态已发布但 busy 尚未清除时继续点击；改为同时等待实际状态/epoch 与任务结束，仍验证真实 actor，不放宽业务断言。
- 整理 Rust 格式时，补丁准备器曾误带 `rustfmt --emit stdout` 的 Windows 路径标题并导致编译失败。已用补丁去除全部误写标题并修正准备器；随后格式检查、最终 app 构建、15 项相关用例和 workspace 两项门禁均重新通过。该中途失败保留在本机日志，不记作成功。

## Standards

适用依据为根 AGENTS.md、本地 UI Base 行为与主题契约、公开入口测试和注释规则。审查覆盖新增文件，排除比较点之前的其他任务改动。

- 原 P2：共用列表裸绘制缺少 Base 的键盘/可访问行为。已替换本地 Button，完整 KeyDown/KeyUp 的实际焦点回归通过。
- 原 P3：新增方案与验收没有加入插件文档入口。已同步 `docs/plugins/README.md`，并为旧日志方案注明明确替代关系。
- 筛选触发器发布 `expanded(open)`，焦点与 Popup 撤销继续使用 Base；新模块和复杂分支有职责、状态边界及生命周期注释。

最终未解决：硬性规则 0 项，判断性坏味道 0 项。独立只读 Standards 代理复审后再次读回交付文档。

## Spec

适用依据为本轮确认选择、共用布局补充方案和现行市场安全/版本规则。

- 原 P2：未安装插件的失败日志归属会隐藏首次下载错误。共用错误区域在市场显示该操作失败，坏 SHA 的真实下载回归通过。
- 原 P2：旧安装名称搜索会在市场新名称中隐藏历史选择。进入市场历史前清除旧搜索，再指定插件 ID/版本；Legacy → Journal 的合法 ZIP 组合回归通过。
- 六 Tab 的顺序和数据可用性、本地/审核 README、两个版本标识、安装与实例管理的操作目标，以及原权限、离线和仅升级条件均按确认要求执行。

最终未解决 0 项。独立只读 Spec 代理按最终比较点和所有新增文件复审；没有把构建检查当作 UI 行为证据。

## 验证限制与交付状态

原生证据为 Windows GPUI 自动化真实事件、渲染和布局断言；没有宣称人工桌面验收，也没有验证 Linux/macOS。本次没有构建 Release 安装器、发布版本或线上市场数据。未运行的额外 WASM/线上网络用例见上表；本次改动没有改变其安装事务实现。

2026-10-13 验收时，上述源码、测试、双语文案、图标、方案和验收记录保持未提交、未推送。2026-10-14 的追加授权只将这批共用布局交付纳入 main 的普通提交和推送，提交前核对全部 20 个目标文件；其他未提交内容及原暂存状态须保留。没有关闭 issue、修改父设计议题，或将其他任务与早期社区平台成果再次计为本次交付。

2026-10-14 提交准备阶段运行 `cargo test -p editor-app --no-run --message-format=json`，app 测试产物为 Fresh，SHA-256 与上述最终验收二进制一致；再次 `cargo fmt --check` 通过。本次只有验收文档的授权状态更新，代码、输入与环境未改变，因此复用上表实际通过的行为测试和 workspace 检查结果，提交前另核对暂存差异与文档链接。
