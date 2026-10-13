# Windows 外部项目打开实施记录

日期：2026-10-13。状态：实现、自动验证、真实 Windows 行为验收及双轴源码审查完成，待普通提交、推送与 GitHub 关闭读回。

依据：[工单 #105](../tickets/project-windows/03-windows-project-entry.md)与[总规格 #102](../specs/project-windows.md)。#103、#104 已交付；本单从 `c8da09db104f4ed29ad249b2dd7510b19017f879` 开始，分支 `codex/windows-project-entry`，复用隔离工作区。原 main 不作为测试安装目录。

沿用已确认的两个接缝：真实 GPUI 的统一项目生命周期，以及真实 Windows 安装器/资源管理器。启动与传输是生产打开请求的依赖适配器，测试使用同一路由观察窗口、文档与交付回执；不新增测试专用路由，不复制前两单的完整细分矩阵。

先验证外部请求的目录去重与文档保留，再接入按 profile 独占的进程协调。工具命令和显式开发实例先分流；次进程仅在确认现有所有者接受请求后报告递交成功，所有者忙碌、失联或正在退出时保留失败和重试边界。

安装采用当前用户静态目录 verb，文件夹与空白处分别传递明确目录。任务选择沿用 Inno Setup 的 `UsePreviousTasks`；取消选项与卸载只清理本产品拥有的键。官方契约依据：[Windows 静态菜单与按用户注册](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers)、[前台激活授权](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-allowsetforegroundwindow)、[Inno 任务选择](https://jrsoftware.org/ishelp/topic_taskssection.htm)、[升级保留选择](https://jrsoftware.org/ishelp/topic_setup_useprevioustasks.htm)、[注册表归属和卸载标志](https://jrsoftware.org/ishelp/topic_registrysection.htm)。这些依据不能代替真实安装与右键验收。

## 已完成的针对性验证

- 外部请求复用实际目录的 GPUI 回归先失败于多出一个首页，修正后通过；日志 `external-project-route-red.log`、`external-project-route-green.log`。
- 实际进程协调依赖适配器的首条 GPUI 回归先确认第二次启动仍会成为新所有者，接入 profile OS 文件租约、私有随机令牌与回执后通过；日志 `external-startup-red.log`、`external-startup-green.log`。真实 socket 线程只发布到同步邮箱，GPUI 自己的 executor 消费请求，不从外部线程唤醒测试调度器。
- 扩展场景覆盖 8 个并发 B 目录别名、中文/空格/特殊字符路径及文件定位、无参数首页复用、无效路径、局部关闭待决、全局退出与隔离 profile。组合运行捕获接收 socket 继承非阻塞模式的实际竞态；改成带超时的阻塞读取后，阶段性日志为 `external-startup-targeted-green.log`。最终增加慢速半帧退出回归后，`cargo test -p editor-app -j 1 external_ -- --test-threads=1` 为 **15 passed、0 failed、3 ignored**；见 `external-startup-final.log`。四个新增高层场景通过，三个原有 ignored 场景未在这个过滤命令中执行，不算通过。
- `cargo check -p editor-app -j 1` 通过，记录在 `external-startup-main-check.log`。当前 GUI 参数支持明确的 profile 和 `--` 路径边界，工具命令在此前分流；错误使用原生 GPUI 与本地 Base 按钮报告，不能将请求发送等同于已打开项目。
- 直接 ISCC 构建并实际安装旧模板的独立 AppId 后，目录对象和背景菜单均不存在，`registry-red.json` 保留该失败。新增任务与受控注册后生产模板编译通过；最终程序的实际桌面安装和右键联合验收见后文。

日志与安装资料保留于 `C:/Users/Tmiracle/.codex/visualizations/2026/10/12/01a12e28-69b5-7ea2-b14e-f155ef6bf763`（下文简称验收目录）。代码、打包和原生验证复用同一个最终候选；系统交付与安装生命周期验收完成后才能关闭工单。

## 最终代码检查与审查

源码相对 `c8da09d`，最后审查的暂存树为 `69c242119e9df97d038a11e8d6fe9057768aa169`；后续仅补验收文档，未改变 Rust/安装器输入。详见[双轴审查](windows-project-entry-review.md)。初审发现的本地 UI、双语、升级选择、清理重试及慢速传输退出问题均已修正，最终两轴确定发现为 0。

| 命令 | 实际结果 | 日志 |
| --- | --- | --- |
| `cargo fmt --check` | 通过 | `entry-fmt-final.log` |
| `cargo test --workspace --exclude editor-app -j 1` | 267 passed、0 failed、255 ignored | `entry-workspace-tests-final.log` |
| `cargo check --workspace -j 1` | 通过 | `entry-check-final.log` |
| `cargo test -p editor-app -j 1 external_ -- --test-threads=1` | 15 passed、0 failed、3 ignored | `external-startup-final.log` |
| `cargo test -p editor-app -j 1 -- --test-threads=1` | 533 passed、7 failed、230 ignored | `entry-ui-tests-final.log` |
| `cargo build -p editor-app --release -j 1` | 通过，6 分 18 秒 | `entry-release-build.log` |

全部命令使用已核对的工具链与共享 target，不升级依赖或安装缺失工具。七个 UI 失败与 #103 在干净 main 上建立的基线、#104 的结果完全相同：`status_popover_preserves_language_loading_and_keyboard_dismissal`、`stale_rust_timeout_after_project_reenable`、`status_indicators_open_plugin_lists`、`ctrl_click_definition_on_declaration`、`ctrl_click_definition_releases_source_editor`、`middle_click_definition_stays_on_target`、`operation_errors_retain_plugin_ownership_through_worker_publication`。不将完整 UI 套件报告为全绿，也不把 ignored 计入通过。前两单已显式执行的真实包/原生程序场景继续引用其记录，不机械重跑未受本单改变的完整矩阵。

慢速帧回归真实持有 OS 租约并通过 TCP 发送半帧：旧实现退出等待 3.70 秒，`external-shutdown-red.log` 为 red；绝对读写期限和取消检查修正后通过，下一次进入可以取得所有者。回执在整个客户端重试期限内保留，同一请求不因丢失响应而再次执行文件动作。

## 最终产物

版本为 workspace 的 `0.1.0`，Windows 11 专业版 x64，系统版本 `10.0.26200`。PE machine 为 `0x8664`。直接调用 Cargo、现有 ISCC 与 `Compress-Archive`；不调用已移除脚本或打包代理，不发布 Release。载荷恰为 `Nanobug.exe`、`Nanobug.ico` 和 `licenses/alacritty-terminal-LICENSE-APACHE`，无插件 ZIP、编译器、SDK 或源码副本。

| 产物 | SHA-256 |
| --- | --- |
| 最终 `Nanobug.exe`（115,992,576 字节） | `371bd839e003245abfdb888ab16c027826a189b6f69c77440beb950b6d078b69` |
| 原生产模板安装器（22,545,202 字节） | `6d538a87e0719498cc5b16a6e41912c21839b0c926ff42b6cf7cd4af0435fdce` |
| 隔离验收安装器（22,545,342 字节） | `879db6a582c2782a69d6d8e3f827156663178aba685a25b3c6a4e5c3f63e29df` |
| 便携 ZIP（32,164,946 字节） | `7c9ad4e63d8202789d865802c1e0103ea6b1b2f943a5a4eca04796c1f58c84a2` |

产物与清单在验收目录的 `entry-install/`，`artifact-manifest-final.json` 核对摘要、架构与载荷。生产模板未安装到用户生产位置；验收变体仅更换 AppId、私有 verb/快捷方式标识、显示名、已有中文语言资源路径，并在命令中固定隔离 profile，执行相同注册与清理逻辑。所有真实安装步骤必须显式传入验收目录下的 `/DIR`，不能依赖工具视图中的上次目录。

## 已完成的原生增量

- 最终便携程序以独立 profile 无参数启动，真实首页显示“开始你的工作”，不加载进程目录。运行时执行 `--export-plugin-sdk` 成功且 GUI 仍只有一个首页；关闭首页后进程结束并移除 endpoint。见 `portable-home-exit.json`、`sdk-cli.json`。
- 从明确的中文、空格、`&`、`[]` 文件参数打开 B，活动文件和恢复布局可见。未保存中文文本在一个重复目录请求、八个并发等价目录请求、两个无参数请求后保持，实际只保留 B 和一个首页，十个次进程全部退出 0。见 `native-owner-recovery-and-duplicate.json`、`native-concurrent-requests.json`。
- 已保存 B 的所有者意外退出后，下一进程真实取得租约并打开 B；未保存内容没有用于崩溃恢复测试。全局退出提示中取消，两个窗口与未保存文本保持；保存后退出，磁盘内容与历史快照可见、进程和 endpoint 均结束；重开恢复该文本与活动文件。见 `native-safe-exit.json`。
- 未知 GUI 参数显示原生、本地控件的中文错误，Escape 关闭。显式 `--plugin-dev-instance` 在受限夹具、独立 descriptor/profile 中显示另一个真实编辑器，不加入普通 GUI endpoint；关闭开发实例时普通 GUI 继续存在。见 `native-profile-and-cli-isolation.json`。

## 注册表视图问题与真实桌面复核

最初的工具进程安装、注册表读取和 `Shell.Application.Verbs()` 都能枚举菜单，但资源管理器经典菜单没有它；用户也手动确认缺失。没有因此把枚举成功当作 P20 通过。短系统命令、短 ASCII 目录、Directory/Folder 对照以及 Explorer 重启均不能解释该差异，不为这些未证实的假设修改产品源码。

最后让**资源管理器已有窗口的 `Document.Application.ShellExecute` 直接启动同一安装器 CLI**，其真实父进程是 Explorer；经典菜单随后实际出现。通过独立的 `StdRegProv` 读取交互用户的 `HKEY_USERS/<SID>_Classes`，确认工具写入的 Notepad 对照项在工具视图存在、真实桌面视图不存在（返回 2）；同名菜单的 `NanobugOwner` 安装路径在两个视图也不同。见 `registry-view-comparison.json`、`registry-owner-view-comparison.json`。这是已经观测到的验收环境隔离，不能再以工具内读取代替桌面注册验证；未将其未经证明地归为 MSIX 或修改 Windows 安全策略。

首次外部安装漏带 `/DIR`，落到默认 Programs 目录。已核对该目录创建于本次安装时刻、程序摘要、私有 AppId/归属，生产 AppId 不存在；随后用该实际卸载器清理并核对程序及记录移除。日志 `real-default-cleanup.log`。后续所有实际桌面步骤固定独立 `/DIR`，实际安装位置已由服务侧读回为验收目录的 `app/`。

最初在工具隔离视图完成的菜单勾选/取消、外来注册冲突、未知数据保留与缺少 command 清理记录，仅作为真实安装器逻辑的前置证据，不算最终桌面生命周期通过。桌面复核使用同一安装包和 `StdRegProv`，七个连续阶段均读取实际 CLI 退出码 **0**：保留已选、取消、升级继续未选、重新选择、升级继续已选、英文标题、恢复中文标题。两项注册的存在、标题及独立安装目录归属均符合预期；见 `real-choice-lifecycle.json`。真实首次取消、卸载归属/保留与跨入口联合流程随后按下文完成。

## 实际桌面安装生命周期收尾

所有阶段直接执行同一最终安装器或其原生卸载器，没有新增安装/打包代理。`real-first-selected.log` 记录真实首次默认选中：用户权限 None、非管理员模式、HKCU、固定独立 `/DIR`，两个菜单注册成功。后续步骤如下：

| 阶段 | 实际结果 | 证据（均在 `entry-install/`） |
| --- | --- | --- |
| 英文升级与实际显示 | 原生退出 0，两个实际经典菜单均显示 `Open with Nanobug` | `real-visible-english-final.log`、`real-joint-native-ui.json` |
| 卸载并保留未知注册数据 | 原生日志成功并结束，安装记录、程序和菜单默认值消失；未知值、未知子键和另一软件夹具原样保留 | `real-uninstall-preservation-final.log/json`、`real-uninstall-ownership-final.json` |
| 首次明确取消 | 从无安装记录状态以 `/TASKS=""` 安装，原生退出 0，两项实际注册均不存在 | `real-first-unchecked-final.log/json` |
| 升级保留取消 | 不传任务选项，原生退出 0，仍无两个菜单 | `real-first-choice-and-retry-final.json` |
| 勾选恢复 | `/MERGETASKS=contextmenu`，原生退出 0，两个入口恢复到当前安装目录 | 同上 |
| 中断清理后再取消 | 保留自有标记，移除背景 command 的默认值后取消任务；原生退出 0，两项菜单完整移除；重新勾选又成功 | 同上 |
| 外来命令冲突 | 在独立 QA verb 内保留原标记、换成外来 command；实际安装日志明确拒绝，命令、标记及程序摘要未变 | `real-command-conflict-final.log/json` |
| 最终卸载与清理 | 原生日志 `Uninstallation process succeeded`、`Removed all? Yes`，两个实际 verb、安装记录及独立安装目录均消失；实际对象菜单不再显示 Nanobug | `real-native-final-cleanup.log/json`、`real-desktop-cleanup-keys.json`、`verification-cleanup-final.json`、`real-joint-native-ui.json` |

两次实际卸载前后核对同一组 **23 个持久化文件**的完整路径、字节数与 SHA-256，包括 A/B 会话、最近项目、历史、插件保存夹具、项目文本及另外两个隔离 profile；全部一致。安装器只处理 `{app}` 和自有注册值，没有删除用户数据路径。未知注册夹具验证后仅移除本次准备的精确值/空键；最终还清理了工具私有视图中遗留的 QA 注册。真实桌面的生产 AppId/两个生产 verb 始终不存在，未将 QA 产物留作日常安装。

验收助手曾在卸载记录/程序消失后、最终日志刷新前读取，初次 `logCompleted=false`；等待真实日志成功并关闭后复核通过。未知值保留时，键仍存在但缺失值的 `GetStringValue` 返回 1，而不是键不存在的 2；独立枚举确认仅余夹具值，产品归属标记已移除。修正的是验收读取条件，Rust/安装器候选没有改变。冲突拒绝没有捕获退出码，不虚构数字；使用实际致命拒绝、结束日志与未覆盖注册/文件的观察判定正确拒绝。

## 系统联合流程

从真实经典对象菜单点击“使用 Nanobug 打开”，冷启动中文项目 A 并显示该目录的首次信任弹窗。Computer Use 安全规则禁止代理操作信任请求，用户亲自选择“以受限模式打开”后确认完成。读回 A 的 `workspace_trusted=false`；随后再次真实右键打开时恢复活动文件，不重复询问信任。没有改写信任记录来替代用户选择，也没有通过内部路由或直接 CLI 冒充系统入口。

最终联合轮次由实际资源管理器驱动，所有者 PID `51976`，A 的 HWND `1707348`，B 的 HWND `593406`：

| 步骤 | 可观察结果 |
| --- | --- |
| 右键 A 冷启动并编辑 | 原生 A 显示 `编辑 A.txt`；输入“右键联合验收 A 保留未保存中文”，标签出现未保存标记 |
| 再次右键 A 对象 | 仍为同一 HWND，只有一个验收项目窗口，未保存中文保持 |
| A 目录空白处右键 | 经典背景菜单实际显示该入口；点击后仍激活 A，不把父目录或进程目录作为项目 |
| 右键 B 对象 | 出现独立 B 原生窗口，A 保留；B 显示自己的文件及布局，可独立输入“右键联合验收 B 独立中文” |
| 运行中安装保护 | 实际桌面安装器日志显示检测到 Nanobug、选择 suppressed Cancel、EAbort 并结束；A/B 与未保存文本继续存在，安装程序摘要保持不变 |
| 全局退出并取消 | 从 B 请求退出，在 A 的未保存提示取消；两个窗口均保留，两份中文内容完整 |
| 保存并安全退出 | 两窗分别 Ctrl+S 后退出，PID `51976` 消失、endpoint 删除；两份磁盘文本及 A/B 历史快照读回正确 |
| 再次右键重开 | A 恢复刚保存的文本、活动文件和原布局；关闭最后窗口后验收实例退出 |

证据：`entry-install/real-joint-native-ui.json`、`real-joint-safe-exit.json`、`real-running-application-protection.json`、`real-running-unsaved.log`。Sky 在退出动作后立即枚举的一次窗口清单仍含正在关闭的窗口；随后进程、端点和新清单确认退出，不以该瞬态清单判定关闭完成。运行中安装拒绝过快，实际 PID `18236` 在取得句柄前已结束，StopTrace 订阅返回拒绝访问；因此不虚构该次退出码，使用已结束的真实 Inno 日志和窗口/文件状态验证保护。其他正常安装阶段的退出码另有真实句柄证据。

原生桌面使用隔离空插件存储。保留用的 `xml-0.2.2.zip` 只是数据夹具，不冒充已获权限的安装插件；真实插件/语言/原生程序的并行及清理由 #104 的公开 Manager、WASM 和 GPUI 集成证据证明。本单没有重复所有插件故障和输入矩阵。

## P01–P24 覆盖核对

| 场景 | 主要证据及本单增量 |
| --- | --- |
| P01 | #103 首页、#104 应用内首页；本单便携首页及十个实际次进程中的两次无参数请求复用首页 |
| P02 | #103 原生选择器与取消；#104 有其他项目时从首页打开，相关既有用例由本单完整 UI 命令回归 |
| P03 | #104 双项目持续工作；本单真实右键 B 新窗口且 A 未保存内容保持 |
| P04 | #104 目录身份与应用内去重；本单八个并发等价路径、同请求回执及真实 A 两种菜单复用 |
| P05 | #103/#104 无效目录与多窗保留；本单真实 GPUI 外部无效路径、退出/局部关闭拒绝及原生 CLI 错误 |
| P06–P07 | #103 最近项目、失效提示、移除不删文件及持久化；完整 UI 回归既有高层用例 |
| P08–P09 | #103 三种信任结果与受限编辑；#104 双项目权限隔离；本单首次真实右键、人工受限选择及记录恢复 |
| P10 | #104 三个显式 ignored 真实 actor/项目测试与运行时真实程序场景，未将空存储桌面验证冒充插件验收 |
| P11 | #103 原生队列/IME、#104 独立输入与迟到结果；本单 A/B 实际独立中文和外部请求不覆盖文档 |
| P12 | #104 公共 Manager、共享安装 lane 与主题同步；本单次进程递交后退出，不创建第二个共享数据所有者 |
| P13–P14 | #103 同窗回首页、#104 逐窗关闭及最后窗口退出；本单实际退出释放 endpoint，重开后最后窗口结束 |
| P15–P16 | #103 保存/放弃/取消、磁盘冲突与真实清理锁；#104 全局决定重验；本单取消保留和保存落盘 |
| P17 | #103 终端 PID 收尾；#104 真实 A/B 原生活动与所属关闭，未受本单入口接线改变 |
| P18 | #104 全局收集、输入重验、取消和失败重试；本单真实系统入口联合退出取消/保存 |
| P19 | #103 单项目会话、#104 两项目独立布局；本单最终安装版真实右键重开恢复，不恢复运行程序 |
| P20 | 本单最终安装版实际资源管理器对象/空白处、冷/热启动、中文/空格/`&`/`[]` 路径及同项目复用 |
| P21 | 本单真实 socket/OS 租约 GPUI 高层测试、普通次进程、独立开发实例、SDK CLI 与错误可见；半帧退出先红后绿 |
| P22 | 实际桌面首次默认勾选/取消、升级保留、重新勾选、缺少 command 重试、冲突拒绝、两次卸载及未知注册/23 文件保留均通过 |
| P23 | 最终便携候选无参数首页/明确目录请求通过，实际生产 verb 不存在；运行不注册菜单 |
| P24 | #103 中英文/主题/滚动/缩放与 #104 原生中文 IME/跨窗焦点；本单中英文实际经典对象/背景菜单、中文编辑和键盘退出通过 |

前两单证据分别见[首页验收](project-home.md)与[并行窗口验收](concurrent-project-windows.md)。普通全量 UI 回归包含受本单公共入口影响的既有用例；未选择的 ignored 测试仍不计通过。P01–P24 的分段证据已完成核对，剩余交付动作是普通提交、推送和 issue 状态读回；不以本地勾选代替远程交付。
