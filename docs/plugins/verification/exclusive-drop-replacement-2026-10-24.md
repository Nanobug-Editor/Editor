# 独占窗口整区替换验证

日期：2026-10-24。状态：功能验收及规定工作区检查完成；会话扩展测试仍有 1 项既有 OpenFiles 期望失败，未计为通过，见下文。

## 范围与实现

依据[整区替换规格](../specs/exclusive-drop-replacement.md)，拖入窗口或目标窗口独占时，通用 LocalDock 遮罩覆盖目标实际所在侧的完整区域，中间与边缘均执行单窗替换。被替代实体、原始显隐意图、资源和布局继续保留；普通窗口之间沿用边缘分屏。

- `editor-ui` 的拖放快照显式区分分屏和替换，预览与提交共用同一模式。区域按实际布局计算，主编辑器不被替换。
- `PanelPresentation` 保存临时单窗选择，以稳定窗口名称写入 `dock_replacement_panels`；新实体恢复后仍仅显示拖入窗口。显式选择普通入口时清除此临时选择，恢复普通组合。
- 提交先恢复来源侧实际可见性，再清理空区域，避免搬走独占窗口时删除其隐藏普通同伴。布局结构、侧别、区域开闭、成员或显隐变化使拖动失效，并停止 Base 拖放载荷，防止后续普通落点处理再次移动窗口。
- 原生终端使用同一公开呈现策略默认独占。独立 Git 只将 `history` 主面板声明为 `exclusive: true` 并要求 `ui.panels >=1.2,<2`；`staging` 提交窗口和两个独立工作窗口保持原声明。

本次保留开始时已有的工作区改动；固定 HEAD 为 `561aa5b2006c38668a083482156646a16cd02b3e`。本次未提交变更以任务开始时的文件快照单独审查，不能把 HEAD 全部差异计作本次实现。

## 已执行的回归

证据目录：`C:/Projects/RustProjects/Editor/target/exclusive-drag-replacement-2026-10-24/`。Cargo 输出使用其独立 `cargo/` 子目录，`metadata.json` 已核对 workspace root 与 target directory。

| 场景 | 失败证据与修正 |
| --- | --- |
| 普通窗口拖入独占目标中心 | `red-ordinary-replacement-retry.log` 复现缺少遮罩；后续专项通过完整遮罩、单窗显示和保留隐藏同伴检查 |
| 会话恢复 | `red-replacement-restore.log` 复现普通组合重新出现；补稳定名称意图后通过新实体恢复检查 |
| 原生终端默认独占 | `red-terminal-default.log` 复现与普通窗口并排；补通用属性后专项通过 |
| 独占来源离开后保留隐藏同伴 | `red-source-peers.log` 复现同伴从注册表消失；先同步呈现再清空源区域后，`green-source-peers.log` 通过 |
| 拖动中目标换侧 | `red-stale-side.log` 复现旧事务继续预览；首次修正仍被 Base 普通落点接管，补载荷撤销后 `green-stale-side2.log` 的 4 项专项通过 |
| 普通入口重新选择临时独占窗口 | `red-ordinary-reselect.log` 复现普通同伴仍隐藏；区分显式激活与恢复／提交选择后，新 5 项专项及完整 52 项拖放回归通过 |

初轮构建曾因其他并行改动尚未导出 `file_tab_actions` 报 E0425；导出补齐后相同入口重跑，不将该编译失败视作功能回归的 red。来源最后一窗搬走后，目标可随空区域释放自然扩展；遮罩比较悬停时的完整目标，落下后比较扩展后的完整区域，不能把回收空间误当分屏。

原有 `exclusive_tests::terminal` 专门验证普通终端夹具的共享、宽度与恢复行为，现显式关闭该夹具的独占属性；产品默认值由新的终端专项单独检查。此前该组 17 项通过。

并行任务将 Problems 与 Messages 的初始区域改为编辑器右列，旧拖放夹具依赖普通 Bottom、原生 Right 和独立 Editor Center。现通过 Base 公开构造与移动接口显式建立这些测试前提，不改变产品默认值、实际拖放输入或断言。隐藏终端尚无 Bottom 区域时先注册保留实体。右边界断言继续要求双向误差小于 2px，不采用只判断下界的放宽方式。另为新并行 `tests/problems.rs` 补必要的闭包类型以解除测试编译阻塞；未改变其业务断言。

最后一轮记录：`cargo fmt --check` 通过；`cargo test --workspace --exclude editor-app --offline --jobs 2` 为 **554 通过、402 跳过**；`cargo check --workspace --offline --jobs 2` 与 `cargo build -p editor-app --offline --jobs 2` 均通过。扩展回归如下，重叠过滤不累计为独立测试数：

| 验证入口 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test -p editor-app extensions::dock_tests --offline --jobs 2 -- --test-threads=1` | 52/52，含本次新增 5 项 | `verified-final-extensions-dock_tests.log` |
| `cargo test -p editor-app extensions::exclusive_tests --offline --jobs 2 -- --test-threads=1` | 17/17 | `verified-final-extensions-exclusive_tests.log` |
| 最后固定测试程序的 `terminal:: --test-threads=1` | 46 通过、2 跳过 | `last-terminal.log` |
| 最后固定测试程序的 `app::session::tests --test-threads=1` | 7 通过、1 失败；差异仍为隐藏 OpenFiles 节点 | `last-app-session-tests.log` |
| 最后固定测试程序的 `extensions::dock_tests::drag::editor_left_drop_keeps_bottom_across_both_columns --test-threads=1` | 1/1，严格双向右边界 | `last-strict-boundary.log` |

后面三项使用相同 Cargo 构建产物，SHA-256 为 `e44141a08958d9822969599d0d4afe8db708c7b9b4a90d75ad927fe5d585e599`；身份见 `strict-test-host-identity.json`。只读检查 `strict-boundary-binary-check.json` 确认其中存在严格边界断言、没有单向放宽断言，再执行对应行为测试，避免把编译中的中间产物当作最终验证。402 和 2 个跳过测试均没有计为通过；下文原生 ignored 专项另行显式执行。

过程中还遇到并行列宽控件未完成的导入、并行原生测试的 Windows 命名空间错误、测试程序运行时重链接失败，以及夹具缺少目标区域；这些失败均保留日志，未计为通过。最后以完成的实际产物和明确源码前提重新验证。

## Git 候选与当前限制

独立源码：`C:/Projects/RustProjects/Git-for-Nanobug/`，版本从本轮实际基线 `0.1.39` 提升为 `0.1.40`，同步清单、Cargo 自身锁项和双语 README。未修改访客业务 Rust 源码。

- 实际包：`target/exclusive-drag-replacement-2026-10-24/packages/git-0.1.40.zip`。
- ZIP SHA-256：`49c59de01f652ae9c691e8384db36a83a3785a463c1d7e9150736c46302fba61`。
- 通过宿主 `--plugin-cargo` 入口运行访客测试：185 通过；通过宿主打包入口构建实际 ZIP。
- 构建所用中间宿主 SHA-256：`6182e3fa4f662f00e5c4d3465a3445298df70888314f0089c72c73348665db6a`；内嵌 SDK SHA-256：`73518f813a96e9c6e51dff230d452ea15b34e104de1c056825177ec0abf525d6`。中间宿主不代表最后 UI 修复后的宿主。
- 中间宿主静态准入 `passed=true, executed=false, diagnostics=[]`，仅代表包、协议和能力校验，动态行为另验。

最终生产宿主：`target/exclusive-drag-replacement-2026-10-24/cargo/debug/editor-app.exe`，SHA-256 `b1ab6881263e8d21aad9d46eac69ec49e6483df870e72a1e0a0d6b1d4c14bc62`，内嵌 SDK SHA-256 `c975de30c4d501af957232f535b090a4b1e68b984f0550bcfb68ea161b609cbe`。该 SDK 包含其他并行任务的后续协议改动，本次不将这些改动计为独占拖放实现。最终宿主对同一固定 Git 0.1.40 ZIP 再检查仍 `passed=true, executed=false, diagnostics=[]`，退出 0；记录为 `git-static-check-delivery.json`。

独立 Git 源码随后由其他并行任务推进到 **0.1.42 / ui.canvas >=1.6**。2026-10-24 17:25 读回确认本次 `history.exclusive=true`、提交窗口未声明独占、`ui.panels >=1.2` 均继续保留。没有把该后续版本倒退为 0.1.40，也不把固定 0.1.40 的验收外推为后续全部功能验收。

初轮和最后固定程序的 `app::session::tests` 均为 7 通过、1 失败：`plugin_dock_layout_survives_delayed_startup` 的旧期望没有迁移后新增的隐藏 `OpenFiles` 节点。任务开始快照中的 `app/layout.rs` 已包含两处 `ensure_open_files_dock` 调用；本次未修改该实现或该测试。该失败保留，不计为通过，亦不扩大本次范围修复。本次整区替换自身的会话保存／新实体恢复检查通过。

## Windows/D3D 实际验收

使用上述固定 Git 0.1.40 包、隔离 profile、实际 WindowsPlatform/D3D11 和生产 `Window.dispatch_event` 输入路径；没有用测试布局替代被测拖放，也没有用 OS SendInput 或系统显示设置变更。深色中文 100% 与浅色英文 150% 两组均通过：

- Explorer 拖到 Git 中间：完整 Git 区域遮罩，悬停时原窗口不动，落下后 Explorer 单独占据该侧，Git 保留原始显隐意图。
- 原生终端打开：独占完整底部区域；再拖到大纲／提交组合的边缘，遮罩覆盖整组，落下后终端填满该侧。
- Escape 恢复原分配；来源普通同伴继续存在，原区域选择恢复；选择大纲后恢复普通组合，再从 Git 入口恢复其独占。
- 两组均完成宿主、受管进程和原生窗口清理，`unexpected_error_logs=[]`、`shutdown_complete=true`。

通过入口为本次 Cargo 成功构建的固定测试程序，加 `--exact extensions::worker::native_acceptance::exclusive::replacement::git_native_exclusive_full_replacement --ignored --nocapture --test-threads=1`，实际 **1 个父测试通过、含 2 个原生子进程**，耗时 242.40 秒。固定测试程序 SHA-256 `d3903f505956d95892059a0d808c6ca3ebdbc19e64e8c7e1220c8fc0cd041aa0`，身份记录 `native-host-identity.json`，结果 `native-replacement-final-actual.log`。之后的变动只补普通测试夹具，没有修改这组生产拖放实现或原生流程。

两份 `native/<case>/evidence.json` 的 scope 均为 `exclusive-full-replacement-only`，`exclusive_full_replacement_complete=true`，候选摘要一致。每份成功 receipt 精确列出 7 张实际截图，共 **14 张**；已目视核对完整遮罩、最终边界、恢复组合和深浅主题。目录中较早失败的截图不计为本轮成功证据。首次完整尝试在恢复入口误用了 `outline-panel-toggle`，修正为实际 `outline-toggle` 后才取得上述两组完成记录。

本次没有修改用户当前安装、重启用户宿主、提交、推送或发布版本。实际包的热安装机制没有新增宿主重启要求；宿主源代码改进需使用本记录的新构建。

## 审查与范围

按开始时文件快照执行 Standards / Spec 两轴只读审查，生产层来源同伴清理与拖动失效两项已补真实 red/green 并关闭；后续测试右边界的单向放宽也已恢复为严格双向检查。最终两轴未关闭发现均为 0。`review/base`、`review/current` 和 `review/review.patch` 保存本次可复核差异；共享的 main、原生 bridge / matrix 只纳入已确认的本次变更，未将后续并行功能计入本次成果。

返回[插件文档目录](../README.md)。
