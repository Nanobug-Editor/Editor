# 文件历史存储方法：一手来源与比较边界

状态：调研；没有形成已确认的产品规格或实施方案。

调查日期：2026-10-22。范围：不依赖 Git 的文件历史存储机制，供 Nanobug 的本机微基准和后续设计选择使用。本记录没有运行各参考系统，也没有安装工具或修改产品代码。

## 先分清比较维度

| 维度 | 要回答的问题 | 与其他维度的关系 |
| --- | --- | --- |
| 捕获与版本语义 | 保存、未保存编辑、外部变化、重命名、删除何时形成版本？ | IDEA 的事件和 ChangeSet 模型属于此层；它的内容变更可以引用完整旧内容，不等于文本增量编码。[事件分发](https://github.com/JetBrains/intellij-community/blob/master/platform/lvcs-impl/src/com/intellij/history/integration/LocalHistoryEventDispatcher.kt)、[ContentChange](https://github.com/JetBrains/intellij-community/blob/master/platform/lvcs-impl/src/com/intellij/history/core/changes/ContentChange.java) |
| 内容表示 | 完整对象、相邻 delta、分块清单或操作序列？ | 决定内容的写入、重建及依赖关系；可以额外使用压缩和内容寻址。[Fossil delta](https://fossil-scm.org/home/doc/trunk/www/delta_format.wiki)、[restic 设计](https://restic.readthedocs.io/en/stable/100_references.html#design) |
| 索引与事务 | 如何按文件身份、时间和版本分页查询，并原子提交元数据？ | SQLite 是可用容器，BLOB 中仍可放全量、压缩数据或 delta；选择 SQLite 不等于选择某种内容编码。[SQLite BLOB 比较](https://www.sqlite.org/intern-v-extern-blob.html)、[事务与 WAL](https://sqlite.org/wal.html) |
| 物理后端 | 单独对象文件、数据库 BLOB、pack、文件系统共享块？ | 会影响小文件开销、同步、空间回收和平台限制，不能用内存编码测试替代。[SQLite 文件比较](https://www.sqlite.org/fasterthanfs.html)、[ReFS block cloning](https://learn.microsoft.com/en-us/windows-server/storage/refs/block-cloning) |

以下“推导”是根据机制对编辑器场景作出的工程判断，不是来源中测得的 Nanobug 性能。

## 1. 独立压缩全量对象＋内容寻址去重

- **来源事实**：Zstd 提供无损压缩及可调速度/压缩率；其公开主表使用 Silesia 数据集和内存基准。小数据压缩另有字典机制，依赖样本相关性。[Zstd 官方 README](https://github.com/facebook/zstd/blob/dev/README.md#benchmarks)、[固定版本 v1.5.7](https://github.com/facebook/zstd/blob/v1.5.7/README.md#benchmarks)
- **来源事实**：restic 用内容标识和不可变对象组织数据，并另用索引定位对象；内容寻址与压缩是可组合的机制，不局限于某一种版本编码。[restic 仓库格式](https://restic.readthedocs.io/en/stable/100_references.html#repository-format)
- **推导**：每个不同的完整内容独立压缩，版本元数据引用对象。同内容再次出现可复用同一对象；只有局部相似、整体不同的文件仍需新全量对象。获取任意版本无需回放它之前的版本，历史列表也无需解压内容。两版本比较仍需解码双方并计算 diff。
- **推导与限制**：每次不同内容通常都需扫描、哈希和压缩全文件；共享对象的回收需要引用或可达性判断。若使用共享压缩字典，还须版本化、保留字典，不能声称对象完全无外部依赖。是否比 delta 更快、更省空间，取决于文件大小、文本重复性、版本频率及磁盘提交成本。

## 2. 相邻增量＋全量检查点

### 字节补丁编码

- **来源事实**：Fossil delta 用“从基线复制字节范围”和“插入字面字节”重建目标，格式含目标大小与校验和；这是字节级 delta，既不是编辑器操作日志，也不要求逐行文本 diff。[格式规格](https://fossil-scm.org/home/doc/trunk/www/delta_format.wiki)、[编码算法](https://fossil-scm.org/home/doc/trunk/www/delta_encoder_algorithm.wiki)
- **来源事实**：Git 的 pack 文档明确将 delta 链深度与解包性能联系起来，说明不能仅按 delta 体积优化；此处只借用编码机制证据，不提议让文件历史依赖 Git。[git-pack-objects 的 window/depth](https://git-scm.com/docs/git-pack-objects#Documentation/git-pack-objects.txt---depthltdepthgt)
- **推导**：若每 K 个版本建立一次全量检查点，并使用前向相邻 delta，单版本恢复最多回放 K−1 个 delta；K 是待测参数，不是现有产品约定。反向 delta 可让最新版本直接读取，但会改变旧版本访问和追加写入成本。
- **推导与限制**：适合内容变化局部而版本很多的负载。小 delta 不代表低延迟：生成 delta 要比较基线，随机浏览旧版本要解码依赖链；删除过期基线前要保留依赖或重建检查点。基线/链损坏会影响依赖版本；链长度、恢复缓存、压缩后体积及重建时间都要计量。
- **基准注意**：只找共同前后缀、中间整体替换的简化 delta，没有测试 Fossil 这类多段 copy/insert 编码；结果必须按实际实现命名。

### 参考版本作为 Zstd 字典

- **来源事实**：Zstd 1.5.7 的 `ZSTD_compress_usingDict` 明确允许任意数据段作为字典，解压必须提供完全相同的字典；不要求先用训练器生成字典。官方同时指出每次加载字典有启动成本。[固定版本公开 API](https://github.com/facebook/zstd/blob/v1.5.7/lib/zstd.h#L890)
- **推导**：可以把前一版本或某个完整检查点的原始内容作为字典，压缩当前版本。这是合法的参考版本差分方案，但输出是依赖该参考内容的 Zstd frame，不能命名为已测试 Fossil/VCDIFF 补丁，也不是一份可独立解码的压缩全量对象。
- **推导与限制**：前一版本作字典会形成依赖链；统一引用组内完整检查点可把依赖限制为一层，但与检查点差异逐渐增大时，压缩率可能变化。两者必须记录精确基线身份、编码方式和目标内容校验；基线自身若也需解码，应计入首次随机读取成本。CAS 复用对象时也须保留其原有编码及基线依赖，不能只保存目标哈希便假定可解码。

## 3. 内容定义分块（CDC）＋分块寻址去重

- **来源事实**：restic 用 Rabin Fingerprints 进行 CDC，文件以有序 blob 引用重建；默认小于 512 KiB 的文件不分块，目标平均块约 1 MiB。插入、删除内容后，未变化部分仍可复用。[restic 的备份与去重](https://restic.readthedocs.io/en/stable/100_references.html#backups-and-deduplication)
- **来源事实**：Borg 区分固定分块和内容定义分块；后者的边界由滚动哈希确定，去重身份另用强哈希/MAC。更细的块会增加索引、引用和缓存数量；官方同时给出这些结构的内存成本模型。[Borg 数据结构与分块](https://borgbackup.readthedocs.io/en/stable/internals/data-structures.html#chunks)、[索引/缓存内存](https://borgbackup.readthedocs.io/en/stable/internals/data-structures.html#indexes-caches-memory-usage)
- **推导**：版本直接引用自己的有序块清单，无需沿时间链回放；大文件局部变化、跨文件重复和移动后的相同内容可共享块。恢复需读取并拼接块，随机 I/O、索引缓存、压缩块大小和 pack 布局都会影响延迟。
- **推导与限制**：备份工具的大块默认值对小源码文件可能近似全文件去重，不能直接推论适合编辑器。调小块尺寸可增加局部复用，也增加哈希、清单、对象/索引及垃圾回收开销。固定分块基准不能冒称 CDC；单字节前插、分散修改与完全改写要分别测试。

## 4. 操作日志与持久化写时复制（COW）

### 操作或事件日志

- **来源事实**：IDEA 把结构变化组织为 ChangeSet；`ContentChange` 保留旧内容，`StoredContent` 通过 VFS content id 读取内容字节；历史重建反向应用相关变化。因此“有变化日志”不能推出“只存文本编辑指令”。[ContentChange](https://github.com/JetBrains/intellij-community/blob/master/platform/lvcs-impl/src/com/intellij/history/core/changes/ContentChange.java)、[StoredContent](https://github.com/JetBrains/intellij-community/blob/master/platform/lvcs-impl/src/com/intellij/history/core/StoredContent.java)、[反向回放](https://github.com/JetBrains/intellij-community/blob/master/platform/lvcs-impl/src/com/intellij/history/core/LocalHistoryFacade.kt#L128)
- **后续核实**：IDEA 固定提交 `56f1f5c09bece0c124a3684a64c189b4532a1054` 的 VFS 内容按哈希复用，默认对大于 8000 字节的内容使用普通快速 LZ4；这层可以与旧内容引用日志组合。反向回放替换的是内容引用，不要求解压每个中间完整版本。[压缩默认值](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/vfs-impl/src/com/intellij/openapi/vfs/newvfs/persistent/FSRecordsImpl.java#L118)、[引用回溯读取](https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/platform/lvcs-impl/src/com/intellij/history/core/ByteContentRetriever.java#L49)。统一后端实验见[模型比较](history-models-benchmark-2026-10-22.md)。
- **推导**：真正的插入/删除日志可以直接消费已经校验的编辑事件，减少每次重新比较全文的工作；但外部程序改写、重载及缺失事件仍需快照或差分兜底。必须定义基线、事件顺序、revision、事务边界、检查点及截断恢复。日志短不保证任意历史版本能快速重建。

### 持久化结构共享

- **来源事实**：LMDB 的持久化 B-tree 页使用 COW，读事务可保持旧视图；旧页随后会被复用，长读事务阻止复用并可能造成快速增长。[LMDB 官方头文件与注意事项](https://github.com/LMDB/lmdb/blob/mdb.master/libraries/liblmdb/lmdb.h#L17)
- **推导与限制**：持久化 Rope/树可让多个版本共享未改节点，但必须明确保留版本根、持久化页/节点、回收与格式迁移。数据库提供 MVCC 或内存 Rope 提供 COW，都不自动等于可保留 30 天的文件历史。把每个版本的整份文本塞进普通 BLOB，也不自动获得文本级结构共享。
- **Nanobug 边界**：这类设计不得另建第二份可变文档或 Undo 栈；若要利用编辑事件，需沿现有 `EditorState`/`DocumentSession` 的唯一状态接缝集成。[仓库协作约定](../../../AGENTS.md#文档状态与原生-ui)

## 5. 文件系统克隆与卷快照

- **来源事实**：ReFS block cloning 主要更新元数据并共享物理簇，后续写共享区域时分配新区域。源、目标必须在同一 ReFS 卷，范围须按簇边界对齐，另有区域和文件属性限制。[Microsoft ReFS 文档](https://learn.microsoft.com/en-us/windows-server/storage/refs/block-cloning)
- **来源事实**：VSS 协调 requester、writer、provider 建立卷的时间点视图；系统 provider 用 COW 在覆盖前保存旧块，其他 provider 可采用不同机制。[Microsoft VSS 文档](https://learn.microsoft.com/en-us/windows-server/storage/file-server/volume-shadow-copy-service)
- **推导与限制**：适合满足环境条件时优化磁盘数据的备份/克隆。它们不自动捕获编辑器中尚未写盘的内容，也不提供文件历史版本语义和 UI 索引。项目与用户历史目录可能不在同卷，且不能要求所有用户使用 ReFS；VSS 是卷级系统协调，不能与轻量单文件版本追加直接等同。作为唯一跨平台后端的适用性较弱，作为条件优化仍需本机验证。

## 6. SQLite BLOB 与独立文件：另一个独立选项

- **公开测量**：2011 年 SQLite 测试在 Ubuntu/Ext4/SATA、SQLite 3.7.8 上比较不同页大小和 10 KB—1 MB BLOB，测试随机读取；约 100 KB 的分界只是该实验的经验，作者要求在目标硬件复测。[原始测试与条件](https://www.sqlite.org/intern-v-extern-blob.html)
- **公开测量**：2017 年官方小 BLOB 测试报告数据库相对单独文件的优势，并解释 open/close 及文件块填充可能带来的影响；页面明确指出硬件、系统、实验细节改变结果。这不是 Windows 编辑器版本追加、校验、压缩和持久化提交的端到端测试。[官方实验与 caveats](https://www.sqlite.org/fasterthanfs.html)
- **推导与限制**：应独立测试“小对象进 BLOB”“对象文件＋SQLite 元数据”和必要的混合方案。统一同步等级、事务批量、索引、缓存和清理规则；SQLite WAL 会增加 checkpoint 及附属文件，不能把 WAL 本身当成长期文件历史。[WAL 生命周期](https://sqlite.org/wal.html)

## 7. 本机比较必须报告的边界

以下为根据上述机制制定的测量要求，不是任何参考系统的已测结果：

1. **同一语义**：同一版本序列、同样捕获/合并规则、同样保留期、同样损坏检测和落盘同步；不能把频繁记录方案与合并后少量记录直接比较。
2. **同一实现层**：分开报告内存编码、带索引的落盘提交和 UI 历史浏览。哈希、压缩、delta、文件创建、SQLite 提交、同步、清理和 diff 计算分别计时，并记录组合后的端到端延迟。
3. **多种负载**：真实 Rust/JSON/文本、不同体积、短编辑/分散编辑/整篇格式化/前插/完全改写、重复版本/回退重现/跨文件重复。保留原始字节并对每个重建版本逐字节验真。
4. **历史浏览**：区分只读元数据列表、随机读最旧/最新版本、连续滑动版本和两版本 diff；解码链预热缓存的成绩要与首次读取分开。
5. **空间和资源**：同时记录逻辑内容字节、对象/数据库/WAL/索引/清单体积、文件数、峰值内存和清理后的实际占用；标明是否测了文件系统 allocated size。
6. **统计与可复现性**：记录机器、文件系统、工具/库版本和参数、数据集来源/种子、重复次数、中位数及尾部延迟；标清热缓存、冷缓存、是否同步和是否包含启动成本。未控制 OS 缓存的测试不得标成冷读。

本轮本机微基准比较六组：raw 全量、raw 全量 CAS、Zstd level 1 全量 CAS、Zstd level 3 全量 CAS、Zstd level 1 前一版本字典链（每 16 个版本一个完整检查点，最多 15 步）和 Zstd level 1 直接引用完整检查点（最多一层参考差分）。最后一组对新内容优先引用本组检查点；重复内容可复用旧对象及其原检查点，仍保持最多一层依赖。名称描述实际编码；结果见[本机微基准](storage-benchmark-2026-10-22.md)。CDC、操作日志/COW、ReFS 和 VSS 在本记录中仅进行机制比较，未完成本机实现和测量。

只有在同一 Windows 机器、代表性编辑器负载与一致持久化保证下的结果，才能支持 Nanobug 的条件选择。跨硬件公开测试、压缩器吞吐、Borg/restic 的备份规模及参考实现复杂度，均不能单独证明某一完整历史系统“最优”。当前证据支持把压缩全量＋去重、受限参考差分、适配小文件的 CDC 作为不同候选做对照；操作日志/COW 和文件系统能力须额外满足集成与平台条件。
