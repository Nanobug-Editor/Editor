# 05：通用树与编辑器装饰实施候选

日期：2026-10-11。状态：正式包、自动宿主接入及当前三项仓库门禁已通过；当前独立双轴复查及主代理桌面验收尚在进行。T24/C03 未因此标记完成，议题 [#97](https://github.com/T-miracle/Nanobug/issues/97) 保持开放。

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
| 当前行为/构建候选 | `7ab592878ed71943479a9e20e4423abb27249354` |
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

实际四包分别执行同一公开构建入口，日志为 `action-{id}-package.log`，均为 release 成功；没有仓库 `crates/` 路径依赖、SDK 源码副本或历史构建脚本。宿主构建日志 `action-sdk-host-build.log`：exit 0，1m27s。宿主 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-trees/target05/debug/editor-app.exe` SHA256：`27189941951236959DC137B44F5AF0F58468A5DE24316B1E31CE15B07CB9ED20`。

当前 SDK 导出为 41 个源码/Cargo/WIT/JSON 资源、0 个 Markdown；逐文件与本候选 `crates/plugin-protocol/` 的原始字节核对，hash 差异为 0。内容缓存 digest：`93226c2989d2714785bb6d2b9562b5c4afa98e3e191417df8d2fbd127dc34ae2`。正式包均使用此宿主管理的 SDK 缓存，日志记录实际 cache 路径。

包位于工作树 `target/community-presentation/`；SHA256 取自 ZIP 及其实际 WASM 条目：

| 包 | ZIP SHA256 | WASM SHA256 |
| --- | --- | --- |
| `database-tree-0.1.1.zip` | `782887A98A18D6D052378DB527E6A8DAE47226D1ED4F961134A8FB54BF5AE3DE` | `ED657192890B1876F1755B60C8042D8DCA98C1ECEB563CEE3B2D7C2A5E782FFA` |
| `todo-tree-0.1.1.zip` | `CCAFBE037DD5A73277DC5C8F28380DE12AD0B20281E7F7100C329A74D16C68FE` | `A51F4C862F19F50579A8BB76411D9E29EB50F15EE533B31B7B2BF6652CD5EBC4` |
| `bookmarks-0.1.1.zip` | `B31717B0F0A397C78BAF57FED4C7EE427992119B9166979F4CC0ADA953BA07C0` | `B75B6CD834949B452BAF0CC68936CF4A6586BA09659BEE29F684FFB210985472` |
| `coverage-0.1.1.zip` | `D099242DFEFD27100D4EA08EB4693F0F46472FCB761D38A5B07A89D86D2076EC` | `19F4971C357439A6BA32F79C6265FBD8011067BFD9506D7A2016AA1879211CFF` |

## 当前自动验证

这些测试消费上述正式包，运行数据各用独立 tempdir；GPUI TestAppContext 的实际绘制和输入不等同于主代理的物理桌面验收。

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

### 真实 Red/Green 与失败保留

- 菜单异参回归先在真实 native 菜单执行 details，却收到 `recovered:summary`，**0 passed / 1 failed**、22.23s，日志 `tree-action-native-red-arguments.log`；增加精确 action 身份后上述当前 Green 通过。两次更早菜单 ID/语言选择失败没有冒充 typed 参数 Red。
- 先前的树错误自动重试、保留退役行及 native revoke 未归还配额均有实际 Red/Green，原始日志分别保留为 `database-native-retry-*`、`database-native-paging-menu-*`、`tree-native-revoke-*`。这些旧日志说明开发过程，不替代当前 0.1.1 包验证。
- 初次合并后的非 UI 构建遭 `LNK1104 msvcrt.lib`，exit 101，`merged-non-ui-workspace.log` 未报告测试通过；同一 shell 核对 VS 环境后 jobs 2 重试 exit 0，244 passed / 206 ignored，`merged-non-ui-workspace-retry.log`。这属于旧固定点证据，最终当前门禁另列。
- 曾误用较宽 `community_` 过滤词命中未准备的 01 夹具；相关失败保留，没有计为 05 通过。ignored 测试必须先直接构建正式包，再显式执行，默认跳过不能计数。

## 主代理桌面接入与待验项目

本阶段不直接操作桌面。主代理已独占原生验收；以下是现有公开入口的操作指引，尚未冒称执行结果。

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

尚待当前四包同窗口的深浅主题、中英文、中文 IME、滚动/缩放、可见 gutter/range/hover、基础辅助名称与提供者替换确认。当前源码已提供语义颜色/主题图标、本地 Tooltip 和 Base 树语义；这些实现与自动断言没有替代物理原生验收。其他操作系统未执行 native 验证。

## 独立审查与交付边界

原固定点 `de539764…a4ad93` / `a4ad93…0d42133` 的 Standards 结论为硬性 0、判断性 1（P3 重复图标准入）；Spec 随后发现 P2 同名节点 action 参数混淆。两项已在当前候选修正；新的 `a4ad93…7ab5928` / `0d42133…7ab5928` 增量正在分别交给独立 Standards 与 Spec 复审，旧结论不覆盖新输入。

本记录与普通候选提交不表示 push、议题关闭、版本发布或 T24/C03 完成。主代理在当前复查、门禁与桌面验收完成后统一集成及交付；07 再补跨能力组合，不重复本单负面矩阵。
