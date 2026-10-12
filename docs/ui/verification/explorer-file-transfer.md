# 资源管理器文件转移验收记录

日期：2026-10-07。工作分支：`codex/explorer-file-transfer`；基线：`main` 的 `ae63225afd6e8a362c6f186d7374323e97405469`。

规格：[explorer-file-transfer](../specs/explorer-file-transfer.md)。实施顺序：[外部粘贴 #78](https://github.com/T-miracle/Nanobug/issues/78)、[拖放 #79](https://github.com/T-miracle/Nanobug/issues/79)、[撤销重做 #80](https://github.com/T-miracle/Nanobug/issues/80)。

状态：基础实现已合并主分支；2026-10-12 已补 Windows 复制/剪切回归、跨入口联合操作、中英文明暗主题及最终退出清理。系统文件管理器跨窗口原生多项拖入仍待人工操作结果，不能据此关闭 #79 或父方案 #76。下文保留 2026-10-07 的历史记录，最新证据见本次复验章节。

## 2026-10-12 原生复验

- 起点为已重新 fetch 核对的 `origin/main`：`4d172b97cbbf7aff2bbcd986116321e3566023b9`。`origin/codex/explorer-file-transfer` 为 `b0b601d81f2f7ebfb63efe4697169a8e24bf1ed3`，已包含在该主分支中。
- 使用独立工作树 `codex/explorer-native-acceptance`，没有纳入主工作区的既有改动。验收资料根目录为 `C:/Users/Tmiracle/.codex/worktrees/explorer-native-acceptance/Editor/target/native-20261012/`；工作区为其 `project/`，来源为 `external/` 与 `windows-drag/`。`ME_EDITOR_PROFILE_HOME`、`ME_EDITOR_PLUGIN_HOME`、TEMP/TMP 及应用数据均定向到本任务目录。
- 最终中文原生二进制 SHA-256：`C3C3A9E18800AD195A9EB5DBC7039023D6C55C1A0A118BC524ED091D39C28600`。所有最终中文复验使用该独立副本，不使用来源不明或仍在运行的其他实例。

### 实际通过的原生操作

| 场景 | 实际输入与观察结果 | 本地证据 |
| --- | --- | --- |
| 多项外部复制 | 系统文件资源管理器选中文件及目录，Ctrl+C；资源树选中 `01-target` 后 Ctrl+V。源文件保留，目标字节哈希一致，原有 anchor 保留 | `evidence/after-system-copy.json` |
| 系统剪切回归 | 最终二进制中，以系统 Ctrl+X 和资源树 Ctrl+V 把 `cut-file.txt` 移入 `02-moved`。源路径消失，目标内容正确 | `evidence/final-system-cut.json` |
| 外部导入 → 树内移动 → 撤销 → 重做 | 打开已导入文件，输入中文未保存内容；真实树内拖动到 `02-moved`。目录树 Ctrl+Z 恢复旧路径，Ctrl+Shift+Z 再次移动；内容、标签和文本历史保留 | `evidence/after-tree-move.json`、`after-file-undo.json`、`after-file-redo.json`、`joint-text-preserved.png` |
| 标签路径与文本历史 | 会话文件的 open_tabs 包含迁移后的 `02-moved/native-file.txt`；编辑区文本撤销不改变磁盘位置。发现并修复文本 Ctrl+Shift+Z 缺失后，在最终构建实际输入、撤销及重做成功，再保存到新路径 | `evidence/final-text-redo.png`、`final-session.json` |
| 中英文、明暗主题 | 原生菜单显示文件操作及具体对象；同名选择、恢复冲突、未保存内容二次确认、进度取消及持续消息均可读，无翻译键泄漏 | `evidence/final-zh-*.png`、`en-*.png` |
| 二次确认拒绝与接受 | 中文与英文实例均先拒绝二次确认，未保存文本保留；重新执行并明确确认后才丢弃内容并恢复文件 | `evidence/final-discard-cancel-preserved.png`、`final-zh-*-discard.png`、`en-*-discard.png` |
| 冲突等待中退出 | 最终中文实例在文件冲突弹窗和后台等待任务存在时 Alt+F4；退出码 0，stderr 为空，已完成操作的项目外备份和等待任务清理，源及项目文件保留 | `evidence/final-exit-waiting-conflict.png`、`final-backups-before-exit.json`、`final-exit.json`、`final-cleanup.json` |

英文原生验收构建只临时将启动 locale 从 `zh-CN` 改为 `en`，其余源码与最终修复一致；SHA-256 为 `6B7CF728616DD996F03AF7790026E929620CD7D475C1169F078899CD1686A95F`。构建后已用补丁恢复启动代码，`main.rs` 无本次差异。该证据验证英文资源及原生布局，**不代表产品已经提供运行时语言切换**。

### 本次修复、回归与审查

1. 编辑区 Windows 默认只有 Ctrl+Y 重做，Ctrl+Shift+Z 未恢复文本。扩展已有目录移动/文件历史测试后先失败（`saved` 与 `saveddraft` 不符）；给既有 Base Redo 动作补 `Input` 范围的 Ctrl+Shift+Z 默认绑定后通过，继续保留 Ctrl+Y。文件和文本历史仍按焦点分开处理。
2. 冲突及进度取消按钮引用不存在的 `common.cancel`，原生显示该键。复用已有 `explorer.cancel` 双语资源，并用同一显示文字发布原生无障碍标签；从真实冲突等待入口检查两种语言的弹窗与延迟进度取消按钮。修复前测试失败（`common.cancel` 与 `Cancel` 不符），修复后通过；取消仍保留源、目标内容。

| 检查 | 实际结果 |
| --- | --- |
| `cargo test -p editor-app explorer_transfer_ -- --test-threads=1` | 29 通过、0 失败、0 ignored；含两个退出清理用例及本次两项回归 |
| `cargo test -p editor-app shortcuts -- --test-threads=1` | 22 通过、0 失败、5 ignored；未构建该过滤范围中无关的真实插件夹具 |
| `cargo test -p editor-app leave -- --test-threads=1` | 4 通过、0 失败、1 ignored（既有 LSP 日志夹具）；不是全部 UI 测试 |
| `cargo fmt --check` | 初次指出新增断言格式；已按提示修正，最终通过 |
| `cargo check --workspace` | 最终通过；保留既有 warnings |
| `cargo test --workspace --exclude editor-app` | 首次链接失败：找不到已安装的 `msvcrt.lib`。给该进程设置已有 MSVC/UCRT/UM 的 LIB 目录后以 `-j 2` 复跑通过，238 通过、0 失败、188 ignored；未修改或安装全局工具链 |
| `cargo build -p editor-app` | 最终中文及临时英文原生构建均通过；未依赖退役 scripts |
| 两轴独立代码审查 | 固定 `4d172b9` 到本次五文件差异。Standards：0 硬性违规、0 需处理的判断性坏味道；Spec：0 剩余代码发现 |

日志位于上述 `evidence/`。188 项非 UI ignored 和额外 UI ignored 均未报告为通过；本次文件转移验收不依赖实际 WASM 包，不扩大为无关插件夹具构建。未重复完整 UI 套件。

### 剩余原生步骤与平台限制

- **待人工：Windows 系统文件管理器到 Nanobug 的多项跨窗口原生拖入。** 当前 computer-use 的 drag 仅针对单个窗口；未调用自制输入接口，也未用 GPUI 合成事件代替 OS 协议。已打开本任务两个专用窗口，选中 `windows-drag/native-file.txt` 与 `native-folder`，落点为资源树 `03-os-drop`。该目录目前只有 anchor；收到实际拖入反馈后再核对来源保留、目标字节、目录树及操作提示。样例与窗口保留，等待结果。
- 基线中文实例和英文验收实例关闭时，AccessKit Windows UIA 线程曾记录 `ClientToScreen` 的无效窗口句柄 panic。该现象在本次修复前已出现；最终中文退出复验 stderr 为空，退出码 0 且文件操作资源全部清理。英文实例也返回退出码 0、备份清零，但保留其原始 stderr，不把它称为无异常退出。未修改上游依赖，该 UIA 生命周期现象仍需平台专项排查。
- macOS/Linux 没有原生运行环境，仍维持下文实际支持范围；Windows 结果不代表其他平台验收通过。

### 本次交付状态

两项修复已分别普通提交：`19f5b40`（文本 Ctrl+Shift+Z）和 `c3d4461`（取消文案及无障碍标签）。第一项提交还以仅含该 index 的隔离源码导出复跑格式和目录移动/文本历史回归，1 通过；第二项对应已验证的最终五文件代码树。交付分支为 `codex/explorer-native-acceptance`，推送及议题读回结果随后核对。

#78、#80 的本票矩阵已满足，关闭前仍核对推送和议题读回状态；#79、#76 保持打开，等待上述 Windows OS 拖入证据。不得用此次审查、自动测试或已完成的粘贴联合操作替代该缺口。

## 测试入口

以已挂载 EditorApp 的资源树菜单、键盘和拖放事件为入口，检查临时工作区的真实磁盘与原有标签状态。Windows 系统剪贴板及原生拖入另行验收，不以合成事件替代原生协议验证。

## 已执行

- 首个回归测试 `explorer_transfer_paste_conflict_keeps_both` 在基线上失败：同名粘贴没有冲突选择。
- 第一阶段 `cargo test -p editor-app explorer_transfer`：10 项通过，覆盖剪切保留未保存会话、快捷键焦点、同名选择、合并、脏目标保护、失败与取消、Windows 联接跳过及删除失败清理。
- 第一阶段 `cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --exclude editor-app` 均通过。
- Windows 原生验收：在系统文件资源管理器中对本任务临时样例执行 Ctrl+C / Ctrl+X，再在资源树执行 Ctrl+V。复制保留源文件，剪切移除源文件，目录原有内容保留，树更新正常。验收仅涉及 `target/explorer-native/` 下的临时样例。
- 第二阶段 `cargo test -p editor-app explorer_transfer_`：13 项通过，新增树内移动与 Ctrl 复制、外部多项拖到文件父目录、599/600ms 悬停展开、边缘滚动和 Escape 恢复临时展开。首个拖动测试在实现前失败。
- 第二阶段格式、非 UI workspace 测试和 workspace 编译检查通过；真实原生拖入与主题检查留到第三阶段合并验收。
- 第三阶段针对性文件操作测试已通过 18 项，新增菜单撤销部分成功记录的用例也在完整套件中通过。覆盖复制、目录移动、合并、路径占用跳过、强制恢复的未保存内容二次确认及重做新冲突。
- 第三阶段格式、非 UI workspace 测试和 workspace 编译检查通过。`cargo build -p editor-app` 通过。
- 完整 UI 套件并行运行：369 通过、15 失败、142 ignored。失败包含既有共享语言/插件注册状态的用例及菜单标签查找；同一版本以 `--test-threads=1` 串行复验为 384 通过、0 失败、142 ignored。ignored 夹具未在本次任务中构建和运行。
- 上一轮审查修复后的完整 UI 套件串行运行：390 通过、0 失败、142 ignored。随后新增退出清理测试通过；该次 `cargo test -p editor-app explorer_transfer_ -- --test-threads=1` 为 26 通过、0 失败，`cargo test -p editor-app leave -- --test-threads=1` 为 5 通过、1 ignored（既有 LSP 夹具未构建）。
- `7c3b964` 对应的 `cargo fmt --check`、`cargo check --workspace`、`cargo build -p editor-app`、`git diff --check` 通过；非 UI workspace 测试在该轮修复后通过。最后的退出与备份释放修复另按下条记录验证，当前原生验收窗口尚未载入这些最后修复。
- 最后复核新增两项回归：强制撤销移动遇到未保存占用标签，以及 Quit action 退出时仍在等待冲突选择。两项均先失败后通过；最终资源管理器针对性测试为 28 通过、0 失败。会话测试 3 通过，退出相关测试 5 通过、1 ignored。格式、非 UI workspace 测试与 workspace 编译检查再次通过；未重复完整 UI 套件。
- 站点 `npm test`：15 通过、2 跳过（新工作区没有搜索构建产物）；本次仅修改双语资源管理器使用说明。

## 待执行

- Windows 系统文件管理器跨窗口原生多项拖入，等待本次专用窗口的人工操作结果；原生联合操作、主题和最终退出清理已按上文复验，不再沿用历史未执行状态。
- macOS/Linux 原生运行验收没有环境，本轮不宣称通过；实际支持范围见下文。

## 审查修复

以 `git diff main...HEAD` 沿 Standards 与 Spec 分别审查。首轮 Standards 4 项、Spec 4 项，其中普通覆盖授权的空窗为两轴共同发现；逐项处理，不合并两轴结论。

- 磁盘准备、完整内容校验、私有备份和递归清理留在后台。最终发布在一个 UI 回调内检查当前未保存状态、会话身份及 revision，仅执行同卷重命名和标签路径迁移。失败通过重命名回滚，避免在 UI 复制大备份；回滚失败保留停车原件并记录实际变化。
- 强制恢复的丢弃授权绑定 `file_id`、文档 revision 与 capability revision；关闭重开或确认后新输入使旧授权失效，异步读取还检查打开/保存时间。
- 目录恢复维护由已检查状态与本次实际发布结果推导的内容树，拒绝确认期间新增文件；新目录容器不会删除用户跳过的子项，整目录替换可正常撤销、重做。
- 工作区路径重新解析 canonical root，拒绝父目录被联接替换后的重定向。
- 拖动落点使用本帧实际可见行的裁剪布局，自动滚动重绘和释放时重新命中；离开目标失效旧悬停计时，返回后重新开始 600ms。
- 新增回归测试分别复现目录替换无法重做、确认后输入被覆盖、返回目录无法展开，再验证修复。25 项文件操作测试通过，另覆盖目录确认后变动、边缘滚动释放和父目录联接替换。
- 正常窗口退出显式取消后台工作、等待其完成并清理本会话私有备份，避免依赖进程退出的析构。实际粘贴生成备份的退出回归测试通过，重复关闭不能绕过未完成清理，也不删除项目、来源或无关临时内容；Windows 联合验收正在执行。
- 强制移动恢复遇到未保存的占用标签时，最终身份与 revision 校验通过后先关闭该占用会话，再迁移原文档；保持原编辑器身份。新输入仍会使旧丢弃授权失效。
- 原生窗口关闭与 `QuitEditor` 共享同一个清理任务；先取消活动工作、释放冲突等待，等待已启动的历史备份后台释放，再清理资源登记表。关闭期间菜单和恢复入口禁止新任务，晚到结果保留到登记路径已清理后，不与登记表并发删除。
- 新操作废弃 redo、空结果和未保留记录时，在后台释放最后的备份引用，避免 UI 递归删除大型备份。

最终代码提交为 `ae26fa963e32d46c7417d654f11447515b55b393`。两个独立审查均已按 `git diff main...HEAD` 确认：Standards 0 项硬性违规、0 项需处理的判断性坏味道；Spec 0 项剩余代码规格发现。审查通过不代表下述原生验收已完成。

## 平台与原生验收边界

- Windows 原生剪贴板复制/剪切已验收；系统文件管理器跨窗口拖入仍需真实操作验收。当前 computer-use API 的 drag 限定在单一窗口内，越界调用被拒绝，不能用 GPUI 合成拖入冒充 OS 协议通过。
- macOS 使用 GPUI 原生路径及文件 URI 输入，文件 URI 的 `cut` 标记可表示移动；未实现 Finder 原生剪切意图。
- Linux 使用文件 URI 输入与公共原生拖入入口，原生文件剪贴板的支持依赖 GPUI 后端。
- macOS/Linux 未运行原生 UI 验收，未安装额外工具链；Windows 结果不代表其他平台通过。

## 实现参数

- 保留两份沿用原扩展名，依次选择 `名称 (2).扩展名`、`名称 (3).扩展名` 等空闲名称。
- 后台操作超过 350ms 显示进度。
- 文件操作错误使用本地 Notification，直到手动关闭；定位提示仍为 1.2s。
- 会话备份使用项目目录之外的私有临时目录；不承诺跨重启恢复。
