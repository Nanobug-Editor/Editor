# Git 历史自动续页与最早提交

日期：2026-10-24。状态：滚动分页修复、最终 0.1.37 包、公开 Manager 与两组合 Windows 原生回归完成；完整插件测试的一项范围外失败见下文。未安装到用户配置、提交、推送或发布。

## 问题和反馈循环

用户截图在仍有父提交连线的位置停止。当前插件每次只读 128 条，滚轮和滚动条只夹紧已加载范围；原有“更多提交”菜单可以继续分页，但普通滚动没有调用它的读取路径。

本轮先按项目约定检索历史，`status=ok`。采用的相关记录为历史分页性能优化（`memory_id=84dd71ee-351d-4ed2-befd-9f4706199458`，`document_id=conversation:01a15bdd-0356-7141-805f-12ac00606d33:turn:01a16295-2d06-7942-bb95-a6ca91fc81dd`）及面板隔离／原生帧滚动（`memory_id=4015d46f-a9e8-403d-a719-13acf75ec57c`，`document_id=conversation:01a15bdd-0356-7141-805f-12ac00606d33:turn:01a1635c-c123-7dc0-ba29-6dd4a6aaf2e3`），均为 2026-10-22。核对当前实现后，只将它们作为既有分页和窗口隔离的历史依据。

证据目录为 `target/git-history-origin-2026-10-24/`。

- 在实际 `State::history_canvas` 回调和完整 Git 结果解码边界建立回归：128 条对照通过，129 条稳定只显示 128 条，日志为 `minimal-control.log`、`minimal-red.log`。测试执行约 0.01 秒，不依赖人工操作。
- 扩大回归覆盖 385 条、多页滚轮／拖动滚动条、视口变高、已有串行任务、失败后的手动重试、重叠页无进展及到达起点后新增提交的刷新；新增刷新场景也先失败于只保留 129／130 条，见 `refresh-red.log`。
- Windows 原生旧 0.1.34 包在真实 389 条仓库中停在第一批的范围，偏移 3352；失败信息为 `native scrolling stopped before the repository root`，项目及原生窗口撤销完成。

## 修复

独立 Git 源码位于 `C:/Projects/RustProjects/Git-for-Nanobug`。视口接近尾部时保留一个可合并的读取需求；沿用当前筛选、代次和受控 Git 队列，从已接受记录数量继续读下一批。最多提前补一个视口，保持绘制缓冲和已接受行，避免一次性加载整仓库。忙碌完成后按最新位置重试需求；向前滚离尾部或隐藏历史面板后撤销需求。

短页建立本次观察的末尾。失败及全重叠页面结束本次自动续页，后续新手势可重试，不能通过空转重复查询同一偏移。刷新或新 HEAD 完成后重新检查视口需求，避免原来已经可见的起点被推到下一页而永久隐藏。新过滤器清除旧续页意图，过期 epoch 和旧查询回执沿用现有拒绝规则。

宿主只增加实际包与原生回归。现有拓扑颜色测试同步提供并行变更新增的列测量参数，以继续覆盖跨页颜色和缓存，不改变绘制行为。

## 验证和交付

| 检查 | 实际结果 |
| --- | --- |
| 初轮独立插件全量测试 | **179 passed**；六项分页回归随后在最终快照中全部通过 |
| 最终 0.1.37 固定快照全量测试 | **183 passed、1 failed**，`snapshot-tests-verified.log`。失败为 `panel_isolation_commit_details_keep_history_query`：在空 `State` 上直接点击已移除的 `status` 节点，收到“请打开仓库根目录”；该测试及提交区节点路由不属本次分页修复，未改写其断言 |
| 最终 0.1.37 实际 WASM／Manager | **1 passed**，`delivery-package-test.log`。真实多页 Git 历史自动加载完整数量，末行在视口内，滚动保留原选择，末行点击的 OID 与 `git rev-list --max-parents=0 --all` 一致 |
| `git_native_history_scroll_probe --ignored` | **1 父测试、2 个窗口场景通过**，`native-final.log`。深色中文 100%、浅色英文 150%，各自真实仓库均为 **389 条**，均用 **4 次**真实滚动读到起点；完整根行可见，点击后的公开预览 OID 与 Git 一致 |
| 同一原生程序使用旧 0.1.34 | 预期失败，`native-red-verified.log`：停在首批 128 条的范围，偏移 3352，未到真实根提交。没有把早期验收代码编译失败或文字绘制类型断言错误算作行为红灯 |
| `cargo test --workspace --exclude editor-app --jobs 2` | **551 passed、400 ignored、0 failed**，`workspace-tests.log`；ignored 不计为通过 |
| `cargo check --workspace --jobs 2` | **通过**，`workspace-check.log`，保留已有警告 |
| `cargo fmt --check` | 最终 **通过**，`workspace-format-final.log` |
| 固定 Git 快照自身的格式检查 | **未通过**，`snapshot-format.log`：差异在冻结时的并行列宽测量及相关测试等文件；当前源目录中本次涉及文件的独立格式检查通过，未为排版改写已经验证并打包的快照 |
| 全仓 `git diff --check` | **通过**，`diff-check.log`，保留已有 CRLF 提示 |

两组原生证据为 `native-final/dark-zh-CN-100/`、`native-final/light-en-150/`，包含 `evidence.json` 和已人工查看的 D3D11 `history-scroll.png`。两组均记录 `shutdown_complete: true`、`lifecycle_scenario.ok: true`，完成项目关闭和原生窗口撤销。根行可见性取实际绘制坐标，点击结果取公开预览文档；没有合成 Git 专用宿主状态。未验收其他平台或系统输入法候选窗。

共享工作区的并行改动新增了文本测量和输入框行数契约。旧宿主导出的 SDK 缺少这些类型；首次新导出也缺少 `ui/canvas/text_measure.rs`。已核对产物来源，使用包含完整 SDK 的新构建入口。Git 源码冻结在 `guest-snapshot-final/`，先以 `cargo metadata --no-deps --format-version 1` 确认 `workspace_root`，并使用独立 `snapshot-target-final/` 构建；记录为 `snapshot-metadata-final.json`、`snapshot-files-final.json`。复制出的快照只用于复现和交付，没有覆盖正在并行修改的源目录。

组合快照保留了当时其他任务的 UI 改动，因此须使用配套最新宿主；清单协议仍为 **7**，权限集合与开始时相同，快照另含其他 UI 改动所需的 `ui.native >=1.2` 和 `ui.styles` 声明。本次自动续页未增加公开 API。

- 交付包：`C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.37.zip`，与 `packages/git-0.1.37.zip` 相同。
- SHA-256：`dc4c990eced35028346a6639909115f2cb23de1c66d7b3780dea96b8433732fd`。
- 配套普通 Debug 宿主：`target/git-history-origin-2026-10-24/host-verified.exe`，由 `cargo build -p editor-app --jobs 2` 的本机产物复制，SDK 导出、固定快照测试与打包入口已实际使用；SHA-256：`4997e9cecfeb5c9caa14e40078e3b1d6962d7c170929ae83524b7b3f3df2f291`。
- 原生测试程序：`native-verified.exe`，由本轮 `cargo test -p editor-app git_native_history_scroll_probe --no-run --jobs 2` 构建；测试程序不是普通宿主。

格式检查、范围外失败和验证限度均按日志保留，不将未运行场景或完整插件测试记为全绿。
