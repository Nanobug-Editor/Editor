# Plugin SDK

公开契约的用途、权限、错误、限额、兼容与释放义务维护在 [Rust 源码注释](src/lib.rs)，供宿主和独立访客使用。多来源诊断见 [diagnostics](src/diagnostics.rs)，实验直接语言结果见 [direct](src/direct.rs)，标准 LSP 的实际能力边界见 [language](src/language.rs)，预览修复与只读源前提见 [workspace](src/workspace.rs)。

SDK 只导出源码、Cargo、WIT 与 JSON 构建输入，不包含此 Markdown 或其他仓库读者资料。

`workspace.files 1.4` 的 `Plan.conditions` 为预览和提交附加只读文档版本条件；它不授予读取权限，也不将辅助输入加入分组撤销。跨文件重命名与选中时解析的补全使用既有原生文档会话；主补全和附加导入共同校验、一次应用。契约和限额见 [workspace](src/workspace.rs) 与 [language](src/language.rs)。

HTTP、密钥、私有状态、计时器、通用树与编辑器装饰分别见 [http](src/http.rs)、[secrets](src/secrets.rs)、[storage](src/storage.rs)、[timers](src/timers.rs)、[tree](src/tree.rs) 与 [decorations](src/decorations.rs)。

通用树的 `ui.tree 1.2` 提供 `tree::update`，原子更新缓存节点的显示和禁用状态，保留已加载页、展开、epoch 及未完成请求；结构变化仍用 `refresh`。清单须明确声明 1.2 最低版本，完整预算与回调约束见 [树契约](src/tree.rs)。

原生树未指定 `Node.icon` 时，目录与文件显示名称使用资源管理器的图标及深浅主题优先级；显示名称不授予文件访问权。显式包内 artwork 继续遵循 `ui.icons`／`assets.read` 校验。未声明 `Command.menu` 的插件不显示空标题栏菜单按钮。

主编辑区的持续比较标签见 [文档与比较契约](src/api/documents.rs)：`editor.diff 1.1` 提供双源标签，`1.2` 允许 `OpenDocumentComparison.third` 加入第三个只读来源；第三源采用相同的归属、精确版本及撤销边界。

默认按命令、语言、视图或服务需求启动，后台用途须明确声明，见 [activation](src/activation.rs)。结构化配置、变化通知、适用的热应用与密钥引用见 [settings](src/settings.rs)；宿主版本下限、稳定同 major 演进、实验能力及迁移义务见 [lib](src/lib.rs)。包版本、传输协议、能力版本和宿主版本分别处理。

独立 SDK 只导出本 crate 的源码、Cargo 清单、WIT 与 JSON 协议资源，不内嵌或导出 Markdown。

按文档发现和共享项目上下文使用 [language.lsp 1.5](src/language.rs)；[language.control 1.0](src/language_control.rs) 在同一受管连接上传输有界控制消息，并以宿主封印的原生文档身份及 revision 关联普通源同步。受管原生适配闭包使用 `language.modules 1.0` 与独立 `native.modules` 授权，模块不加载进宿主，也不具有 WASM 级别的相互隔离。只读项目成员分页形状见 [language_context](src/language_context.rs)；显式多提供者集合及其撤销语义见 [plugin.services 1.2](src/service.rs)。

`language.lsp 1.6` 的[上下文引用](src/language_context/registry.rs)封印 provider 归属、工作区和配置代次。插件登记候选并通过既有有界控制选择实际分析计划；宿主不解释其业务配置。显式交互准备 profile 可调用原生 QuickPick 和本实例私有偏好 CAS，嵌套服务调用不继承私有访问来源。原生导航只借出一次有效来源引用，明确刷新校验原文档 revision，内部 provider 仅从界面候选隐藏；能力、权限与清理要求不变。

关联快照同时要求提供者和原始调用链具有 `workspace.read`，方法的较弱权限声明不能代替该授权。原生控制失败只有在匹配 JSON-RPC 响应携带严格的 `error.data={"version":1,"imports":"released"}` 回执时，才释放本次新增模块使用；已有成功使用及其他进行中的使用仍有效。回执表示没有仍存活的受管 worker 保留本次新增导入，不承诺代码从未执行或外部副作用已经回滚。普通错误、取消和超时无法证明释放，宿主继续保守保留可能已加载代码的授权。

- [SDK 源码与使用注释](src/lib.rs)
- [组件接口](wit/plugin.wit)
