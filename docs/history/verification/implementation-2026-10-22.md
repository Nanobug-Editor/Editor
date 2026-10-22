# 基础文件历史实施与验收记录

状态：基础实现及 Windows 定向验收完成；workspace 测试与编译检查通过，全量格式检查仍有其他任务的差异。日期：2026-10-22。

依据：[已确认方案](../specs/local-history.md)、[H01–H38 验收范围](acceptance-plan.md)。本记录报告真实产品结果；前序 Python 模型没有计作产品验收。未提交、推送或发布。

## 实现范围

- 新增 [editor-history](../../../crates/editor-history/src/lib.rs)：Zstd level 1 独立全量对象、SHA-256 内容去重、SQLite 直接版本索引、持久逻辑身份、结构事件、保护租约、保留与清理、旧快照迁移及有界后台队列。
- [platform-windows](../../../crates/platform-windows/src/history.rs) 保留数据目录、Windows 文件身份证据、安全旧快照访问、缺失父目录准备和原子仅创建适配；旧 LocalHistory 已移除。新库不依赖 GPUI 或平台 crate。
- [宿主集成](../../../crates/editor-app/src/editor/history/mod.rs) 覆盖主保存、实际公开 WASM 保存、运行前保存、图片输入的首次源文件保存、外部观察、重命名/移动/复制与删除。普通历史失败不阻塞正常保存；恢复和删除的必要保护失败则停止。
- 原生界面支持文件、工作区、已删除、无工作区、旧版和全部分类；每页 50 条、路径筛选、只读预览、有界对比、恢复、原字节导出，以及单条/文件/工作区/全部清理确认。文件树、标签和应用菜单均接入入口。
- 恢复前持久化未保存文本，经唯一 EditorState/DocumentSession 作一次可撤销编辑，恢复动作不直接写盘。已删除文件保持待保存，仅创建写入，不覆盖竞争创建的其他文件。
- 用户设置保留 30 天、十进制 1 GB、默认单文件 8 MiB、最高 32 MiB；排除独立于 Git ignore。关闭记录保留浏览、导出和维护。
- 修复退出部分失败后的保存重试：关闭递增保存代数并封闭提交；失败后仅重新允许新保存，旧准备继续失效，其他已停止资源不被重启。

保留 MeEditor 用户数据与所选 profile。没有新增周期草稿采样、目录整体回滚、Git 依赖或云同步。

## 构建来源与工作区边界

实施前 HEAD：fc9dc531ef991315d73e21c8a092f12a0f1ff78b；工作区实施前已有数百项改动，后续还有并行调试功能修改。未把整个 HEAD 差异归为本任务。

实施前相关文件副本位于 target/history-implementation-baseline-20261022。当前 [源码指纹](sources-2026-10-22.json)记录相关文件、基线摘要及共享文件的归属限制。Cargo 输出使用独立 target/history-implementation，已通过 cargo metadata 核对真实 workspace_root 和 target_directory。

Rust/Cargo 1.98.1，Windows x86_64-pc-windows-msvc。新增 rusqlite 0.40.2 bundled，复用 Zstd 0.13.3，没有升级既有依赖。性能程序与原生测试二进制摘要、硬件和缓存条件见[性能记录](performance-2026-10-22.md)。

## 实际检查

以下均从仓库根执行；Cargo 检查使用：

~~~powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location).Path 'target/history-implementation'
~~~

| 命令或实际入口 | 结果 | 证据 |
| --- | --- | --- |
| cargo test -p editor-history -p platform-windows | 52 通过、0 失败、4 ignored | [日志](../../../target/history-storage-final.log) |
| cargo test -p editor-app history -- --test-threads=1 | 52 通过、0 失败、18 ignored；最后使用同一已编译测试二进制直接运行相同过滤词 | [日志](../../../target/history-host-final.log) |
| history_public_wasm_save --ignored --test-threads=1 | 真实固定 capability-example ZIP/WASM 经请求队列保存，1 通过 | [日志](../../../target/history-public-wasm-save.log) |
| a_launch_saves_modified_documents_before_starting | 1 通过 | [日志](../../../target/history-run-save.log) |
| explorer::interaction::tests --test-threads=1 | 40 通过、0 失败 | [日志](../../../target/history-explorer-integration.log) |
| app::projects::tests --test-threads=1 | 46 通过、0 失败、5 ignored | [日志](../../../target/history-project-close-final.log) |
| project_windows_quit_cancel_retains_every_editor_and_revalidates_new_input | 修复退出部分失败后重试保存的回归后，1 通过 | [日志](../../../target/history-project-quit-final.log) |
| cargo test -p platform-windows native_picker -- --ignored --test-threads=1 | 原生文件选择器显式取消、所有者销毁两项通过 | [日志](../../../target/history-native-picker.log) |
| history_native_matrix --ignored --test-threads=1 | 真实 Windows/D3D11，两组独立进程均通过；父矩阵 1 通过 | [回执](native-2026-10-22.json)、[日志](../../../target/history-native-matrix-final.log) |
| release benchmark | 三轮混合负载、六档大文件、2048 变化长历史、近预算组均成功 | [实测报告](performance-2026-10-22.md) |
| rustfmt --check --edition 2024 --config skip_children=true 加本次 Rust 文件列表 | 82 个相关 Rust 文件通过 | [日志](../../../target/history-owned-fmt-final.log) |
| cargo check --workspace | 通过；有既有警告 | [日志](../../../target/history-workspace-check-final.log) |
| cargo test --workspace --exclude editor-app -j 2 | 最终重跑 509 通过、0 失败、375 ignored；包含 editor-ui 控件测试 | [最终日志](../../../target/history-workspace-tests-final-recheck.log) |
| cargo test --workspace --exclude editor-app --exclude plugin-runtime | 295 通过、0 失败、15 ignored，包含 editor-ui 控件；不是完整 workspace 测试的替代 | [日志](../../../target/history-workspace-remaining.log) |
| cargo fmt --check | 当前失败位于本任务之外的原生控件映射和 Java 调试测试格式 | [日志](../../../target/history-workspace-fmt-final.log) |
| git diff --check、文档链接/路径/JSON 检查 | 通过；118 个本地链接、89 项源码指纹、38 条验收映射、全部 JSON 有效 | [文档检查](../../../target/history-docs-check.log)、[差异检查](../../../target/history-diff-check.log) |

ignored 未当作通过。存储输出中两个私有崩溃子用例由通过的父用例带隔离参数显式启动；文件选择器另行运行。宿主 history 过滤词中的实际 WASM 保存和原生矩阵也已另行显式运行，其余 Git/语言等既有 ignored 测试没有计入本次覆盖。

门禁结果与过程中出现的问题：

1. 早期 plugin-runtime/tests/host_execution.rs:958 的 DebugAbilities 初始化缺少并行调试改动新增的 thread_tree 字段，使用 -j 2 后仍曾复现，见[早期失败日志](../../../target/history-workspace-tests-bounded.log)。该初始化随后由对应任务补齐；最终全量重跑 509 通过，本任务未修改该接口或测试。
2. 早期高并发重编译还出现 Windows os error 1455（页面文件太小，mmap 失败）及后续编译/链接错误；限制构建并发后的最终重跑通过。没有清空其他任务的缓存、修改页面文件或安装工具。
3. cargo fmt 的最终差异位于 crates/editor-ui/src/aligned_editor/tests/native_map.rs 与 crates/plugin-runtime/tests/java_package/debugging.rs，共两处其他任务的测试文件。本任务 82 个 Rust 文件独立检查通过，未顺手格式化无关代码。
4. 额外执行的旧综合插件请求测试在后半段插件面板显示断言失败；其前面的保存及历史字节断言已通过。另建的专门历史用例通过真实公开 WASM 请求链路独立验证 H07，没有改动该面板行为来绕过断言。
5. 较早并行宿主测试中的原终端恢复用例发生过超时；隔离运行及最终串行 history 运行均通过。未改终端代码掩盖超时。

## H01–H38 覆盖

表中“通过”指下列具体自动化、原生场景及源码边界验证；未执行的跨平台、OS 候选窗及真实断电不在其声明范围内。

| 编号 | 结果与主要证据 |
| --- | --- |
| H01 | 通过。[对象/时间点测试](../../../crates/editor-history/tests/history.rs)验证 A/B/A、相邻不变、共享对象与独立身份；实测每轮 576 次得到 504 时间点、444 对象 |
| H02 | 通过。关闭重开独立读取、跨范围清理后其他引用仍可读；2048 变化的最旧/最新版本均直接解码，[存储接口](../../../crates/editor-history/src/store.rs)无差分回放链 |
| H03 | 通过。[原始字节契约](../../../crates/editor-history/tests/contracts.rs)覆盖空、中文、多字节、BOM、不同换行及无结尾换行 |
| H04 | 通过。同时间稳定游标、分类、路径和旧路径筛选；损坏载荷不阻止元数据列表；原生 113 条实际分页 50/50/13 |
| H05 | 通过。逻辑身份、删除后独立重建、显式恢复；[真实宿主重命名/移动](../../../crates/editor-app/src/tests/history/observations.rs)保持原标签及 FileId，普通原子保存延续原身份 |
| H06 | 通过。[真实 Windows 部分删除](../../../crates/editor-app/src/explorer/history/tests.rs)逐文件核对实际存在性；文件树传输集成 40 项通过 |
| H07 | 通过。主保存、真实公开 WASM 请求、运行前保存、[图片输入首次保存](../../../crates/editor-app/src/extensions/image_input/history_tests.rs)均检查实际成功字节 |
| H08 | 通过。[主保存测试](../../../crates/editor-app/src/tests/history/capture.rs)保留覆盖前磁盘及成功新内容；内容回退不会因对象已存在而丢时间点 |
| H09 | 通过。真实写入拒绝不产生 Saved 新内容；不同内容零秒合并，相邻未变才复用当前时间点 |
| H10 | 通过。初始化不可用、额度及实际写入故障分别验证；普通保存仍成功，失败可见；保护失败保留缓冲区 |
| H11 | 通过。必要删除保护失败不开始删除；只有点按显式失败例外才允许无保护删除；逆向复制的删除也经过必要保护 |
| H12 | 通过。Git ignore 不屏蔽保护，默认 target 不入库；遍历有界，原生部分删除如实对账 |
| H13 | 通过。[磁盘对账测试](../../../crates/editor-app/src/tests/history/observations.rs)验证干净文档重载、脏文档保留本地输入与冲突、重复观察仅一个状态 |
| H14 | 通过。关闭后外部改写、重新打开补记当前状态；可靠重命名/移动才延续身份，不虚构离线中间版本 |
| H15 | 通过。BeforeRestore 独立持久化并租约保护；同内容也保留保护时间点；额度/策略失败不改变输入 |
| H16 | 通过。[原生恢复测试](../../../crates/editor-app/src/tests/history/recovery.rs)及 Windows 实机流程验证一次 Undo/Redo，无第二份可写文本模型 |
| H17 | 通过。恢复后磁盘保持原字节，正常 Save 才发布；内容恢复保留当前位置，脏状态来自真实保存基线 |
| H18 | 通过。输入、磁盘、取消、只读/IME、活动标签及关闭生命周期均有最终校验；[迟到回执回归](../../../crates/editor-app/src/tests/history/late_results.rs)阻止切换后的比较和已删除恢复 |
| H19 | 通过。只读和活动原生 IME 组合拒绝恢复；真实窗口组合/取消及焦点、Undo/Redo 正常。未声称人工 OS 候选窗测试 |
| H20 | 通过。已删除分类打开待保存原生文档，沿最后位置及旧身份；已有打开文档保留自身时间线并走保护流程 |
| H21 | 通过。[原子创建竞态](../../../crates/platform-windows/tests/history_files.rs)在准备后竞争创建目标，原文件不被覆盖；恢复副本保留来源并使用新身份 |
| H22 | 通过。[原生父目录与路径测试](../../../crates/platform-windows/tests/history_paths.rs)覆盖缺失父目录、根消失、路径占用、越界；预览不创建目录 |
| H23 | 通过。默认 8 MiB、最高 32 MiB 的前一字节/边界/后一字节；比较 1 MiB 和 2000 行独立边界；真实 8 MiB 单版本预览及超限降级 |
| H24 | 通过。UTF-8/BOM、二进制/NUL/文件类型、独立排除、工作区外归属；不合格结构事件清除旧内容提示，不借用过期文本作当前备份 |
| H25 | 通过。[无工作区设置](../../../crates/editor-app/src/app/settings/history/tests.rs)验证非法输入和持久化；真实设置窗口在两种主题/语言/DPI 下应用所选 profile |
| H26 | 通过。关闭停止捕获，原数据可浏览；恢复拒绝绕过保护；定期维护不随记录开关停止 |
| H27 | 通过。[原生旧快照迁移](../../../crates/platform-windows/tests/history_legacy.rs)保留原时间、校验后退休源、重复导入幂等；可靠移动后的旧快照归同一身份 |
| H28 | 通过。未知/有歧义归属保留 raw legacy，不猜路径；33 MiB 二进制旧内容原字节流式导出 |
| H29 | 通过。迁移提交账本后、物理删除前进程退出，重开可重试 GC；损坏不自动清空；迁移不刷新时间 |
| H30 | 通过。注入时间验证精确日历边界；同 profile 共享预算/租约，独立 profile 隔离；旧副本及待回收均计入 |
| H31 | 通过。单条/文件/工作区/全部清理的共享引用与最后引用回收；内容头不形成永久 GC 根 |
| H32 | 通过。准确计数、去重后可回收估计、epoch 防止过期确认；原生预览/取消/确认不改变当前文本或项目文件 |
| H33 | 通过测量覆盖。近预算组维护后实际库低于预算；记录完整库、WAL、SHM、TEMP 与进程工作集，明确采样及暂态限制；见[性能记录](performance-2026-10-22.md) |
| H34 | 通过。活动版本与保护点租约、同 profile 多所有者、捕获/清理串行事务、有界队列及取消计费回归 |
| H35 | 通过。[进程中断](../../../crates/editor-history/tests/crash.rs)只保留已提交完整版本，integrity_check 与 foreign_key_check 正常；另含真实迁移 GC 崩溃恢复 |
| H36 | 通过。[不可用/锁定/只读存储](../../../crates/editor-history/tests/failures.rs)、实际 SQLite FULL 事务注入、队列满及计费上限；宿主三类失败策略分别验证 |
| H37 | 通过。[Windows 大小写、路径别名、junction/reparse](../../../crates/platform-windows/tests/history_isolation.rs)、Unicode、目录穿越、项目内 profile 拒绝及正常 MeEditor/profile 目录边界 |
| H38 | 通过。独立损坏载荷、长度、摘要、悬挂引用、未知 codec、缺失元数据表/外来库均诊断并保留数据；损坏共享对象不能被确认为保护成功 |

## 原生交互证据

真实 WindowsPlatform/D3D11，测试自行拥有临时工作区、profile 与 HWND，结束等待生产清理和窗口退休。没有操作用户其他窗口或数据。

验证组为深色中文 100% 与浅色英文 150%；应用菜单、文件树右键、标签右键入口均通过实际指针事件打开。原生预览、对比、恢复、Undo/Redo、受保护删除、仅创建保存、113 行筛选分页、清理预览/取消/确认和设置持久化均成功。

对比截图只在两侧真实原生字符投影已绘制且与可见源视口相交后采集，同时核对全文、历史只读、当前可写与非零尺寸；代表截图已人工检查像素。早期只等待一帧的截图可能截在后台比较或布局完成前，没有作为最终可见结果证据。

| 可见结果 | 证据 |
| --- | --- |
| 深色中文历史与可写当前缓冲区比较 | [截图](assets/history-comparison-dark-zh.png) |
| 浅色英文 150% 第三页与文件名 | [截图](assets/history-pagination-light-en-150.png) |
| 浅色英文 150% 历史设置 | [截图](assets/history-settings-light-en-150.png) |
| 每步观测与所有截图的几何记录 | [JSON](native-2026-10-22.json) |

全部原始 PNG 与子进程日志保留于 target/history-native-20261022-j。三张代表截图另保存在本目录 assets/，不依赖构建缓存留存。

## Standards

并行只读规范审查覆盖根约定、模块职责、深模块边界、原生 UI、注释、资源生命周期、路径与 Fowler 坏味道基线。发现的问题已修正并复查；最终无剩余规范违例或需报告坏味道。未将其他任务的全量格式差异归为本任务。

## Spec

并行只读规格审查按已确认方案核对。已修复后台活动标签复核、移动后的绑定与迁移归属、保护额度、损坏对象重复引用、维护周期和关闭失败后保存重试等遗漏。最终无剩余规格阻断；测试结果以上述实际日志为准。

两个轴剩余发现数：Standards 0；Spec 0。

## 保留的限制

- 全量 workspace 测试与编译检查已通过；全量格式门禁仍有上述其他任务的两处测试文件差异，需由对应任务处理后复查。
- 8 MiB 单行 debug 原生预览约 2 秒，退出历史后的原文档重绘约 4 秒；取消状态撤销约 0.3 ms。功能、布局与可取消性通过，尚不能声称没有可感知停顿；没有约定或伪造绝对性能 SLA。
- Windows 原生适配已验收；macOS/Linux 的平台无关存储边界保留，但本轮未执行其构建或原生交互，必要的原生安全操作明确 Unsupported。
- 中文组合通过原生输入接口验证，未执行真实 OS 候选窗人工操作。进程强制退出测试不等于真实硬件断电。
- 固定 WASM ZIP 已按现有摘要账本使用；没有升级 SDK、重建第三方插件或将跳过项计作通过。

