# 完整快照与文件变更日志＋旧内容引用：本机比较

状态：调研实验；未实现产品 `editor-history`，未改变保存或文档状态流程。日期：2026-10-22。

这是用户要求补入 IDEA 风格模型后的实验。结果来自统一后端的模型原型，**没有运行 IDEA 或 VS Code 产品测速**。完整源码、样本和前序输入分别见：

- [模型原型与主基准](benchmark_history_models.py)、[21 组主结果 JSON](history-models-benchmark-2026-10-22.json)。
- [长历史查询探针](benchmark_history_lookup.py)、[探针原始结果](history-models-lookup-2026-10-22.json)。
- [共同的原生 Zstd／SQLite 工具与输入生成器](benchmark_storage.py)、[前序实验](storage-benchmark-2026-10-22.md)、[机制的一手依据](storage-methods-sources-2026-10-22.md)。

## 源码核实及建模边界

IDEA 证据固定到提交 `56f1f5c09bece0c124a3684a64c189b4532a1054`：

1. `ContentChange` 持有旧内容引用；回放时将引用设回树节点。查询不需要解压每一个中间版本，定位目标后读取目标内容即可。[ContentChange](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/lvcs-impl/src/com/intellij/history/core/changes/ContentChange.java#L58)、[ByteContentRetriever](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/lvcs-impl/src/com/intellij/history/core/ByteContentRetriever.java#L49)。它不是前序实验的“相邻 Zstd 字典链”。
2. VFS 按内容哈希复用完整对象，默认对**大于 8000 字节**的内容使用 LZ4。不能将 IDEA 简化成未压缩日志，也不能将“日志”与压缩、去重当作互斥方案。[内容复用](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/vfs-impl/src/com/intellij/openapi/vfs/newvfs/persistent/PersistentFSContentAccessor.java#L48)、[默认参数](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/vfs-impl/src/com/intellij/openapi/vfs/newvfs/persistent/FSRecordsImpl.java#L118)。
3. 优先压缩器为 JetBrains 修改的普通快速 LZ4，回退为 `fastCompressor()`，不是 HC。VFS 此路径过阈值后直接存压缩结果，没有“更小才采用”的检查。[实现选择](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/util/src/com/intellij/util/CompressionUtil.java#L27)、[优先实现](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/util-ex/src/com/intellij/util/io/lz4.kt#L115)、[VFS 压缩路径](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/vfs-impl/src/com/intellij/openapi/vfs/newvfs/persistent/mapped/content/CompressingAlgo.java#L140)。
4. 当前 VFS 的 `releaseContentRecord` 是空操作；删除过期历史日志不保证同步物理回收共享 VFS 内容。本实验为了 Nanobug 的容量上限实现可达性回收，这是设计候选，不是 IDEA 实际 GC 测量。[释放实现](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/vfs-impl/src/com/intellij/openapi/vfs/newvfs/persistent/PersistentFSContentAccessor.java#L107)。

VS Code 源码按完整文件克隆条目、保存 JSON 索引。原型取其“独立完整版本”的组织方式，没有重建它的对象文件、克隆优化或 JSON 刷新策略。[VS Code 存储源码](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/workingCopy/common/workingCopyHistoryService.ts)。

## 公平性与输入

- Windows 11 / Ryzen 9 9950X3D / 32 逻辑处理器；Python 3.12.14，SQLite 3.53.1，原生 Zstd 1.5.7，原生 LZ4 1.10.0。LZ4 从官方固定版本源码用本机既有 MSVC 编译，仅写入 `target/`，没有安装工具；C 编码器与 IDEA 的 Java/Kotlin 编码器不是相同实现。
- 复用前序不可变 corpus：`target/history-storage-research-20261022-b/corpus.bin`，SHA-256 `40faca99afd567342b145f1799be67112a6ecac12defdaa35fc5cc1b466efdbd`。12 文件，初始约 8/64/512 KiB，每文件 48 个内容版本，576 次内容捕获。不是从已经变化的工作区重新采样。
- 将文件交错排列，并为每个文件加入一次重命名、一次移动、一次删除、一次显式恢复，共 **624 个操作后状态**、每文件 52 个。所有组都验证同样的内容、路径及存在性。恢复沿用同一逻辑身份；没有把同路径新建的另一文件混成旧文件。
- 同一 SQLite BLOB 后端：WAL、`synchronous=FULL`、4096 字节页、8 MiB 页缓存、禁用 mmap、1000 页自动 checkpoint，每个捕获单独提交。新 head 的内容和指针均在同一事务中持久化，不能让日志组免费使用一个不收费的外部 VFS 缓存。
- 两种模型都保留同样的版本标记与当前 head。相邻完全相同内容复用当前引用；raw 快照仍会为非相邻重复内容存新副本，其他组按 SHA-256 去重。元数据操作复用内容引用，不为一次重命名再复制整份文本。
- 快照：版本直接保存路径、存在性和完整对象引用。日志：保存字段变化掩码、前镜像及当前 head，版本标记不包含目标内容引用。重建从 head 反向回放被掩码标记的元数据；内容只解码一次。三种日志编码另测 next-change 部分索引跳转：目标是操作后状态，取**第一个更晚的相关字段变化**的旧值，无后继时取 head。
- 每方案三次全新数据库、轮内随机顺序，共 **21 组**；最终主采样目录为 `target/history-models-research-20261022-c`。长历史探针在主实验完全结束后独立运行；早期重叠实验及含多余快照查询的试跑均未纳入结果。
- 所有主组写完后 checkpoint、关闭重开并全状态验证，再测热缓存随机读取；没有跨请求解码缓存。写延迟含哈希、必要编码、对象和元数据事务提交、自动 checkpoint，不含输入字节切片与批次末手动 checkpoint。读取时间不含之后的校验、GPUI、diff 或语法高亮。

## 主结果

单位：空间 MiB、时间 ms；空间和 GC 为三次中位数，p95 为三轮原始样本合并后的 nearest-rank。每方案写入样本 1728、结构操作样本 144、每种读策略样本 1872。

| 模型／编码 | 空间 | 内容捕获 p95 | 结构捕获 p95 | 随机读取 p95 | 日志索引跳转读取 p95 | 清理并压实中位数 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 快照／raw 无全内容去重 | 105.594 | 2.350 | 1.403 | 0.786 | — | 511.42 |
| 快照／raw CAS | 90.703 | 1.945 | 0.592 | 0.736 | — | 409.50 |
| 快照／LZ4 CAS | 40.434 | 2.184 | 0.568 | 0.655 | — | 216.73 |
| 快照／Zstd 1 CAS | **26.477** | 2.399 | 0.543 | 0.705 | — | 153.95 |
| 旧引用日志／raw CAS | 90.734 | 2.083 | 0.557 | 0.770 | 0.743 | 427.18 |
| 旧引用日志／LZ4 CAS | **40.465** | **2.114** | 0.578 | **0.680** | 0.689 | 221.43 |
| 旧引用日志／Zstd 1 CAS | **26.508** | **2.233** | 0.596 | **0.669** | 0.660 | 134.80 |

主表的“随机读取”对快照使用直接查询，对日志使用逐文件元数据反向回放。相同内容编码下，全部版本内容对象池相同；两种模型的数据库差异只有 **32 KiB**。这说明旧内容引用本身并不将完整内容变成微小补丁，也不能仅根据日志名称推断空间更省。

Zstd 快照比 LZ4 旧引用日志少约 **34.6%** 空间；Zstd 旧引用日志也获得相同量级收益。记录延迟约 2 ms、完整读取约 0.7 ms，三轮间有波动，不能把这组小差别视为统计显著的产品排名。这里使用统一后端且未运行 IDE，没有证据说某款产品比另一款快。

最新 50 条历史列表在七组间 p95 为 **0.027—0.034 ms**。列表只读取共同元数据索引，不回放内容或解压，因此这部分没有显示某一存储编码的优势。

清理保留每文件 `mark >= 26` 的 312 个操作后状态；日志删除边界及更早事件，再回收不可达完整对象。保留边界是操作后状态，因此只需要更晚事件的旧值，不必保留边界之前的一份文本检查点。GC 后空间依次为 55.824、45.680、22.477、14.336、45.695、22.492、14.352 MiB。

本轮不应与前序编码表直接合并：虽然内容输入相同，本轮增加了结构操作并统一过滤相邻无内容变化的重新写入，raw 空间由此变化。两轮均不是 VS Code 或 IDEA 产品的测速。

## 较长历史的查询代价

独立探针给一个约 8 KiB 文件保留 2048 次交替内容变化，只有两个完整内容对象，用来隔离元数据查询成本。每个距离、每种策略各 100 个随机顺序样本，全部检查路径、存在性和完整内容哈希。

| 距当前状态的变化数 | 直接快照 p95 | 旧引用日志反向回放 p95 | 旧引用日志索引跳转 p95 |
| --- | ---: | ---: | ---: |
| 0 | 0.0156 | 0.0253 | 0.0275 |
| 16 | 0.0141 | 0.0351 | 0.0267 |
| 256 | 0.0143 | 0.1754 | 0.0269 |
| 1024 | 0.0188 | 0.6386 | 0.0293 |
| 2047 | **0.0151** | **1.2440** | **0.0272** |

长距离反向回放增加的是元数据步骤，并非解压 2047 份文本。索引跳转使用三个部分索引分别定位内容、路径、存在性的下一次变化；这是供 Nanobug 评估的查询优化，不宣称 IDEA 实际采用了本实验的 SQLite 查询。

探针的小内容池是人为条件，不代表 2048 个大型唯一版本的冷缓存随机访问，也未测试目录级 ChangeSet 回放或并发写入。

## 正确性与适用结论

- 主实验每库重开后检查 624 个状态，GC 后检查 312 个；日志两种查询策略均检查，合计 **28080 次**路径、存在性及内容重建验证。每库 `PRAGMA integrity_check` 通过。长历史另有 1500 次完整状态验证。
- 独立只读复核再次验证主库保留的 9360 个状态、查询探针的 6144 个状态，并复算 108 组 p50/p95，均通过；GC 后同编码快照／日志九对对象清单逐项相同，均为 216 个对象。GC 前物理镜像未另存，前集合一致依据相同输入、CAS 插入逻辑及 444 对象计数，没有冒称从已被清理的文件逐对象重验。
- 全部方案都通过重命名、移动、删除状态及删除后显式恢复；不能将这些功能误认为只有某一组织方式能够支持。本实验没有覆盖目录子树批量变更、多文件同一 ChangeSet 的交互语义、符号链接、身份别名或崩溃回放。
- 热缓存 SQLite＋原生 C 压缩器是这次比较的范围；没有测实际 IDE、JVM/VFS、GPUI/diff、冷设备、后台维护并发、峰值 RSS、WAL 峰值、设备写入量或真实断电。
- 主结果是机制原型：原生 C LZ4 1.10.0 与 IDEA 的 Java/Kotlin LZ4 不能做绝对吞吐等同；共用 SQLite 也隔离了真实 VS Code 对象文件／JSON 与 IDEA VFS 之间的后端差别。
- **基础单文件历史的条件推荐仍是：独立 Zstd 1 完整对象＋内容去重＋直接版本索引。** 随机读取和过期回收不依赖其他文本版本，查询实现较简单；本次数据在空间上优于 LZ4 对照。这里没有因样本中的微小延迟差异宣称它绝对更快。
- **旧内容引用日志同样是合理候选，且可直接搭配 Zstd 去重对象。** 若产品确认更强调目录、多文件或有意义操作组的历史，可采用这类事件组织，并提供索引跳转／查询投影和真实可达性回收。其压缩、去重与快照可共用同一个平台无关内容存储；两种组织方式并不要求二选一。
- 实验结论本身不定义产品范围。后续已确认的[基础方案](../specs/local-history.md)采用 Zstd 独立全量对象与直接版本索引，明确替代原开发计划的增量／检查点路线；产品功能与验收仍待实施。

## 复现与工具记录

主程序和探针都要求新 scratch 目录；使用前序 corpus，不能把新的工作区输入冒称同一数据。Python 和 Zstd DLL 使用本机已有路径，LZ4 从官方 `v1.10.0` 的 `lib/lz4.c`、`lib/lz4.h` 编译：

- [lz4.c](https://raw.githubusercontent.com/lz4/lz4/v1.10.0/lib/lz4.c)，SHA-256 `9396f7de527bc8435de9c7569fb7998e56545a84b4f3c2d808c0235c01774539`。
- [lz4.h](https://raw.githubusercontent.com/lz4/lz4/v1.10.0/lib/lz4.h)，SHA-256 `26b82efc53d1570f3b54eef02e9c4764c1ad374ff03cac04e2ced5ea4d4c552f`。
- 编译器：`C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe`，参数 `/O2 /LD /DLZ4_DLL_EXPORT=1`；显式指定 VC include/lib 和 Windows SDK `10.0.26100.0` 的 UCRT include、UCRT/UM x64 lib。初次批处理入口引号解析失败，改为直接调用该编译器后成功，没有安装缺失工具。

在仓库根目录，主实验实际成功命令为：

```powershell
& 'C:\Users\Tmiracle\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe' `
  docs/history/research/benchmark_history_models.py `
  --corpus 'C:\Projects\RustProjects\Editor\target\history-storage-research-20261022-b' `
  --scratch 'C:\Projects\RustProjects\Editor\target\history-models-research-20261022-c' `
  --zstd-dll 'C:\Users\Tmiracle\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\git\mingw64\bin\libzstd.dll' `
  --lz4-dll 'C:\Projects\RustProjects\Editor\target\history-change-log-tools-20261022\lz4.dll' `
  --report docs/history/research/history-models-benchmark-2026-10-22.json --repeats 3
```

主实验结束后，将程序名改为 `benchmark_history_lookup.py`，scratch 改为新的 `target/history-lookup-research-20261022-b`，report 改为 `docs/history/research/history-models-lookup-2026-10-22.json`，并去掉 `--repeats` 运行查询探针。重跑时所有 scratch 都需改为尚不存在的目录。
