# Nanobug 文件历史

状态：基础功能已实现，Windows 定向验收和性能实测已执行；workspace 测试与编译检查通过，全量格式检查仍有其他任务的差异。日期：2026-10-22。

文件历史采用独立的 editor-history crate，负责平台无关的历史存储和管理。内容使用 Zstd 独立全量压缩与内容去重，版本直接引用内容对象，同时保留文件结构变化元数据。

| 材料 | 用途与状态 |
| --- | --- |
| [基础方案](specs/local-history.md) | 已确认的记录、恢复、身份、迁移、范围与清理行为，以及模块边界和实施顺序 |
| [验收范围](verification/acceptance-plan.md) | H01–H38 功能、故障、原生交互和性能通过条件 |
| [实施记录](verification/implementation-2026-10-22.md) | 实际结果、逐项证据、双轴审查及剩余格式门禁；不把 ignored 计为通过 |
| [产品性能实测](verification/performance-2026-10-22.md) | 实际 Rust 存储、原生保存/预览/分页、空间与峰值；记录大文件界面延迟限制 |
| [存储方法调研](research/storage-methods-sources-2026-10-22.md) | 一手机制依据与比较边界 |
| [编码微基准](research/storage-benchmark-2026-10-22.md) | 全量、去重和参考版本编码的原型实验 |
| [完整快照与旧内容日志比较](research/history-models-benchmark-2026-10-22.md) | 统一后端的结构操作、压缩、去重和历史定位实验，附原始数据与程序 |

新方案替代历史开发计划中本地历史的“压缩增量、检查点”路线。原始实验保留测量条件，不能当作 Nanobug 产品功能或真实 IDEA、VS Code 的性能验收。

实际执行记录归入 verification/，后续工单归入 tickets/。当前未创建工单、提交或发布；全量格式的后续复查要求见实施记录。
