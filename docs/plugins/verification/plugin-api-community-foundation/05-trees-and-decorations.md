# 05：通用树与编辑器装饰实施候选

日期：2026-10-11。状态：正式包、受影响回归、当前宿主构建、三项 Rust 门禁、独立双轴与当前宿主原生复验通过，T21–T24/C01–C06 的行为验收已满足；普通推送与议题 [#97](https://github.com/T-miracle/Nanobug/issues/97) 关闭结果另行读回核对。

依据：[05 工单](../../tickets/plugin-api-community-foundation/05-trees-and-decorations.md)、[社区方案](../../specs/plugin-api-community-foundation.md)、[共同完成条件](../../tickets/plugin-api-community-foundation/README.md)。公开用法、权限、错误、限额、取消与释放说明位于 [Tree 源码](../../../../crates/plugin-protocol/src/tree.rs)和[装饰源码](../../../../crates/plugin-protocol/src/decorations.rs)，不内嵌或导出 Markdown。

## 固定输入

| 项目 | 当前值 |
| --- | --- |
| 05 工作树/分支 | `C:/Users/Tmiracle/.codex/worktrees/plugin-api-trees/Editor` / `codex/plugin-api-trees` |
| 初始依赖点 | `b54aa46931bc24021791456f47fbe894660742ce` |
| 05 首个候选 | `58904ca803d046c49696d6707867624cf5f8ce3c` |
| main + 01/02 的已验证基线 | `de5397645d05c5d91e8cd74108d46ad9d7a88ff3` |
| 普通合并 | `a4ad93dc088d0f4973bb3e8d0efb66315bbe76ac`，同时保留两侧能力 |
| 平台说明与 Base 树访问断言 | `0d4213302ce0b408ec7a6cef24000a177798c313` |
| 共享图标准入 | `51e5bb676c13fec7a0424e2efbb21ad128c323d7` |
| 首轮正式包/自动矩阵/桌面输入 | `7ab592878ed71943479a9e20e4423abb27249354`；后续纯文档 `d9b06d4b30da398fec681f073ff47ee7f9bd7aca` |
| 通用更新快照与恢复修复 | `465b44d30c5007888b6bdacc1aef438fdc8c88cb` |
| 当前滚动修复/宿主/门禁候选 | `edb2cd838d5fc5427708498401b9bb8f27a97300`，Git tree `352912ee584a6c561261929428d17e4b76c97506` |
| SDK / 协议 / 能力 | SDK crate `0.2.0`；protocol `7` / base `1`；稳定 `ui.tree 1.0`、`editor.decorations 1.0` |
| 工具链 | Rust/Cargo `1.98.1`，Windows x64 MSVC，已安装的 `wasm32-wasip2` |
| Cargo 输出 / 日志 | `../target05` / `../target05/logs/`，下文日志名均相对此目录 |

原项目中的其他在制品没有进入本候选；未恢复退役的独立 terminal 插件。原生终端、通用执行/PTY、main 的 configuration/debug/targets 和已交付 01/02 源码接口仍保留。

## 实际消费者与契约

四个独立项目使用同一公开管理器、能力协商和权限边界，没有宿主插件 ID 专属业务分支：

| 消费者 `0.1.1` | 领域行为 | 原生接缝 |
| --- | --- | --- |
| `todo-tree` | 从打开文档的当前 Snapshot/DocEvents 提取 TODO/FIXME；节点命令定位文档 | `ui.tree`、typed 命令和正式文档事件 |
| `database-tree` | 受控 `main → users` 512 项、每页 16 项；首次错误等待 Retry；`slow` 保留迟到页 | 同一 Tree、分页/错误/取消/刷新与 typed 节点菜单 |
| `bookmarks` | 对当前真实选区建立右亲和点书签，读回当前位置 | 同一 `editor.decorations`、原生点几何和行号旁图标 |
| `coverage` | 从未保存的 COVERED/UNCOVERED 文本重算业务范围 | 同一装饰接口；正式文档事件后重新取 Snapshot |

Tree 使用 Base `TreeState`/`TreeItem` 的虚拟化、焦点、键盘和扩展状态，本地 `ui/` 提供外观。节点身份在刷新内稳定；句柄、epoch、请求序号、父节点与分页 cursor 一起约束准入。折叠、刷新、移除场景或实例退役拒绝旧回复。显式错误页保留已接受数据，用户 Retry 才重新请求。

一次节点发布的激活行为与各菜单序号是独立身份，即使命令名称相同也分别解析完整 typed 参数。缺失菜单序号或旧 epoch 返回 `StaleRevision`；命令仍通过既有声明、参数 schema、权限与实例边界。

装饰通过唯一现有 InputHighlighter 的批量更新接缝与折叠状态组合，不建立可变 Rope、额外 Undo 栈、语法工厂或另一份 EditorLayout。`0..0` 点保留单独锚点元数据，因为上游范围集合会移除空范围。点遵循明确亲和性；范围外侧插入不纳入、内部插入扩展、整个范围删除/替换后变为 `None`。普通 Undo 不自动复活已删除范围；coverage 根据正式 DocEvents 与当前文本重新计算含义。编辑不会清空其他提供者的标记。

树和装饰共用实例内包图标准入：`ui.icons` + `assets.read`、包内路径、每图 64 KiB、SVG 校验与主题变体去重；各调用方继续承担自己的资源归属和 2 MiB 总 artwork 预算。原生 marker 的布局、命中和绘制按编辑器可见输入区域裁剪。

## 正式构建与指纹

每次 Cargo 都在该进程 shell 中初始化 VS，不改变系统环境或安装工具链：

```powershell
Import-Module 'C:/Program Files/Microsoft Visual Studio/18/Community/Common7/Tools/Microsoft.VisualStudio.DevShell.dll'
Enter-VsDevShell -VsInstallPath 'C:/Program Files/Microsoft Visual Studio/18/Community' -DevCmdArguments '-arch=x64 -host_arch=x64' -SkipAutomaticLocation | Out-Null
$env:CARGO_TARGET_DIR='../target05'
$env:CARGO_BUILD_JOBS='2'
cargo build -p editor-app --jobs 2
& '../target05/debug/editor-app.exe' --export-plugin-sdk '../target05/sdk-source-actions'
& '../target05/debug/editor-app.exe' --plugin-package plugins/database-tree plugins/todo-tree plugins/bookmarks plugins/coverage --output target/community-presentation --release
```

首轮 `7ab5928` 的四包分别执行同一公开构建入口，日志为 `action-{id}-package.log`，均为 release 成功；没有仓库 `crates/` 路径依赖、SDK 源码副本或历史构建脚本。旧宿主构建日志 `action-sdk-host-build.log`：exit 0，1m27s，SHA256 `27189941951236959DC137B44F5AF0F58468A5DE24316B1E31CE15B07CB9ED20`。该旧 EXE 及首轮真实失败证据由主代理另存，不冒充修复后的宿主。

当前固定 `edb2cd8` / Git tree `352912e` 直接执行 `cargo build -p editor-app`，exit 0，39.44s，日志 `scroll-update-host-build.log`；同 shell 工具链、源码点和干净状态见 `scroll-update-host-environment.log`。新宿主 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-trees/target05/debug/editor-app.exe` SHA256 为 `B5B3D67403836D0BE4A347E822162C2B8117567FF11BEC431BC88A7C3CE2B3B5`，见 `scroll-update-host-hash.log`；物理复验另行记录。

当前 SDK 导出为 41 个源码/Cargo/WIT/JSON 资源、0 个 Markdown；逐文件与本候选 `crates/plugin-protocol/` 的原始字节核对，hash 差异为 0。内容缓存 digest：`93226c2989d2714785bb6d2b9562b5c4afa98e3e191417df8d2fbd127dc34ae2`。正式包均使用此宿主管理的 SDK 缓存，日志记录实际 cache 路径。

本轮新 EXE 又实际执行 `--export-plugin-sdk ../target05/sdk-scroll-update-edb2cd8`；此入口在创建 GPUI 窗口前返回，没有启动桌面。`scroll-update-sdk-verification.json` 固定上述完整 HEAD/tree、实际导出目录，读回仍为 **41 文件 / 0 Markdown / 0 hash 差异**，实际 SDK key 不变。`7ab5928..edb2cd8` 的 `crates/plugin-protocol/` 与 `plugins/` diff 为 0，四个正式 ZIP/WASM 沿用原字节，没有重复编译或提升无改动包版本。

包位于工作树 `target/community-presentation/`；SHA256 取自 ZIP 及其实际 WASM 条目：

| 包 | ZIP SHA256 | WASM SHA256 |
| --- | --- | --- |
| `database-tree-0.1.1.zip` | `782887A98A18D6D052378DB527E6A8DAE47226D1ED4F961134A8FB54BF5AE3DE` | `ED657192890B1876F1755B60C8042D8DCA98C1ECEB563CEE3B2D7C2A5E782FFA` |
| `todo-tree-0.1.1.zip` | `CCAFBE037DD5A73277DC5C8F28380DE12AD0B20281E7F7100C329A74D16C68FE` | `A51F4C862F19F50579A8BB76411D9E29EB50F15EE533B31B7B2BF6652CD5EBC4` |
| `bookmarks-0.1.1.zip` | `B31717B0F0A397C78BAF57FED4C7EE427992119B9166979F4CC0ADA953BA07C0` | `B75B6CD834949B452BAF0CC68936CF4A6586BA09659BEE29F684FFB210985472` |
| `coverage-0.1.1.zip` | `D099242DFEFD27100D4EA08EB4693F0F46472FCB761D38A5B07A89D86D2076EC` | `19F4971C357439A6BA32F79C6265FBD8011067BFD9506D7A2016AA1879211CFF` |

## 首轮正式包自动矩阵（历史固定点 `7ab5928`）

这些测试消费上述正式包，运行数据各用独立 tempdir；记录保留首轮实际输入和结果，没有宣称本轮重复执行全部矩阵。GPUI TestAppContext 的实际绘制和输入不等同于主代理的物理桌面验收。

| 命令/接缝 | 当前结果 | 证据 |
| --- | --- | --- |
| `cargo test -p plugin-runtime --test community_presentation -- --ignored --test-threads=1` | **3 passed / 0 failed / 0 ignored**，140.87s | `action-trees-public-manager.log`：正式包分页/Retry/迟到拒绝/菜单异参；配额、越权、optional 不满足、原子页准入与 native close 后配额回收 |
| `cargo test -p editor-app community_database_tree_native_keyboard_waits_for_manual_retry -- --ignored --test-threads=1` | **1 passed / 0 failed / 0 ignored**，43.11s | `tree-action-native-green.log`：真实 Enter/右键菜单异参、512 节点虚拟化、Retry、焦点恢复、刷新时菜单撤销、禁用清理；Base Tree/TreeItem 名称、selected/expanded 断言 |
| `cargo test -p editor-app community_presentation_tests -- --ignored --test-threads=1` | **2 passed / 0 failed / 0 ignored**，97.11s | `action-markers-native.log`：Todo 的真实 Snapshot/定位、coverage 的键盘 cut/replace/Undo 重算、关闭清理；字节零书签、中文/CRLF/非 BMP、真实输入与历史、gutter 投影/可见区；17 次坏 UTF-8 准入不泄配额、stale Replace 不清旧层、取消、跨 owner 和禁用 |
| `cargo test -p editor-app native_outline_single_highlight_and_temporary_follow_expansion -- --test-threads=1` | **1 passed**，1.01s | `action-outline-highlight.log`：既有唯一语法高亮与跟随扩展 |
| `cargo test -p editor-app native_xml_cancelled_parser_preserves_paste_undo_redo -- --test-threads=1` | **1 passed**，0.36s | `action-xml-undo.log`：取消解析不吞真实 paste/Undo/Redo |
| `cargo test -p editor-app sdk_export::tests -- --test-threads=1` | **4 passed**，0.23s | `action-sdk-tests.log`：来源/资源限制、fresh 导出、并发收敛与缓存修复 |
| `cargo fmt --check` | **exit 0** | `action-final-fmt.log` |
| `cargo test --workspace --exclude editor-app --jobs 2` | **exit 0；244 passed / 0 failed / 206 ignored**，构建 2m25s | `action-final-non-ui.log`；默认 ignored 未计为通过，实际包用例另列 |
| `cargo check --workspace --jobs 2` | **exit 0**，12.84s | `action-final-workspace-check.log` |

共享图标准入提交还在隔离的“上一 HEAD + index”完整源码树通过 `cargo check -p plugin-runtime`（41.27s，`icon-intermediate-check.log`）及格式检查，没有借用后续菜单修改验证其中间状态。

## 本轮受影响接缝与当前门禁

针对性 Green 来自本轮修复过程，最终汇入 `465b44d` 与 `edb2cd8`；每次实际 shell/源码状态均另存相应 `*-environment.log`。下表最后三项门禁固定在干净 `edb2cd8` / Git tree `352912e`，没有沿用旧 `7ab5928` 的门禁或把默认 ignored 计为通过。

| 命令/接缝 | 实际结果 | 证据 |
| --- | --- | --- |
| `cargo test -p editor-app community_decorations_hide_offscreen_markers_and_restore_after_scroll -- --ignored --nocapture --test-threads=1` | **1 passed / 0 failed / 0 ignored**，39.19s；构建 55.60s | `decorations-scroll-green.log`；真实滚轮、连续稳定帧、滚回几何与磁盘文本 |
| `cargo test -p plugin-runtime --test community_presentation community_database_tree_update_captures_live_snapshot_before_retirement -- --ignored --nocapture --test-threads=1` | **1/0/0**，41.92s | `tree-update-snapshot-green.log`；旧树在 Snapshot 时仍可用，成功切换后撤销旧 owner |
| `cargo test -p plugin-runtime --test community_presentation community_database_tree_failed_cutover_restores_fresh_owned_tree -- --ignored --nocapture --test-threads=1` | **1/0/0**，58.19s | `tree-failed-cutover-green-retry.log`；公开迁移取消后恢复新 WASM/owner，旧树撤销、新树可用 |
| `cargo test -p plugin-runtime --test data_migration failed_migration_activation_and_commit_preserve_old_data -- --ignored --nocapture --test-threads=1` | **1/0/0**，222.04s | `fresh-rollback-data-migration.log`；失败迁移、激活和提交保留旧私有数据 |
| `cargo test -p plugin-runtime --test hot_update mixed_update_rollback_replaces_all_owners_and_preserves_latest_data -- --ignored --nocapture --test-threads=1` | **1/0/0**，164.64s | `fresh-rollback-hot-update.log`；最新 Data、服务/LSP/进程/owner 的恢复与退役 |
| `cargo fmt --check` | **exit 0**，2.59s，无输出 | 实际工具执行返回已保存；PowerShell `Tee-Object` 未生成空日志，不引用不存在的 `scroll-update-final-fmt.log` |
| `cargo test --workspace --exclude editor-app` | **exit 0；244 passed / 0 failed / 208 ignored**；构建 2m08s | `scroll-update-final-non-ui.log`、`scroll-update-final-non-ui-counts.json`；默认测试并行，无全局 `RUST_TEST_THREADS` |
| `cargo check --workspace` | **exit 0**，4.76s | `scroll-update-final-workspace-check.log` |

本轮沿用自有已真实构建的 `capability-example.zip`，版本 `0.19.0`、冻结 SDK38，SHA256 `BDFC5D3327B5B7A4F3D25BA1AE95E2A6A47B62634FB040E17FB4430C9AA46773`；这是既有 migration/hot-update 夹具，不冒称当前 SDK41 新构建。LSP 夹具直接 `cargo build -p plugin-runtime --example lsp_fixture` 成功，再从 `../target05/debug/examples/lsp_fixture.exe` 复制至本树 `target/debug/examples/lsp_fixture.exe`；源/目标 SHA256 同为 `B67DF87C7A220DE7E2900D503FCFE96C3F878DCE9F3CB171E40520A3AE53E405`，见 `rollback-lsp-fixture-build.log` 与 `rollback-lsp-fixture-hashes.log`。

本轮非 UI 门禁实际默认并行通过；没有放宽 HTTP 超时或断言，也没有用串行结果替代默认执行。已有首轮三项 Manager 正向和菜单/配额/UTF-8 矩阵未受滚动源码影响，不重复执行；两项新更新回归与既有迁移/mixed-update 回归覆盖本轮通用恢复变更。

### 真实 Red/Green 与失败保留

- 菜单异参回归先在真实 native 菜单执行 details，却收到 `recovered:summary`，**0 passed / 1 failed**、22.23s，日志 `tree-action-native-red-arguments.log`；增加精确 action 身份后上述当前 Green 通过。两次更早菜单 ID/语言选择失败没有冒充 typed 参数 Red。
- 先前的树错误自动重试、保留退役行及 native revoke 未归还配额均有实际 Red/Green，原始日志分别保留为 `database-native-retry-*`、`database-native-paging-menu-*`、`tree-native-revoke-*`。这些旧日志说明开发过程，不替代当前 0.1.1 包验证。
- 初次合并后的非 UI 构建遭 `LNK1104 msvcrt.lib`，exit 101，`merged-non-ui-workspace.log` 未报告测试通过；同一 shell 核对 VS 环境后 jobs 2 重试 exit 0，244 passed / 206 ignored，`merged-non-ui-workspace-retry.log`。这属于旧固定点证据，最终当前门禁另列。
- 曾误用较宽 `community_` 过滤词命中未准备的 01 夹具；相关失败保留，没有计为 05 通过。ignored 测试必须先直接构建正式包，再显式执行，默认跳过不能计数。
- 新更新回归先实际失败：`tree-update-snapshot-red.log` **0/1/0，38.99s**；移除非承重的初次分页后，`tree-update-snapshot-minimal-red.log` **0/1/0，40.18s**，两次均出现与现场相同的 Snapshot `Tree was revoked` 与 restart owner 不可用。Snapshot 改在资源撤销前执行后成功切换 Green 如上。
- 公开 `InstallControl` 在 `Migrating` 阶段取消的 `tree-failed-cutover-red.log` **0/1/0，41.12s**，恢复沿用旧 WASM 内存里的退役句柄，旧实例未恢复可用。改为从不可变已安装旧包创建新 WASM、恢复逻辑 Snapshot/设置和原 grants 后 Green 如上；相邻测试纯归位曾遗漏 module path，`tree-failed-cutover-green.log` 的 E0583 为编译失败，没有计为行为 Red 或 Green。

## 主代理桌面接入与当前复验

### 原生滚动缺陷的补充回归（自动与当前宿主物理复验通过）

主代理首轮在 204 行 CRLF 夹具中先验证中文、非 BMP 输入与 Undo 的 revision/锚点，再向下滚动 820 px；稳定首行已为 21，覆盖率图标却仍留在顶部，三项标记的辅助 Label 也未撤销。关闭文档后的标记和 Todo 清理通过；提供者 reload 另遇 snapshot 资源退役错误。首轮失败和本轮成功分开保存，未把首轮计为 T24 通过。

自动复现命令为 `cargo test -p editor-app community_decorations_hide_offscreen_markers_and_restore_after_scroll -- --ignored --nocapture --test-threads=1`，使用当前两个正式 0.1.1 包、生产 Manager、DocumentSession 与真实 GPUI 滚轮事件。首次实际 **0 passed / 1 failed / 0 ignored，95.32s**，`decorations-scroll-red.log`：原生可见首行已大于 3，但 coverage-line-2 仍有 `12px × 12px` 的绘制/命中 bounds；这属于行为 Red。测试要求连续三帧无三项标记 bounds，并在滚回时校验与原生源行几何一致、磁盘内容不变。Windows UIA Label 另由主代理物理复查，TestPlatform 不冒充其结果。

收紧复现时改用普通文件关闭/打开代替批量模拟输入；第一次 `decorations-scroll-minimal-red.log` **0/1/0，44.32s** 停在 coverage 范围为空，没有到达滚动断言，不计为缺陷 Red。夹具补规范化文件身份和插件调用前的载入内容断言后，`decorations-scroll-canonical-red.log` 再次实际 **0/1/0，39.12s**，两个精确 coverage 范围和初始 gutter 均通过，再次在屏外 coverage 图标仍有 `12px × 12px` bounds 处失败。相比首次 95.32s，去掉了非承重的批量输入步骤，保留同一真实缺陷路径。

原因已核对 Cargo.lock 锁定的 gpui-base 0.7.0：`input/base/state.rs:901–922` 的 `line_and_position_for_offset` 对视口前方 offset 使用 `saturating_sub`，会得到首个已布局行的位置；`range_to_bounds` 的返回值因此不能独自证明源行可见。其公开 `visible_row_range` 返回同一 native layout 的未换行 buffer 行范围（`state.rs:2962–2964`、`element.rs:1000–1060`）。本地 prepaint 已先按此范围筛掉屏外源行，才取得 bounds 并创建图标/命中/辅助节点；保留原像素裁剪处理部分可见行，没有复制第二套布局。

同一收紧命令修复后实际 **1 passed / 0 failed / 0 ignored，39.19s**（构建 55.60s），`decorations-scroll-green.log`：三项标记初次可见，真实向下滚轮 820 px 后连续三帧均没有标记 bounds，滚回后准确对齐原生源行，磁盘文本仍为准备夹具。回归归入相邻 `community_presentation_tests/scroll.rs`，共享已有正式包/原生发布 helpers；没有改动或重复既有菜单、配额、越权与 UTF-8 负面矩阵。SDK 与四个正式插件源包均未改变，沿用上表真实 0.1.1 ZIP。

首次桌面观察、独立的更新 snapshot/回滚修复及本轮精确输入见[主代理原生验收记录](05-trees-and-decorations-native.md)。上述行为 Red 与夹具失败日志均保留；当前 `edb2cd8` / `B5B3D674…E2B3B5` 宿主重建、SDK 读回和三门禁已通过。主代理已在修复版真实窗口完成深色 16px 下滚出/滚回、UIA 标签撤销/恢复与 Tooltip 命中、文档关闭、数据库开发 reload 后继续展开和分页；当前控制器正常退出，磁盘 hash 不变。首轮普通输入/Undo 沿用未改变的文档锚点接缝证据，本轮未重复或宣称新测 IME composition。

本子代理不直接操作桌面；主代理独占原生验收。以下保留现有公开入口的复现指引，逐项真实执行范围以相邻原生记录为准，不把指引本身当作执行结果。

同一隔离 profile 可用现有发行/dev bootstrap：把当前 host 放在独立 `bin/`，把 Todo/Bookmarks/Coverage 三个正式 ZIP 原样放入 `bin/plugins/`；DB 使用 `--plugin-dev <database-tree项目目录> --release --profile <独立目录> --workspace <验收工作区> --grant ui.panels`。首次 bootstrap 从 EXE 相邻的 plugins 目录通过 `Package::read` 与公开 `Manager::install` 装入其余三个包；不编辑安装状态，不增加宿主测试 API。每轮用新 profile，避免先前 `bootstrapped.json` 掩盖输入变化。

| 菜单/操作 | 应观察的结果 |
| --- | --- |
| DB 展开 main/users，选择 More | 16 → 32 → … → 512，滚动和键盘目标正常，屏幕只绘制可见行 |
| 展开 fault-once，明确 Retry | 首次错误等待操作，恢复后节点 Recovered/已恢复可用 |
| recovered 节点 Enter；右键查看详情/查看结构 | 状态分别结束于 `recovered:summary`、`recovered:details`、`recovered:schema` |
| 展开 slow 后折叠或 Refresh database/刷新数据库，再 Complete delayed page/完成延迟页 | 旧请求被拒绝，无旧子节点；重新展开后 15 秒内完成可见 `delayed-child` |
| Refresh Todos/刷新待办；Locate Todo/定位待办 | 反映当前打开的未保存 TODO/FIXME，定位准确，关闭后节点撤销 |
| Bookmark current position/标记当前位置；Read bookmark positions/读取书签位置 | 真实 caret 锚点与编辑/Undo 后字节位置一致，gutter 图标和说明跟随当前行 |
| Scan coverage/扫描覆盖率；Read coverage positions/读取覆盖率位置 | COVERED/UNCOVERED 范围和说明正确，cut/replace/Undo 后按正式文档事件重算 |
| 文档关闭、提供者禁用/替换 | 原树、图标、范围、hover 与订阅撤销，其他提供者仍可用 |

主代理夹具固定为 `native-qa-05/workspace/community-markers.txt`：204 行 CRLF、UTF-8 无 BOM、6481 bytes，SHA256 `6DE6707F787B5C592502CD62AB0343F6956C18FD7F474E1AF8088FD4844D0BD0`；由主代理维护，本子代理未写入或复用其运行目录。

首轮四包同窗口的树键盘、双语命令、普通中文/非 BMP 输入、Undo、关闭和原生提示证据与本轮受影响的滚动/缩放/辅助名称/reload 复验组合覆盖本单原生接缝；真实 IME 行为沿未改变的原生编辑器/01 验收证据，不把普通中文输入称为 composition。旧 Settings UIA HWND 退役问题仍按基线记录，本轮仅用截图操作 Settings、关闭后读取主窗口。其他操作系统未执行 native 验证。

## 独立审查与交付边界

原固定点 `de539764…a4ad93` / `a4ad93…0d42133` 的 Standards 结论为硬性 0、判断性 1（P3 重复图标准入）；Spec 随后发现 P2 同名节点 action 参数混淆。两项修正后的 `7ab5928` 已由主代理确认独立双轴复查通过。本轮固定 `7ab592878ed71943479a9e20e4423abb27249354..edb2cd838d5fc5427708498401b9bb8f27a97300`，由未参与本轮实现的 03 代理完成 Standards、04 代理完成 Spec 增量审查：硬性 0 / smell 0；missing 0 / partial 0 / out-of-scope 0 / implementation 0。覆盖 live Snapshot、已安装旧包新 WASM 恢复、原生可见源行过滤、真实包及滚轮回归；没有把旧整体结论冒充新输入审查。

当前针对性验证、三门禁、独立双轴与受影响桌面验收已满足本单交付条件；普通提交不自动表示已 push 或已关闭议题，远端结果仍由主代理核对。后续集成点另记实际 SDK 与资源语义，07 再做跨能力短组合，不重复本单未改变的负面矩阵。
