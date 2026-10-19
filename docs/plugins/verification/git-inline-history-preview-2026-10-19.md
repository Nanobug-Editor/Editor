# Git 内嵌提交预览与历史比较标签

日期：2026-10-19。状态：独立 0.1.14 包、内嵌预览、原生历史／工作区比较及规定检查均通过。

## 本轮行为

- Git 面板保留分支树及提交拓扑，在右侧增加上下可调整的文件树和提交信息预览。分支左键筛选历史；提交选择（包括键盘导航）更新预览，不再默认打开独立提交详情窗口。
- 预览按固定 OID 读取全部改动文件、完整 UTF-8 提交说明、提交人、时间及父提交；过期选择的成功或错误回执均无权覆盖当前预览。较长说明按 UTF-8 边界分块，不截断；超限明确显示失败。
- 点击预览文件打开主编辑区只读比较标签。普通提交为父版本／所选提交，根提交左侧为空；合并提交为父提交 1／结果／父提交 2，多父提交允许显式选择不同父版本。
- 标签键包含 OID、父提交组合及路径，固定标签不随预览变化；历史比较不改写工作区、原文档或 Undo。冲突处理继续使用独立窗口。
- Git 策略及界面在独立项目 `C:/Projects/RustProjects/Git-for-Nanobug`；宿主仅扩展公开通用 `editor.diff 1.2`，没有插件 ID 分支。

## 公开接口与生命周期

`OpenDocumentComparison.third: Option<DocumentVersion>` 省略时保留 1.1 双栏行为；提供第三源必须协商 `editor.diff 1.2`，三源必须是不同的、可读且精确版本的会话。归属、取消和撤销边界与原有两源相同。三栏均只读，复用本地原生编辑器、对齐补白、同步滚动和两侧差异导航；三栏禁用只适用于两源的统一视图。

第三源参加固定标签的重开、源关闭、权限撤销和隐藏来源回收；临时输入限制在视图关闭时恢复。新建历史来源先由当前请求持有，只有比较成功才转交标签生命周期；失败或取消不影响已成功的旧标签，并在右侧预览显示诊断。

## 已执行验证

| 范围 | 命令／入口 | 结果 |
| --- | --- | --- |
| 独立访客 | 新宿主 `--plugin-cargo ./Cargo.toml test --lib` | 最终 109 通过，5.96s；包括 4 项取消／失败／旧来源／转交生命周期回归 |
| 三源准入 | `cargo test -p plugin-runtime --lib comparison_tests --jobs 1` | 2 通过：1.2 门禁及三源去重 |
| 三源预算及导航 | editor-app 测试二进制 `three_source_` | 2 通过 |
| 真实 ZIP 预览 | `git_plugin` 的 `workbench::history::`，显式 `--ignored` | 最终包 2 通过，150.02s；385 个提交分页、Focus／Save／新提交、完整消息及多父选择 |
| 真实 ZIP 同名文件／目录 | `git_plugin` 的 `workbench::tree_collisions::`，显式 `--ignored` | 最终包 2 通过，140.71s；暂存与历史树保留删除文件／新目录的独立身份、真实旧原文及空删除侧 |
| 原生文件点击与标签 | `windows_tests::history_preview`，显式 `--ignored` | 最终包 1 通过，82.73s；物理文件点击、根／普通／合并标签、三源只读输入、固定与切换、禁用后全部隐藏来源回收 |
| 原有工作区比较 | `windows_tests::comparisons::git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs`，显式 `--ignored` | 最终包 1 通过，77.75s；右侧原生编辑／未保存内容、固定标签、统一视图及隐藏来源 |
| 格式 | `cargo fmt --check` | 最终复查通过 |
| 非 UI workspace | `cargo test --workspace --exclude editor-app --jobs 2` | 78 组，295 通过，309 ignored，0 失败；忽略项不计通过 |
| workspace 编译 | `cargo check --workspace --jobs 1` | 最终复查通过，57.20s |

原生测试夹具需显式推进测试时钟，让原生树的 150ms 懒页投影脉冲完成后再物理点击；仅等待 Git 子进程完成不能证明行已绘制。测试没有伪造原生文件源或比较完成回执。

## 产物与边界

候选目录：`target/git-inline-preview-2026-10-19/`，配套宿主 `Nanobug.exe` 与独立 `git-0.1.14.zip`。`identity.json`、`SHA256SUMS` 及使用说明位于同目录。

| 产物 | SHA-256 |
| --- | --- |
| `Nanobug.exe`（118590976 bytes） | `cff38f717f2f4def65f31967753ee91926431c73761300fb7d3e3b0c523c3967` |
| `git-0.1.14.zip`（822971 bytes，7 项） | `486f81228ce163d899cf4f1def38f35f1f5ef08b04f93d8c0536baa0af99a9d9` |
| 包内 `git.wasm`（2535046 bytes） | `81ca173fc5c6ab335414bc99a96f4761034dd5af87513c2ee6d11108ebf26abd` |
| `git-for-Nanobug-0.1.14-source.zip`（177677 bytes，61 项） | `9d118a6269115230c03f71d5f182c4ca4c72849403e3909541ea4e555bbde0ad` |

独立源码快照位于 `C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.14/`，61 个归档项逐一读回，与当前独立源码 SHA-256 对比相同；同目录保存输入清单、逐文件摘要及包副本。无宿主 SDK 或上游源码副本。SDK 来自新宿主的公开缓存，摘要为 `a8c876b37cc1a05e1c71e3d53ca1c24214b1d84635d2811d6971d0dc2460d061`。

宿主通过 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o <候选目录>/Nanobug.exe` 构建（7m33s）；未覆盖运行中的 `target/release/editor-app.exe`。本轮最后一次工作区检查曾发现并发停靠改动的 `left_columns.rs` 类型不匹配，其作者修正后已重新执行并通过；没有把该并发改动归为 Git 功能成果。非 UI ignored 项仍明确保留为未执行。

日志：宿主 `target/git-inline-preview-{build,workspace-check,format,nonui,admission,three-source,manager,collisions,native,worktree-native}.log`；独立项目 `target/inline-preview-{unit,package}.log`。

本轮不提交或推送工作区、不安装／升级用户现有插件、不关闭用户运行中的程序，不修改既有 #119／#120 工单状态。工作区的侧栏／停靠及其他已存在或并发修改均保留；它们不归入本次 Git 改动成果。完整八组合、系统 IME 候选弹窗及长期运行矩阵不在本轮验收范围内。
