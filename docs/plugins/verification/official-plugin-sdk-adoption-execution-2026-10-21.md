# 官方插件 SDK 改造：三会话实施调度

日期：2026-10-21。状态：用户已明确授权实施，三个独立本地会话均已创建并开始工作；尚未汇总为验收完成。

依据：[改造方案](../specs/official-plugin-sdk-adoption-2026-10-21.md)、[源码调研](../specs/official-plugin-sdk-adoption-sources-2026-10-21.md)、[原始静态核验](official-plugin-compatibility-2026-10-21.md)。A0/A1/A2 是方案切片，不是 GitHub issue 编号。本次未创建或关闭 GitHub issue，未授权提交、推送、发布及替换用户日常安装。

| 独立会话 | Thread ID | 实施范围 | 专属验收记录 |
| --- | --- | --- | --- |
| 修复 TypeScript 插件兼容性 | `01a15eea-74e5-72c1-99e5-4648335e16a2` | A0：固定并修复正式宿主/SDK/候选配套，验证受影响 TS/Vue 组合；必要的通用宿主兼容改动由此会话负责 | `official-sdk-typescript-2026-10-21.md` |
| Git 插件接入 ResourceGroup | `01a15eea-ed59-7031-bd02-b265fb43e005` | A1：定时器、文档订阅的拥有、精确事件匹配、部分失败和重置清理 | `official-sdk-git-2026-10-21.md` |
| Markdown 插件接入 EditorIntent | `01a15eeb-658e-73b1-8485-4cd03873a47b` | A2：格式编辑与显式导航；完成后对 A3 滚动作独立接入或保留结论 | `official-sdk-markdown-2026-10-21.md` |

三个会话位于已登记的 Nanobug Editor 项目；Git、Markdown、TypeScript 业务修改分别在其真实独立插件目录执行。不是三个子代理，也不把已有其他任务改动归入本次实现。

## 已传递的范围与隔离要求

- 每个会话立即沿既有方案实施，不等待重复确认；执行适用行为测试、构建、审查、实际包及原生验收。状态、测试结果和未验证部分由对应会话如实记录。
- Git 与 Markdown 固定已包含助手的宿主 `target/plugin-developer-tooling/debug/editor-app.exe`：EXE SHA-256 `c82565746b49011761884604da17e1bcbd30a45cb972b42a2a6ba20fda940856`，SDK `8e2f359fedf819d8e8b6b3da1012722629e12d9731c02c38e5b01ccab8bd02e4`；不覆盖该产物，不引用 TypeScript 会话正在变化的源码来冒充同一 SDK。
- 新宿主构建分别使用 `target/sdk-adoption-typescript-2026-10-21/`、`target/sdk-adoption-git-2026-10-21/`、`target/sdk-adoption-markdown-2026-10-21/`；不同源码快照继续分目录并核对 Cargo metadata。插件构建也使用独立输出，不覆盖正在运行的测试 EXE。
- 各自保留用户/其他任务的源码改动，先固定实际源码、EXE、SDK、ZIP 身份；不复制 SDK 或建立到宿主 crates 的依赖。新交付包提升版本，不覆盖原同名归档。
- 三会话只写自己的实现及专属验证记录，不修改共享总方案/索引；父会话维护本记录和总入口。TypeScript 不替 Git/Markdown 做助手改造；后两者不修改公共 SDK。
- 原生运行使用独立 profile/workspace，不操作用户已有窗口或重启日常宿主；交互测试避免抢占其它会话焦点。兼容热安装必须保持 PID、未保存内容与 Undo/Redo，失败候选保留旧实例，成功后旧 owner 退役。
- TypeScript 不删除 interactive、放宽未知字段或硬编码插件 ID；Git 不用 ResourceGroup 接管进程/树/比较源；Markdown 保留图片导入副作用回执，A3 不破坏 pending+latest 合并。不新增 HttpTask 无用途迁移或网络权限。

## 启动确认

通过一次 `wait_threads` 三目标状态读取及列表核对，三个会话均为 active：

- TypeScript 已复现固定 EXE 对旧候选的拒绝，开始追溯配套实现、公开契约和委派路径。
- Git 已读取 A1 并进入实施/行为回归准备，确认保留工作区既有改动和独立输出。
- Markdown 已开始核对 A2/A3、两个仓库状态及现有 0.18.2 工作。

这里只确认已创建并启动，不将开始工作或历史静态通过当作代码交付通过。后续每个会话的实际验收完成后再更新总状态。
