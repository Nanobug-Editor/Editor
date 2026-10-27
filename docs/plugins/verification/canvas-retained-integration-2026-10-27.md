# Canvas 最终集成验收（2026-10-27）

状态：全部八项验收通过；同一最终 Release 二进制完成 full 30.001 秒和 patch 600.004 秒，真实输入、采样完整性及正常退出均通过。正在交付已验证结果，#131 与总工单 #127 的远端关闭状态待推送、合并后读回。本文记录主分支统一 Editor 与 Canvas 四单的最终组合，前期记录见[保留场景验收](canvas-retained-frames-2026-10-26.md)。中断与退出失败的数据单独保留，不计作通过。

## 源码与工作区

用户要求继续完成剩余工单、验收后关闭总工单，沿用独立工作区实施、全部完成后合并 main 的授权。主分支在恢复时为干净的 `707173c87c46121a8fa46762461ac46b2daf7a45`。其统一 Editor 2 契约与锁定 GPUI Fork `4db6da3927224d2d856b5f3c21b9aa5103eac083` 会影响实际组合，故先在 `C:/Users/Tmiracle/.codex/worktrees/canvas-integration-20261027/Editor` 集成；没有在主目录解决冲突。

Canvas 已交付任务输入为 `4e7f10e000d0eb9fbd3b908848315daa51b39c4d`。集成提交 `f5ac3d07` 保留双方功能；`a5a7bdd1` 的主题缓存与信任撤销测试通过 `600ef5d4` 纳入。用户固定审查点为 `561aa5b2`，同时核对主分支与任务输入，避免把既有改动归为本次新增。

固定包账本共 71 项，逐项 SHA-256 核对通过；任务已有 65 条记录完整保留。主分支五个 Editor 2 输入继续优先用于裸 ID 选择。主分支早期同名 Flow A 候选另存为 `canvas-flow-1.0.0-main-candidate.zip`，原字节未改变；正式 A 输入仍使用已验收的 `canvas-flow-1.0.0.zip`。D 的 Flow 1.3.0、Table 1.2.0、Timeline 1.0.1 保留原构建者、SDK 和摘要，不把新宿主伪记为旧包生产者。

## 补充覆盖与首次集成检查

在原任务源码上，新增公开 Manager 的实际 Timeline 包信任撤销测试通过（1 项，5.03 秒）；取消信任释放资源并封闭旧帧 lease，重新信任产生新实例，旧引用不能消费新请求。日志为原工作区 `target/canvas-d-resume-trust-regression-20261027.log`。

原任务 Release 原生主题测试通过（1 项，1.96 秒）。通过普通主题投影切换亮／暗／暗／亮，同一场景身份、revision 和文本保持不变，实际字形像素颜色随主题改变，原生布局累计数为 5／6／6／7。日志为原工作区 `target/canvas-d-resume-theme-native-20261027.log`。这些结果是新增测试的初始证据，最终集成宿主仍须复跑。

首次 `cargo test --workspace --exclude editor-app --offline --locked -j 8` 完成编译，但新增组合测试的 JSON 缺少必填 `paint` 字段，未进入行为断言；该次失败不能作为通过。日志保留在集成工作区 `target/canvas-integration-workspace-test-20261027.log`。

Spec 审查另发现快照 Editor 动作中的 Canvas 可能接受旧 UI revision 的目标输入。`3f30e1b077f2dcc0eeba2616c4b68f7512403113`（tree `c88d2ed5df2423e66b7fdfe3ace2d8e1c2fd648c`）增加仅判断 UI 作用域的 `is_editor_action`，保留文档授权查询原语义；Snapshot 原生票据和呈现记录的动作 revision 隔离旧输入与旧清理，不伪造文档来源。

公开协议测试实际 RED 为 1 通过、1 失败，失败信息明确为旧目标输入被接纳；GREEN 为 2 通过。运行时生命周期 7 通过，包含旧 Focus(false) 仅消费旧 lease、替代 UI 的新焦点仍能独立清理。日志为 `canvas-integration-editor-actions-red.log`、`canvas-integration-editor-actions-green.log`、`canvas-integration-editor-action-lifetimes.log`。

固定 `3f30e1b0` 的增量复审：Standards 0 项未解决发现，Spec 0 项新增发现，原 P2 已修复。普通 Canvas 跨纯绘制更新的正控继续通过；宿主新增测试在替换文档后、不绘制新帧便派发 MouseUp，证明旧按下不能激活新快照，随后新点击仍正常。

## 集成功能门禁

以下均使用集成工作区独立的 `target`，不是原任务二进制的复用结论。Cargo metadata 已确认源码和输出目录均属于本集成工作区。日志文件位于该目录的 `target/`。

| 检查 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过 | 冻结源码检查 |
| `cargo test --workspace --exclude editor-app --offline --locked -j 8` | 831 通过、0 失败、441 ignored | `canvas-integration-workspace-final-20261027.log` |
| `cargo check --workspace --offline --locked -j 8` | 通过；55.22 秒 | `canvas-integration-check-final-20261027.log` |
| `cargo test -p editor-app --release --bin editor-app --no-run --offline --locked -j 8` | 通过；3 分 22 秒 | `canvas-integration-release-final-20261027.log` |
| Debug `ui::plugin::` | 90 通过、22 ignored；4.42 秒 | `canvas-integration-ui-debug-20261027.log` |
| SDK 导出 | 6 通过；0.61 秒 | `canvas-integration-sdk-20261027.log` |
| 宿主终端 | 38 通过、5 ignored；37.66 秒 | `canvas-integration-terminal-20261027.log` |
| 生产邮箱 | 10 通过；0.61 秒 | `canvas-integration-mailbox-final-20261027.log` |
| SVG 预览跟随文档和未保存编辑 | 1 通过；0.96 秒 | `canvas-integration-svg-preview-20261027.log` |

第一次邮箱过滤误指向实现子模块，得到 0 项；该次不计通过，随后按真实测试名称 `mailbox` 执行 10 项。首次 Release 预编译与修复源码写入重叠，捕获 3 个 `is_editor_action` 不存在错误；源码接口和 workspace/target 已核对，冻结后重复同一 Release 入口通过，没有清理其他工作区产物。

上述功能和原生组合使用 `target/release/deps/editor_app-5978b8e51a333e5a.exe`，当时 132,709,888 bytes，SHA-256 `3be675196593105b8e20c4ba55b7fb4f7f744a56890e8df929a26fb904e51a8a`。随后仅修正 Timeline 测试退出流程，固定为 `5c0184027e8983ec877abd29a56d1865b1ac6d6a`；`cargo fmt --check`、`git diff --check` 和同一 Release 预编译入口再次通过（2 分 32 秒）。最终采样二进制同路径，131,886,080 bytes，SHA-256 `2034e3303aca1021c071a6e0a42fb69c0083647a01612e4334a73d06a97cd628`。两个版本的生产源码及 SDK 相同，故复用上述未受影响模块的门禁，不把旧二进制的运行伪记为新二进制重跑。

按 `sdk_export::digest` 的名称／长度／原始嵌入字节算法核对，当前 94 项 SDK 输入摘要为 `4fb641244caa8cac3a08c0ef6b3584bed8cae01c12de0304bd164d0fcb90e6b4`；无 Markdown。此值属于本次宿主实际嵌入内容，固定客体包仍保留其各自构建时 SDK 摘要。

## 参考环境

2026-10-27 重新读取：Rust/Cargo 1.98.1；Windows 11 Pro 10.0.26200；Ryzen 9 9950X3D（16 核／32 线程）；可见物理内存 64,545,140 KiB；RTX 5080，驱动 32.0.16.1088。当前 WMI 报告显示模式为 **2560×1440、143 Hz**，与前期 3840×2160、144 Hz 不同，故不混用旧 full 基线。最终两份采样 header 的窗口 DPI 均为 **1.0（100%）**，种子 1729，逻辑事件 100,000；GPUI 报告刷新间隔 6,944,444 ns（约 144 Hz），实际呈现约 143.6 fps。60 Hz 指标只作为方案的建议阈值，不把当前显示器伪称为 60 Hz。

## 实际包与原生组合

所有实际包测试显式执行 `--ignored --nocapture --test-threads=1`，使用固定 Flow 1.3.0、Table 1.2.0、Timeline 1.0.1；没有把被跳过项算作通过。

| 实际包接缝 | 结果 | 日志 |
| --- | --- | --- |
| Flow 编辑链 | 1 通过；6.62 秒 | `canvas-integration-wasm-five-20261027.log` |
| Flow/Table 混合发布及编码／展开预算 | 4 通过；24.42 秒 | 同上 |
| Timeline 迟到呈现证明 | 2 通过；6.45 秒 | 同上 |
| Timeline 帧、补丁、信任、更新和三包隔离 | 7 通过；39.64 秒 | 同上 |
| Flow 交换 | 12 通过；92.70 秒 | 同上 |
| Table D 交换 | 5 通过；22.40 秒 | `canvas-integration-table-d-20261027.log` |
| 历史 Table B 原文字素与 Undo | 1 通过；2.59 秒；单独使用 `canvas-table-1.0.0.zip`，保留六格断言 | `canvas-integration-table-b-20261027.log` |
| Git 0.1.45 滚动增量与旧点击拒绝 | 1 通过；5.65 秒 | `canvas-integration-git-scroll-20261027.log` |

功能冻结时的同一 Release 二进制（`3be67519…`）的 8 个原生场景全部通过：基本图形、透明图层、图片像素、保留段落／真实主题变化、双轴坐标、125% 深色 Flow 中文 IME/UIA、200% Table 中文 IME/UIA，以及系统剪贴板。日志前缀为 `canvas-integration-native-`，由原生窗口运行；父子进程各输出一次结果，不重复累计为 16 个场景。后续测试退出修复不改这些场景及生产代码。

Git 原生 `git_native_history_scroll_probe` 通过（19.58 秒），覆盖 `dark-zh-CN-100` 与 `light-en-150`，均读回 `shutdown_complete: true`。使用独立测试仓库和 profile，未运行完整无关 Git 矩阵。已查看两个实际 D3D11 截图：编辑文本、历史节点与明细均有可见内容。可检查[深色截图](canvas-retained-integration-2026-10-27/git-dark-zh-CN-100-history-scroll.png)、[亮色截图](canvas-retained-integration-2026-10-27/git-light-en-150-history-scroll.png)，各自的 `evidence.json.gz`、`native.log.gz` 与图片同目录保存。

## 实机采样、退出修复与完整性

前置 smoke 再次通过：可见帧、最小化暂停、业务定时继续、恢复、停止后 300ms 零 draw，以及真实双轴滚轮和像素变化均进入断言。第一次 full 基线在 **0.405 秒、0 次测量期滚轮输入** 时检测到窗口失去前台并失败，未启动 patch 600 秒。该样本 DPI 为 100%，不能用于性能或长期稳态通过结论。

原始数据保存在 [0.405 秒中断 JSONL](canvas-retained-integration-2026-10-27/full-0.405s-interrupted.jsonl.gz)，对应运行日志为 `canvas-integration-full-30s-20261027.txt.gz`。[功能与失败证据账本](canvas-retained-integration-2026-10-27/functional-evidence.json) 保留命令日志的原始／压缩摘要、测试结果以及完整采样 header/summary。所有压缩数据解压后均为原始字节，零测试和失败尝试也保留；没有替换前期的 20/40 秒中断记录。

另一授权验收聊天确认第一次采样协调期间没有执行工具、激活窗口、注入输入或运行编译；不据此推断前台丢失的具体来源。用户随后明确答复“现在可以，立即重试”。第二次 full 实际测量 **30.014 秒／75 次输入** 完成，但在窗口退出时触发 AccessKit `ClientToScreen` 无效 HWND、不可展开 panic，父进程退出码 101，因此仍判失败。旧二进制的 1.002 秒和 30.010 秒退出诊断随后通过，说明问题是间歇性的，不能用重试通过抹去失败。

`5c018402` 只修改测试生命周期：先停止 Actor 发布线程，撤销场景输入，blur 后完成两次真实绘制帧，移除窗口并有界等待原 HWND 实际销毁，再释放保留实体并正常退出。没有禁用 UIA、捕获或忽略 COM panic、强行终止进程，也没有修改测量逻辑或生产宿主。原始测量错误与清理错误共同保留。Standards／Spec 对此提交复审均无未解决发现。

最终使用新二进制先运行 **full 30.001 秒／75 次真实滚轮输入**，正常退出后再运行 **patch 600.004 秒／1,500 次真实滚轮输入**；父子进程均正常退出，原 HWND 销毁检查通过。每 400 ms 注入滚轮，按 9.6 秒周期双轴往返。全程保留前台检查，未运行 Cargo 或其他桌面测试；结束后已发送恢复桌面测试通知。两份记录的 `frame_trace_complete` 和 `publication_records_complete` 均为 true，缺失字节记录和队列溢出均为 0。full 有 4,311 对 draw/present，patch 有 86,156 对；原始行数与 summary 一致。

本轮原始成功／失败 JSONL、运行日志、重新编译日志、摘要及分析结果见[性能证据账本](canvas-retained-integration-2026-10-27/performance-evidence.json)。退出码来自实际命令结果，不能仅凭写在窗口关闭前的 JSONL summary 推断成功。观测环的计数表示运行时详细观测被省略／淘汰，不是生产输入丢失，也不等同于 GPUI profiler 每秒轮换；发布字节完整性独立由累计 calls 与已读 publication 数核对，扣除的预热基数 full 为 345、patch 为 349。

## 性能与十分钟稳态

下表为最终 GPUI HDR 全程统计，单位 ms。draw 是 CPU 绘制工作；dirty-to-present 含等待；呈现间隔反映帧节奏，不能互相替代。输入指标从原生派发到绘制反馈，不含硬件和 SendInput 排队延迟。

| 指标 | full 30 秒 p95 / p99 | patch 600 秒 p95 / p99 | patch 最大值 |
| --- | --- | --- | --- |
| draw | 0.774 / 1.020 | 0.783 / 1.020 | 2.300 |
| dirty-to-present | 7.504 / 7.778 | 7.487 / 7.778 | 63.570 |
| 呈现间隔 | 7.545 / 7.852 | 7.528 / 7.856 | 63.570 |
| 原生输入到绘制反馈 | 6.652 / 6.951 | 6.832 / 7.229 | 47.120 |

本参考环境达到方案建议的帧 p95≤16.7 ms、p99≤33.3 ms 和输入 p95≤50 ms；仍保留最大 63.570 ms 的单次等待／间隔尖峰，不宣称每一帧均低于阈值。输入 HDR 分别 74／1,499 项，`input_mid_draw_excluded=0`；不把发送次数直接当成延迟样本数。原始呈现动画间隔各有 53 个 null，按未测处理；另从相邻实际 present 时间计算完整间隔，patch p95 7.514 ms、p99 7.839 ms，结果一致。

同机前 30 秒发布量用一秒计数边界线性插值估算：full **146.550 MB（4.885 MB/s）**，patch **8.307 MB（0.277 MB/s）**，patch/full 为 **5.668%**，减少约 **94.33%**。两个模式都复用生产保留缓存，仅发布方式不同，不能据此声称相对历史无缓存宿主的加速。完整实测总量分别为 146,554,782 bytes／30.001 秒和 169,509,655 bytes／600.004 秒，不能直接用二者总量比较效率。每次仅发布视口与 overscan，缓存最多 349 项，没有序列化全部 100,000 条逻辑事件。

稳态使用同一 9.6 秒输入周期内的六个完整往返周期；下表驻留均值来自各区间的实际一秒样本，累计工作速率采用区间边界插值估算。分析同时保留真实采样端点及其差分速率；没有相减或平均累计 p95。

| 指标 | 57.6–115.2 秒 | 288–345.6 秒 | 537.6–595.2 秒 |
| --- | --- | --- | --- |
| RSS 均值（MiB） | 124.051 | 124.016 | 124.629 |
| private commit 均值（MiB） | 114.528 | 114.144 | 114.748 |
| 缓存项均值（区间范围） | 329.84（320–349） | 330.68（320–349） | 329.16（320–349） |
| 实例资源 / 原生顶点 | 2 / 2,022 | 2 / 2,022 | 2 / 2,022 |
| 发布 bytes/s | 281,522 | 283,005 | 283,131 |
| 准备 / 复用项每秒 | 223.23 / 21,147.41 | 223.61 / 21,240.32 | 223.77 / 21,197.76 |
| 塑形 / 栅格化每秒 | 32.449 / 0 | 32.499 / 0 | 32.525 / 0 |

三个区间 RSS/private 的区间内趋势均下降；末区间相对首区间均值只增加约 0.578／0.221 MiB，结合驻留项、资源和工作速率保持稳定，未观察到持续累积。全程一秒观测 RSS 峰值 131,436,544 bytes、private 峰值 121,049,088 bytes，结束时分别 130,367,488／120,000,512 bytes。队列容量 128，全程高水位 10、溢出 0；该值属于夹具 Actor 入口高水位，不冒充生产 WorkSender 的即时占用，生产邮箱另有 10 项门禁覆盖。

该时间轴使用原生段落／图形，派生像素、派生纹理和栅格化计数均为 0；原生 GPU 字形 atlas 字节数未测，不能把派生纹理 0 写成全部 GPU 内存 0。一秒峰值不是 OS 绝对峰值。累计发布、准备、复用和塑形计数随持续工作增加，稳态判断使用区间增量速率及当前驻留量。结论限定为本 Windows 参考机与本次 600 秒范围，macOS/Linux 原生交互不在本次验收范围。

## 验收判定

| AC | 当前判定 | 证据与余项 |
| --- | --- | --- |
| AC1 原子发布 | 功能验收通过 | 协议、实际 presented/retained/hybrid、Editor 动作修订隔离 |
| AC2 复用与回收 | 功能验收通过 | 主题／字体／DPI／几何原生测试，实际 trust/disable/update 及资源 lease |
| AC3 可见显示帧 | 功能验收通过 | 本次真实 smoke 的隐藏／恢复／业务继续／idle 零 draw |
| AC4 有界队列 | 功能验收通过 | mailbox 10 项、实际 IME/键盘/交换及旧 held-press 回归 |
| AC5 双轴映射 | 功能验收通过 | 双轴原生场景、缓冲和 identity 单测、真实 smoke 的双轴移动 |
| AC6 十分钟稳态 | 验收通过 | patch 600.004 秒、1,500 次双轴输入，资源、驻留、队列及区间工作速率有界，记录完整且正常退出 |
| AC7 参考机性能 | 验收通过 | 相同新二进制、参考环境及种子的 full30/patch600；p95/p99 达到建议门槛，保留尖峰、原始数据及观测边界 |
| AC8 组合与旧消费者 | 功能验收通过 | 三包实际组合、Flow/Table 原生、Git 实际包及两种原生场景、终端与图片／SVG 回归，SDK 和包账本一致 |

AC1～AC8 均已满足。用户已授权验收完成后合并 main 并关闭 #131/#127；交付提交、推送结果和远端状态在实际执行后补记。
