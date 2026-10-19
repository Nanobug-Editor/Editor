# 插件工具与共享偏好

`ui.tools ^1` 为 `Document.tools` 提供至多 32 个原生底栏按钮。
每项包含稳定 ID、`LocalizedText` 双语名称与提示、`ToolIcon` 深浅主题包内 SVG、
`visible/selected/disabled/order` 及明确的 `ToolTarget`。
File 目标必须等于当前 `Document.file`；Window 目标只能是本贡献所属独立窗口。
窗口显隐入口来自清单中的独立面板，中心布局和 auxiliary 贡献不自动产生窗口入口。

`ui.panels >=1.1, <2` 允许面板声明 `show_in_window_controls:false`，从窗口组及其
溢出菜单省略该面板。省略字段默认为 true，既有插件入口不变；本字段不改变
`default_visible`、用户保存的显隐意图或 `SetPanelVisibility`，适合由业务操作打开的详情窗口。
面板 `icon_light/icon_dark` 仍引用插件包内 SVG，`status_order` 决定窗口组内顺序。

`ui.panels >=1.2, <2` 允许侧边和底部面板声明 `exclusive:true`（单独占据）。
省略时为 false，普通面板继续组合显示。显式打开独占面板时，它临时替换当前区域的全部面板；
关闭后恢复原组合、尺寸和显隐意图。打开另一个独占面板切换至新面板；点击普通窗口入口恢复组合。
行为依据实际停靠位置，支持左侧、右侧和底部；`editor`、`window` 声明不得使用该属性。
各区域独立，编辑区不受影响；窗口选择随会话保存。宿主不按插件 ID 判断布局。
最低版本必须排除 1.0/1.1，例如 `">=1.2,<2"` 或 `"^1.2"`，普通 `"^1"` 不足以声明本属性。

宿主在短竖线左右分别绘制窗口组和工具组，多个适用文件贡献可以同时提供工具。
插件接收 `Notification::Tool(ToolEvent)` 并实现功能；底栏与各自溢出菜单使用同一状态和事件。
点击底栏保留此前文件或活动窗口目标，文件切换、重开、实例替换、模态、隐藏、禁用、
旧 UI revision 和旧权限均不能通过校验。实例退出撤销全部贡献。

工具图标只能引用当前包内相对路径，运行时校验规范化边界及至多 64 KiB 的安全几何 SVG。
缺失或非法资源拒绝整个发布，不使用宿主专属图标兜底；`currentColor` 接收通用原生主题颜色。
本能力不授予文件读取或进程权限。File 工具需工作区实例和 `editor.read`。

`ui.tree >=1.1, <2` 为 `tree::Command` 增加 `disabled` 和 `separator_before`（默认 false），
并将每节点原生右键菜单上限从 8 提高到 32。清单必须 required 声明排除 1.0 的最低版本；
仅 `^1` 不足以发布新元数据或超过 8 项。原生菜单显示禁用状态，运行时在解析精确菜单槽时
再次拒绝禁用项；disabled 不只是外观。完整参数仍绑定当前树的 owner、epoch、node 与槽位，
使用已声明的 `plugin.commands` 类型化命令。旧 1.0 范围仍只允许 8 项及原有字段语义。
长菜单使用现有原生滚动、键盘导航与深浅主题样式，不需要新的执行权限。
`ui.tree >=1.2, <2` 增加 `tree::update(tree, nodes)`，清单 required 范围必须排除 1.0 和
1.1；仅 `^1` 不足以使用。每批最多 128 个完整节点和 512 KiB，沿用页的元数据、命令类型、
图标归属和权限校验，并在整批通过后原子更新。仅替换已缓存同 ID 节点的显示字段及命令禁用
状态；未缓存 ID 校验后忽略。分支类型、命令名、参数、菜单顺序及分隔位置不得改变，否则
返回 `Conflict`。epoch、展开状态、已加载子节点、游标及未完成页请求保持不变；内容相同不
推进 revision。结构变化仍显式 `refresh`，不能用元数据更新绕过旧回调与权限检查。
类型化命令的 `signature.permissions` 必须声明实际使用的既有权限，宿主继续取交集。
宿主直接调用本实例命令时，`ui.panels` 允许更新本实例拥有的树，不要求无关的
`files.select`。跨插件调用仍不能借用提供者已有的私有树；文件选择仍单独检查 picker 权限。
同样，直接原生命令可在方法声明 `storage` 后读写本实例的有界私有偏好（读取不创建 watch）；
命名空间、CAS 与配额校验不变，跨插件调用不能借用这些偏好，未放开私有文件或秘密访问。

真实宿主类型化命令也可关闭／取消本实例先前创建的资源，包括定时器回调创建的后台进程。
此例外仅适用于资源清理，不授予读取或改写这些资源的权限；跨实例、失效句柄、嵌套委托及
已撤销的宿主命令不能使用此例外。各调用清理其自己分配资源的既有撤销后收尾规则不变。

`storage.private >=1.1, <2` 增加 `ReadPreference/WritePreference`，需要 `storage` 权限。
SDK 提供 `read_preference(key, watch)` 与 `write_preference(key, expected_revision, data)`。
访客 `PreferenceBinding` 负责绑定／撤销 watch、校验新通知和 CAS 冲突后重读；
插件自行解码 schema。切换文件类型或退出内容时调用 `bind(None)`／`close()`，不存储文本和原生会话状态。
宿主给定插件与工作区命名空间，`PreferenceKey` 只包含规范化文件类型和插件本地名称。
名称不能指定路径或其他所有者；空文件类型可以保存工作区窗口意图。

缺失值为 revision 0/data None，写入是 compare-and-set，过期版本返回 `Conflict` 并保留新值。
相同值不推进 revision；JSON 上限 64 KiB，仍计入插件私有目录整体配额及迁移事务。
损坏记录明确报错并保留，不能静默重置。实例最多 32 个 watch，返回普通可撤销资源句柄；
关闭资源、撤销实例均清理订阅，后续变更通过 `PreferenceChanged` 发送。
损坏或不可读记录终止对应 watch 并发送 `SubscriptionFailed`。

数据是不透明显示意图，不授予实际窗口、布局或文本权力。各实例须校验订阅与 revision，
冲突后重新读取并应用当前意图，不盲目重试覆盖；原生编辑会话、选择和 Undo 保持各文档独立。
独立示例：`layout-example` 使用工具及按类型偏好；`tools-example` 同时贡献辅助工具和独立窗口。

旧宿主 session 的模式、同步及工具栏记录只作为有限导入源。后台 `Manager::import_preference`
将不透明 bundle 放入当前插件／工作区／类型的 `imported-presentation` 键；已有记录优先，
损坏、权限、配额和实例变化明确失败。仅成功收据可以删除对应且内容未变化的旧 bundle，
其他设置继续保留。插件随后转成自己的 `display` schema，已有新版意图不被导入覆盖。
