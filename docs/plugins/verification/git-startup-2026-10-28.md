# Git 重启首屏与并行插件启动

日期：2026-10-28。状态：修复与打包完成；首屏与并行回归通过，扩展原生验收保留下述限制；尚未安装到用户配置。

## 问题与边界

用户反馈重新打开宿主后，Git 分支树已出现，提交列表仍长时间空白；要求同时考虑多个插件启动时的相互影响，允许独立插件并行加载。

- 宿主工作区：`C:/Users/Tmiracle/.codex/worktrees/git-diff-scroll-fixes/Editor`，本轮开始同步到主分支 `fbeb058a`，原工作区无未提交修改。
- Git 独立源码：`C:/Projects/RustProjects/Git-for-Nanobug`，起始版本 0.1.46，本轮候选 0.1.47。该目录无 Git 元数据，修改前源码和摘要保存在任务证据目录。
- 证据：`C:/Users/Tmiracle/.codex/task-artifacts/git-startup-2026-10-28`。不重启或覆盖正在运行的用户宿主，不安装候选包。

## 诊断与修复

Git 显示面板时会启动可抢占的本地观察。此前通用输入分类把原生尺寸、字体测量、焦点等通知也当作业务操作，清除了正在运行或排队的查询。初次刷新又没有保存首屏需求，后续布局补页只接受已有权威页面，故可能一直等到下一次 30 秒定时器才重新读取。

- 被动原生回执与焦点、可见性、保存观察不再抢占查询；后者沿既有待刷新标记合并。
- 初次未接受的页面保留需求，真正的用户操作抢占后，串行业务结束时立即恢复第一页。
- 接受空页面也算完成；失败不无限重试；已加载历史的选择、父提交比较、分页和原生滚动保持原有约束。

直接只读测量当前主项目三次：首 128 条 `git log` 为 55–60 ms，refs 约 49 ms，status 59–63 ms，HEAD 暖读 45–48 ms（首次 112 ms）。这些是本机查询样本，不是完整应用启动耗时或屏幕帧率。

宿主原先已支持最多四个独立准备任务并行，但按标识选取任务可能让后台初始化占满所有槽位。本轮按活跃需求选择准备任务：可见面板／命令优先，其次为依赖服务、语言需求与后台观察。最多三个普通启动任务同时准备，剩余容量留给交互；服务依赖也可使用该容量，避免依赖链被预留槽自身阻塞。仍保持最多四个准备任务，不取消已运行任务，不改变各请求的期限、资源归属和取消范围。实现位于通用 runtime，不按 Git 或其他插件 ID 分支。

Windows 原生验收还发现字体测量的另一条丢失回执链：宿主已排队旧版本 `TextMeasured`，actor 在接收前发布了新版本并拒绝旧回执，但画布的样本缓存仍认作已交付。现在字宽缓存与回执版本分开：相同样本／字体保留测量结果，新页面仍请求样本时按新页面版本重发；普通重绘不重复发送，插件撤销请求后停止。保持精确版本校验，不重放旧业务输入。

首次可见之后的截图复查发现原生 `Resize` 也可能丢失，导致拓扑填满窗口而文本只按极小的旧高度生成几行。增加了私有宿主投递回执：actor 仍按原版本调用公开 `Manager::event`；只有 `StaleRevision` 请求同一个原生控件失效同一尺寸缓存，再由实际布局构建新通知。该通道只允许普通／投影视口的 Canvas Resize，退休、成功及其他错误不重试；连续通知保留原合并与队列配额，窗口或实例被替换后不会复活旧控件。没有新增插件专用 API。

## 验证记录

- `guest-startup-red.log`：布局取消和真实操作抢占后的首屏续接两项确定性回归失败；空结果／失败收敛对照通过。
- `guest-tests-final.log`：全量 212 项插件测试通过；`guest-startup-final.log` 中四项针对性回归通过。
- 初次完整插件测试发现刷新误重置已有多父提交比较选择，已将首屏待办限定到没有已显示记录且没有已接受页面的情形，原比较回归恢复通过。
- 初版真实包探针只识别 `Paint::Text`，遗漏实际使用的 `Paint::TextBox`；`startup-package-red*.log` 和最初的 `startup-package-green.log` 不作为内容可见性的红绿证据。修正探针后重新验证旧包与候选。
- `startup-package-content-red.log`：同一修正探针读取旧 0.1.46 包，首轮查询结束后首行仍不可见；`startup-package-content-green.log`：0.1.47 两项实际包回归通过，覆盖重开、连续尺寸／焦点／可见性通知和空仓库，使用独立临时配置与 Git 仓库，没有手动刷新或等待 30 秒定时器。
- `activation-scheduling.log`：三项调度回归通过；`activation-parallel-green-final.log`：两项实际 frozen SDK 包回归通过，分别持有三个未完成后台／语言准备任务，验证第四个可见面板完成、排队优先级与取消隔离。准备租约模拟慢任务，不冒充多插件 CPU 性能基准；首次重跑发现测试清理重复取消已完成请求，修正后完整通过。
- `text-measurement-red.log`：确定性原生控件回归在 0.01 秒内复现新版本丢失测量回执。初次 Windows 流程中首屏可见检查通过，随后列宽测量等待失败；该失败记录保留在 `native-startup/`，不计为完整原生验收通过。
- 首轮 `cargo fmt --check`、`cargo test --workspace --exclude editor-app`、`cargo check --workspace` 均通过，见 `gates.json`。追加测量修复后的复验另行记录。
- `text-measurement-green-final.log`：两项字宽测量回归通过；`native-startup-final/` 中深色中文 100% 与浅色英文 150% 的首行、列拖动和滚动验收通过，但截图发现尺寸问题，因此这轮不作为最终首屏覆盖率结论。
- `native-geometry-red/light-en-150/evidence.json`：加严为文字覆盖真实可视区域后，修复前的 150% 原生场景失败（后台查询已完成、原生场景已当前，文字区域仍未填满）。深色用例未触发，属于启动交错相关缺陷；保留截图与失败清理记录。
- `resize-delivery-tests.log`：八项相关 UI／尺寸回归通过，包含同一尺寸跨页面被拒绝后的原生重测、普通与投影 Canvas、成功后停止、非尺寸业务输入排除和连续通知合并；两个需单独夹具的既有 ignored 测试不算通过。
- 诊断构建 `cargo test -p editor-app --no-run` 同时触发既有 `svg_preview_render` 示例的私有模块路径／`Paint::TextBox` 匹配错误；本次未修改该示例。后续明确使用 `--bin editor-app` 构建和运行宿主测试，工作区规定检查保持独立记录。

## 最终检查与限制

- `gates-complete.json`：`cargo fmt --check`、`cargo test --workspace --exclude editor-app`、`cargo check --workspace` 均通过。非 UI 工作区共 841 项通过、445 项 ignored；ignored 不作为通过。
- `canvas-tests-complete.log`：12 项画布、字宽、列拖动、原生滚动测试通过，5 项 ignored；`resize-delivery-final.log`：8 项尺寸回归通过，2 项 ignored；`mailbox-tests-complete.log`：10 项队列配额、合并、撤销与恢复测试通过。新源文件均纳入格式检查。
- Git 独立源码 212 项单元测试、两个实际 ZIP 重开回归、三项调度单测和两个实际 SDK 并行／取消隔离回归通过，详见上述日志。
- `native-startup-complete/`：深色中文 100% 完整滚动诊断通过并正常退出。浅色英文 150% 的首次内容、刷新后可视区覆盖、列拖动、滚轮增量及选择隔离通过，随后滚动条动画采样未捕获中间宽度，整组父测试失败。实际首屏从首次原生场景至完整内容分别为 883 ms／851 ms；这是本机小型夹具的观察，不是宿主进程启动耗时或用户仓库性能承诺。
- 相同冻结测试程序、相同候选包在停止编译后原样重跑，`native-startup-idle-rerun/` 的深色场景连同动画检查通过；随后退出期间 `accesskit_windows 0.34.0` 在 `util.rs:380` 因 HWND 已失效触发 `HRESULT(0x80070578)`，进程中止，父测试没有继续浅色场景。即使内部 `shutdown_complete` 已写为 true，也不把失败的进程结果算作完整验收通过。
- 两次扩展验收的失败记录均保留；没有修改动画时长、关闭无障碍功能或放宽断言来隐藏结果。这些退出／采样问题与本次首屏修复的关联尚未确证，本轮不扩展为依赖升级或退出系统改造。
- 未运行全部 ignored 套件、全部八种原生组合或用户日常 profile。用户进程和已安装插件未被替换；主分支保持原内容，修复保留在本任务工作区。

## 交付

最终宿主来自本任务 worktree 的独立 `target/release/editor-app.exe`，`cargo build -p editor-app --release` 成功，日志 `host-release-complete.log`。现有编译警告保留，没有以批量修复方式扩大范围。

| 产物 | 路径 | SHA-256 |
| --- | --- | --- |
| Windows x64 便携宿主 | `C:/Users/Tmiracle/.codex/task-artifacts/git-startup-2026-10-28/delivery/Nanobug-startup-fix-2026-10-28-x64.zip` | `ea7785690df8beced5a8f01c95d0c93b88d358d1c3c3cea831f3d7e3f67b7cee` |
| Git 0.1.47 | `C:/Projects/RustProjects/Editor/dist/git/git-0.1.47.zip` | `c6b5ef1ce2275735e7695a938fb01dc04e81e0176d176e0f41859947277e3b47` |

宿主 ZIP 仅含 `Nanobug.exe`、品牌图标和 Alacritty 许可；PE 架构为 x64。归档内 EXE 与最终 Release 的摘要相同：`b7d49f85d528c23cd3bb86f7b6a3660c9d48c0750964897dd82e4aaa0e7cdfe3`。构建来源及完整载荷清单见证据目录 `delivery/build.json`。Git 清单、Cargo 版本和 ZIP 均为 0.1.47，协议仍为 7，没有增加权限。

用户手动关闭原宿主后替换便携宿主程序，再从插件管理器安装 Git 0.1.47，使用原项目与原配置重开验证。无需删除 profile、插件私有数据或项目文件。
