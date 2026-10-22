# Nanobug 文档目录

本目录按主题管理项目文档。方案、工单和验收记录相互链接；历史材料保留原始结论，不作为当前实施状态。

| 分类 | 入口 | 用途与状态 |
| --- | --- | --- |
| 项目基线 | [需求整理](project/需求整理.md)、[开发计划](project/开发计划.md)、[全分支整合记录](project/branch-integration-2026-10-12.md) | 已确认的历史产品基线；具体实施进度以对应主题的验收记录为准 |
| 产品品牌 | [Nanobug 名称、图标与兼容标识](project/branding.md) | 当前品牌入口与保留旧数据、开发和仓库标识的依据 |
| 开发构建 | [构建缓存隔离与恢复](project/verification/build-cache-2026-10-18.md)、[Release 启动复查](project/verification/build-cache-release-2026-10-18.md) | 源码接口存在却报缺失的实际复现、按 profile 清理缓存及工作区隔离约定 |
| 主分支整合 | [2026-10-18 整合记录](project/verification/main-integration-2026-10-18.md) | 当前工作区、Git 工作台分支和远端主分支的保留、冲突处理与验证 |
| 安装与分发 | [安装包目录](distribution/README.md) | 第一期 Windows 安装器与验收；macOS/Linux 兼容打包入口不验收 |
| 插件系统 | [插件文档目录](plugins/README.md) | 方案、工单与验收；原平台 20 张工单已完成，社区基础能力七单的状态见对应验收入口 |
| 原生 UI | [UI 文档目录](ui/README.md) | 原生 UI 方案与 GPUI 交互调研；快捷键面板已交付，资源管理器文件转移 Windows 原生及联合验收已完成，平台限制见验收记录 |
| 文件历史 | [文件历史目录](history/README.md)、[基础方案](history/specs/local-history.md)、[验收记录](history/verification/implementation-2026-10-22.md) | 独立 editor-history 与 Zstd 全量去重已落地；Windows 定向验收、workspace 测试与编译检查通过，全量格式仍有其他任务差异；产品实测见主题记录 |
| 运行与公开 SDK | [英文 README](../README.md)、[中文 README](../README.zh-CN.md)、[协议源码](../crates/plugin-protocol/src/lib.rs) | 双语运行、构建与打包入口；接口使用、权限和释放义务维护在源码注释 |
| 历史文档站点 | [静态文档站点方案](website/specs/static-docs-site.md)、[规格](website/specs/docs-site.md)、[实施工单](website/tickets/README.md)、[交接说明](website/handoff/static-docs-site-handoff.md)、[移除验收](website/verification/removal-2026-10-09.md) | 2026-10-09 按用户要求移除站点、发布工作流、文档内嵌与独立读者文档目录；旧方案、工单及验收仅供追溯 |
| 协作约定 | [领域术语](agents/domain.md)、[议题工作流](agents/issue-tracker.md)、[分诊标签](agents/triage-labels.md) | AI 与维护者的协作依据；仓库入口为 [AGENTS.md](../AGENTS.md) |

## 后续存放约定

- 插件系统文档统一放在 `plugins/`：方案进 `specs/`，工单进 `tickets/`，执行证据进 `verification/`，使用及格式说明放在主题目录根部。更新[插件索引](plugins/README.md)。
- 其他主题沿用同样的“主题/specs、tickets、verification”结构，按需创建目录，并在本总目录登记；UI 调研放在 `ui/research/`。
- 会话交接按“主题/handoff/”放置，只记录状态、边界与下一步，用路径引用方案正文而不复制其内容；方案或验收更新后同步核对交接是否需要作废。
- 项目级需求和计划放在 `project/`，协作规则放在 `agents/`。失效且没有追溯价值的副本、临时会话资料和源码快照直接移除，历史可从 Git 查询；需求基线和真实验收证据保留并标明适用范围。
- 基本运行、构建和打包入口维护在根双语 README；公开 SDK 的接口说明与类型共置于 `crates/plugin-protocol/` 源码注释。宿主只内嵌和导出其源码、Cargo 清单、WIT 与 JSON 资源，不内嵌或导出 Markdown。
- 本目录服务于 AI 与维护者，不发布站点；官方插件包说明保留在各自`Nanobug-Editor` 组织的独立插件仓库的 README，避免复制公开契约正文。
- 新方案与工单按实际阶段标注“草案、已确认、待实施、进行中、已完成”；完成时补齐验收证据、实际交付状态和平台限制。历史基线与未验收内容不统一标成已完成。
- 2026-10-08 辅助脚本移出仓库后，历史验收中的 `scripts/` 命令仅保留当时证据，不是当前执行入口；打包步骤以[原生安装器说明](../installer/README.md)为准。
