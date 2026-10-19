# Git 分支树右键操作

开始：2026-10-18；最终验收：2026-10-19。状态：本地候选交付完成，未安装、未提交或发布。

## 行为与边界

用户要求移除 Git 顶部工具栏，将操作放到左侧分支树的鼠标右键菜单，未实现的功能先禁用。

- 独立 Git 插件升级为 0.1.11；不内置 Git 业务，不增加权限。
- 顶部按钮组及全局目标输入行移除。左键筛选提交；右键捕获该节点的完整本地／远程引用，目录没有隐含分支。
- 刷新、全部分支、获取、拉取、推送、切换、新建分支、高级操作和停止操作均迁入原生右键菜单。菜单沿用滚动、键盘导航及深浅主题。
- 新建分支保留右击来源并单独输入名称。高级操作弹窗预填右击引用，仍可明确输入 revision 和多个有序的 cherry-pick 提交；影响预览、原生缓冲检查及确认继续生效。
- 拉取／推送只在当前检出分支启用；远程引用直接切换、当前本地分支重复切换，以及目录中的目标操作均置灰。仅实现的操作进入清单，未知动作不会落入默认可执行分支。
- 业务执行时保留分支菜单的停止入口，其他动作按状态置灰。菜单可用性在业务／弹窗转换时刷新，30 秒静默观察继续保留原有节点和树页。

宿主只扩展通用 `ui.tree 1.1`：菜单槽增加默认 false、序列化时省略 false 的 `disabled`／`separator_before`，每节点上限 32 项。使用新元数据或超过 8 项必须 required 声明排除 1.0 的最低范围；旧包仍受原 8 项规则约束。原生显示与运行时槽解析都拒绝禁用项，仍检查 owner、epoch、node、槽位和类型化参数。SDK 源码注释及 TOOLS.md 同步，无上游复制或 fork。

## 验证

- 先以 0.1.10 实包运行新增公开管理器回归，确实因旧工具栏仍存在而失败：`target/git-branch-menu-red.log`。
- 独立访客测试：100 passed、0 failed；已撤下停止按钮的旧断言移至真实包／原生树菜单验证。最终日志 `Git-for-Nanobug/target/branch-menu-unit-final.log`。
- 最终 `cargo fmt --check`、`cargo check --workspace --jobs 1` 通过；`cargo test --workspace --exclude editor-app --jobs 2`：78 组、291 passed、307 ignored、0 failed。ignored 未被计作通过。日志分别为 `target/git-branch-menu-format.log`、`target/git-branch-menu-workspace-check-final.log`、`target/git-branch-menu-nonui-verified.log`。
- Release 宿主及独立 ZIP 通过公开 `--plugin-package` 构建；既有编译／链接警告保留，未扩大范围清理。
- 最终真实包／公开管理器两项回归通过（100.08 秒，`target/git-branch-menu-manager-final.log`）：禁用的非当前分支 Pull 被拒绝；Switch 审查确认后实际切换；从右击来源创建分支；Merge 保留正确引用的影响预览；原生缓冲枚举期间可由右键菜单停止，迟到回执不会执行旧 Switch；直接原生命令不需 files.select 即可刷新自己的树；退出资源归零。把同一包的 ui.tree 最低范围改为 `^1` 后，扩展菜单整页拒绝并提示 `ui.tree >=1.1`，无部分元数据泄露。
- 最终原生 GPUI 菜单两种主题通过（53.10 秒，`target/git-branch-menu-native-final.log`）：真实包节点／菜单槽投影至生产 PluginView，右键打开；非当前分支 Pull 点击无事件；Switch 由真实菜单打开审查，再原生点击确认后实际切换。只挂载真实分支子树及真实对话框，不宣称覆盖整工作台的 canvas 布局或长时前台焦点。
- 最终有序多个 cherry-pick 提交的实际 Git 回归通过（52.07 秒，`target/git-branch-menu-batch-final.log`），使用右键参数弹窗进入原确认。
- 最终远程成功流程通过（65.80 秒，`target/git-branch-menu-network-final.log`）：非 origin 远程的 Fetch、Pull、Push、已发布提交改写、两轮精确 lease 及独立确认均在隔离的本地 bare 远程完成。期间发现类型化方法缺少执行权限、私有偏好误用跨插件限制，已补齐原有 process/editor/workspace/panel/storage 方法权限，并修正通用原生命令自身树及有界偏好的检查；未新增安装权限，跨插件借用及 picker 限制保留。

全量默认并发的一次尝试因 Windows 页面文件不足（1455，mmap rlib 失败）中断，随后单／双作业重跑完成；没有清理缓存或修改系统设置。初期原生测试挂载整个 history 场景时，异步 canvas 尺寸回执抢先替换了测试场景，已将本次原生验收限定为实际分支子树与实际对话框，未放宽 revision 校验或重放旧操作。

## 交付身份

- `C:/Projects/RustProjects/Editor/target/git-branch-menu-2026-10-18/git-0.1.11.zip`：809517 bytes，SHA-256 `39e4916e1e6d7e9c3a7149add54cb526a1cf204e5085f946a3f3682a864742cd`。
- 同目录 `Nanobug.exe`：118527488 bytes，SHA-256 `f1527dbdda3ef01d5f3d506bb23e320b54a6e9a1899b4ca79ea3e565ec6bafb3`；需配套此宿主的 ui.tree 1.1 及直接原生命令资源检查。
- 公开 SDK 0.2.0，缓存摘要 `fb54e0069ef7502dee92c95578e0e698cca6ca1ce6631f98141721971352b5df`，协议 7。ZIP 仍仅 WASM、清单、两份用户 SVG 和原生 helper，SVG/helper 摘要与 0.1.10 相同。逐项摘要见同目录 `identity.json`。
- 独立源码及 ZIP 副本：`C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.11`。56 份当前源码／图标与源码 ZIP 逐项一致，源码归档 166613 bytes，SHA-256 `12de03f1e84c53897ae36c847055f73bab95257a519ee860c174150a921a692b`。没有 SDK 或上游项目副本；既有 0.1.10 交付保留。
- 安装步骤见宿主交付目录的 `使用说明.txt`：使用配套宿主，再由插件管理器显式更新 ZIP。未静默替换用户安装。

测试只在临时仓库及隔离 Git 配置中操作。用户运行中的应用、安装记录、私有数据、Git 配置和市场均未修改；没有提交或推送。
