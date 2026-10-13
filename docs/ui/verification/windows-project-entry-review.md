# Windows 外部项目打开双轴审查

日期：2026-10-13。工单 [#105](https://github.com/T-miracle/Nanobug/issues/105)，来源为[总规格](../specs/project-windows.md)和[03 工单](../tickets/project-windows/03-windows-project-entry.md)。源码审查完成，真实安装与系统验收在本记录建立时仍待完成，随后已用同一候选完成；本文件不能代替[行为验收记录](windows-project-entry.md)。

固定点：`c8da09db104f4ed29ad249b2dd7510b19017f879`。原比较命令为 `git diff c8da09d...HEAD`、`git log c8da09d..HEAD --oneline`；实现尚未提交，因此记录空结果后，以 `git diff --cached c8da09d` 审查完整暂存候选。两条轴通过独立只读 sub-agents 并行运行，不编辑源码、不重复运行 Cargo 或操作安装/GUI。

初审树 `17746f6dcf55660290b2f683fa96dc55943eb1aa`；第一次修正树 `ff163262e21b3bde6e4783c717d4debe048a33ed`；最终源码树 `69c242119e9df97d038a11e8d6fe9057768aa169`。交付提交另记录于验收文档，普通提交保留这些审查与修正的证据。

## Standards

依据：根 `AGENTS.md`、UI 主题与本地 Base 控件约定、安装归属规则，以及 `code-review` 的完整 12 项 Fowler 坏味道基线。硬性规则与启发式判断分开。

初审硬性发现依次为：P2 失败报告直接绘制上游 `TitleBar`；P2 启动与传输自有用户提示硬编码英文。Fowler 判断性发现为 0。另报告 P1 资源风险：慢速半帧可反复重置 socket 的单次超时，长期拖住接收线程、最终 `join` 和 profile 租约释放。

复审确认：失败报告使用已有本地 shell 窗控；自有错误全部接入 `projects.yml` 中英文资源；帧读写采用绝对期限、100ms 取消检查，停止时不再等待错误回复。回执缓存保留 20 秒，限额时拒绝新请求而不遗忘有效回执。半帧回归先红于退出等待 3.70 秒，修正后通过。

最后的安装器增量另做只读复核：枚举失败按错误处理，未知值/子键保留，空键删除结果检查，根键删除失败恢复归属并报告错误，真正空键可重新注册。符合现有归属与注释要求。

最终结果：硬性规范违规 **0**；Fowler 判断性发现 **0**；新增其他确定风险 **0**。审查不把尚未执行的 Release/系统验证认定通过。

## Spec

初审按顺序报告：P1 `checkedonce` 导致升级取消上次已选菜单；P2 先删 command 后父键清理失败，会因归属要求 command 存在而无法重试。确定源码要求缺失与范围蔓延均为 0；真实 CLI、退出交接、安装与 Explorer 联合流程仍是工单验收门槛。

第一次复审确认上述两项修正，同时发现 P2 尾部空键清理缺口：忽略 `RegDeleteKeyIfEmpty` 的失败，可能在删除最后归属标记后留下无法重新注册的空键。相关官方契约：[Inno 空键删除](https://jrsoftware.org/ishelp/topic_isxfunc_regdeletekeyifempty.htm)、[枚举值](https://jrsoftware.org/ishelp/topic_isxfunc_reggetvaluenames.htm)、[枚举子键](https://jrsoftware.org/ishelp/topic_isxfunc_reggetsubkeynames.htm)、[Windows 注册表访问权限](https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-key-security-and-access-rights)。

最终复审确认：移除 `checkedonce`、保留 `UsePreviousTasks=yes`；单一 `NanobugOwner` 标记同时保存产品和安装目录，最后删除；缺少 command 的自有项仍可重试；command 空键删除失败时标记尚在，根空键删除失败恢复标记并报错；未知数据保留，空键重建没有扩大为覆盖非空外来注册项。

最终结果：(a) 确定源码要求缺失 **0**；(b) 范围蔓延 **0**；(c) 新增确定实现错误 **0**。实际安装生命周期、权限失败重试、真实 CLI/退出交接和 Explorer 联合流程仍须由主代理验收。

Standards：最终发现 0，无未解决严重项；Spec：最终发现 0，无未解决严重项。

后续系统验收未修改受审源码：已识别工具注册表视图隔离，从实际 Explorer 环境安装后两个菜单可见；中英文、实际 A/B 窗口联合退出/恢复、首次取消与升级保留、未知注册保留、缺少 command 重试、外来 command 拒绝、两次真实卸载及 23 个数据文件摘要保持均有行为记录。没有通过修改 ACL 或系统安全设置模拟注册表权限失败，也不将未执行的任意故障组合记为已验证。

最终文档增量树 `145bb9d86e5f21e19b83e31e7e3d374f85b3fdd7` 再次由同一双轴并行只读复核。Standards 核对七份文档、49 处本地链接、测试数字与摘要，硬性/Fowler/其它确定风险均为 0。Spec 核对实际桌面与窗口记录，缺失要求/范围蔓延/确定错误均为 0；上轮菜单缺项已由真实视图对照解释并实际复验通过。两轴确认可以进入普通提交、推送和关闭读回，未提前认定 GitHub 已关闭。
