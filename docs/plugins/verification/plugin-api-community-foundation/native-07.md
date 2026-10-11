# 07：Windows 生产宿主原生验收

日期：2026-10-12（Asia/Shanghai）。本轮物理交互已完成；普通提交、推送和 #99 关闭另按[主验收记录](07-activation-and-compatibility.md)核对。父设计议题 #92 不修改。

## 固定输入与范围

- 工作树 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/Editor`，分支 `codex/plugin-api-community`。源码快照 `2515a09a0a4f2f7640d8b11d71b994571dcf379f`，实际分支当时仍为基线 `90ceab915d63c2e6b8cb260c6f28acfc45adc7fc`；快照不是普通交付提交。
- 最终格式、非 UI workspace 测试、workspace check 和生产 Host build 均 exit 0。`../target01/07-final-delivery-build-results.json` 记录实际命令与时长；生产 `../target01/debug/editor-app.exe` SHA256 为 `E81B1F9733784CCC41B436525DCAF260A5C6EE80451E9CE1DEC056E58B12DBC0`。构建前 `07-pre-final-source-verification.json` 对 `crates/`、`plugins/` 共 1236 个源码文件确认相对上述快照 delta 0。
- 生产 Host 导出的 SDK 目录为 `../target01/sdk07-native-e81b`，50 文件、0 Markdown，与上一生产 Host 导出逐文件字节一致；digest `488a6baccbb61c29dc6f5020f22bac1b813a0529dbf3a7768c9f760a9245f5fd`。实际导出 exit 0、0.3399073 秒，`07-sdk-native-final-host-export.log` 和 `07-sdk-native-final-host-export-result.json`。
- 隔离根 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/native-qa-07`；使用该根内的 profile、两个 workspace 与 query-project，不操作正常用户 profile。真实窗口 ID `28576340`，应用路径是上述生产 EXE，标题 Nanobug。
- 现有 Windows 工具链、系统 Microsoft Pinyin 与屏幕；没有安装工具。原生观察和输入通过 `@oai/sky` 的窗口截图、UIA、click、press_key、scroll 完成，未通过应用内部函数伪造物理输入。截图和 UIA 返回保留在本次工具结果中，没有声称存在另存截图文件。

本轮补足新增冷 FileView、配置热应用及相关原生交互。八个消费者的完整 T35 短组合由实际包 GPUI 用例完成，不把本轮桌面操作写成全部八包物理组合。中英文表单与两主题的组合由 App5 `native_hot_settings_apply_without_owner_restart_and_keep_only_secret_keys` 实际 GPUI Green 覆盖；物理桌面使用默认简体中文。

## 实际包与准备

| 输入 | 版本 | ZIP SHA256 |
| --- | --- | --- |
| example | 0.4.0 | `60790067D10CFA0ECFC98B6092CDCF2F6AFFA0BAC71D22633EB0C497886DE8C1` |
| SVG | 0.6.0，协议 7 | `BD40E930F38FC6195EF102E4463B426771DDB587D6C5BB2745EE684468D8B259` |
| code-host-query | 0.1.2 | `1049771D8AFA45E603901CA7F23423CA37B601BB4FEB58112E8C1AD5FB9349B7` |
| cloud-translation | 0.1.2 | `61273EF15C3750B933AD706FC92A5921B5173A98FE60A8D017E9B60CAA06B74B` |

example/SVG 来源与 byte 相同记录为 `../target01/07-reused-ui-packages.json`；本轮复用这些未变更字节。query 是宿主隔离开发项目，使用已构建实际 WASM 与当前静态项目输入；`07-native-static-project.log`、`07-native-static-project-result.json` 实际 exit 0、0.2147234 秒。translation 通过公开 Manager 冷安装，`07-native-install-translation.log`、`07-native-install-translation-result.json` 实际 exit 0、25.4380091 秒，冷安装和 shutdown 的 live/resource 均零；本轮未发 translation HTTP 请求。

复用的 QA helper `../target01/debug/community-native-qa-lifecycle.exe` SHA256 为 `2F5A43CEE1E9427808E5262A87B9E5B0C7E82D6BD2DE1F0119BAE09250D2BBDA`，直接服务任务自有 loopback，不操作 GUI。其启动命令为：

```powershell
& ../target01/debug/community-native-qa-lifecycle.exe serve 18907 18906
```

生产开发控制器实际使用：

```powershell
$qa07 = 'C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/native-qa-07'
& ../target01/debug/editor-app.exe --plugin-dev "$qa07/query-project" --profile "$qa07/profile" --workspace "$qa07/workspace-a" --grant assets.read --grant network.http --grant 'network.http:http://127.0.0.1:18906' --grant 'network.http:http://127.0.0.1:18907' --grant secrets --grant storage --grant ui.interaction
```

上述是已授权隔离开发的权限输入；本轮 GUI 不操作授权、信任、密钥或凭据设置，不执行凭据写入。密钥与私有数据安全的行为证据来自主记录的实际 SDK/Runtime 用例。控制器和 server 使用交互终端，输出保存于工具结果，没有编造控制器日志文件。

## 冷 FileView、配置与输入

1. 首次从资源管理器选中 `cold-preview.svg`，未调用插件命令或手动 Manager demand。所选原生 source 与 SVG canvas 自动出现，蓝色圆角矩形和白色圆形实际绘制。预览 zoom 操作后可见 149%；源文件没有写入。该行为与 App12 的实际 51.81 秒 Green 共同验证通用冷 FileView fallback 修复。
2. 通过原生 Settings 的插件 schema 数字输入和公开 Apply，query timeout 设为 `120000`、来源用户全局；poll interval 从 `1000`、本项目进一步改为 `500`、本项目。完成后的来源显示“已确认项目配置”，Apply 重新可用，timeout 仍显示用户全局。没有修改 user-only keyRef；只观察到其只读标签。
3. Settings 的长表单经过真实滚动定位与操作。浏览设置自身不作为首次业务激活的证据；冷浏览断言另由 App5 GPUI 实际验收覆盖。
4. Notes 使用真实 Microsoft Pinyin：`n` 显示“1 你”候选；Return 先提交字面拼音 `n`，随后再输入 `n` 并按 `1` 选择“你”。最终原生 Input 与公开 Guest 回显均为 `nn你`。这包含实际 IME 候选和选择，不把 `type_text` 直接注入中文当作 IME 验收。
5. 在 Notes Input 点击后按 Tab，主窗口 UIA 焦点是“按钮 增加一次”；按 Return，插件计数由 0 变为 1。Notes、query 缓存结果与 SVG source/preview 均保留，证明实际键盘焦点与公开插件命令贯通。

## 原始流、热应用、取消与缓存

服务端只绑定 `127.0.0.1:18907` 和 `:18906`。第一块为 `first 世界😀\n`（17 UTF-8 bytes），第二块为 `second 分块\n`（14 bytes）。`received` 统计已完整读取的 HTTP 请求，不能解释为 TCP accept 数；认证只统计 Authorization header 是否存在，不记录其内容。

以下 UTC 时间为实际观察，不将后来超时的请求误报为手动取消：

| 请求 | 实际行为与边界 |
| --- | --- |
| 1，21:44:24 开始 | 首块 HTTP 200 / 17 bytes 与 Receiving 进度可见。定位设置耗时，120 秒绝对 deadline 在 Apply poll1000 前已到期；此请求不计作“热应用保留流”或手动 Cancel。 |
| 2，21:47:58 开始 | 首块 17 bytes 与 Receiving 可见；21:49:09 点击公开 Apply500Project。21:49:48 新值500、已确认项目配置、Apply可用时，原 HTTP 200 / 17 bytes 和 Receiving 仍存在；server 累计 received2/disconnected1/completed0，热应用期间没有额外 HTTP。之后在 Root 操作下一步骤前超过 deadline，自然取消，不计手动 Cancel。 |
| 3，21:50:39 开始 | 首块与 Receiving 可见；21:50:58 点击原生进度卡片 Cancel，仅19秒，早于120秒 deadline。随后显示“已取消”、进度撤销，server 累计 received3/disconnected3/completed0。server `continue` 后 UI 仍已取消，没有迟到第二块或新结果。 |
| 4，21:51:57 开始 | 首块17 bytes实际可见，server `continue` 后 HTTP200 / 31 bytes、两块中文及非BMP文本完整显示，进度消失；server received4/completed1/disconnected3。 |

通过公开面板菜单 Read cache，实际显示 `first 世界😀` 和 `second 分块` 两行正文；没有渲染“Cached31”标签，不补造该标签。server 请求数仍4，没有缓存读取触发的新 HTTP。最终磁盘缓存31 bytes与两块UTF-8原文逐字相等，见下文。

最终 server18907 为 `received=4, completed=1, disconnected=3, authorization_headers=0, rejected=0`；server18906 五项均0。后者只证明此物理轮次没有 translation HTTP，不替代主记录中的双消费者自动行为。

## 主题、缩放、布局与基础无障碍

- 初始浅色、中文14px界面实际可见。原生 Appearance 切到深色，Settings、插件面板和主窗口颜色变化，SVG canvas149%、Notes `nn你` 和查询缓存正文保持。
- 原生 Editor Settings 的 `+` 连续把字体14→15→16px，每次后重新取窗口状态确认真实数值与布局；没有借配置文件写入代替 GUI。
- 最大化窗口2560×1392，之后点击还原，真实窗口1283×821。source自动折行、预览仍有完整蓝色矩形与白圆，Notes、计数1、查询两块仍可见，未丢失面板或叠盖输入。
- 主窗口 UIA 实际返回 tree/tree item、edit、button、tab、checkbox、radio group、progress 等角色，以及 Notes、计数、查询正文；Tab 后焦点与 Return 行为均观察。基础可访问角色和结果可读，不宣称完成屏幕阅读器专项认证。
- 03/04 已记录的 Settings UIA 枚举触发退役崩溃限制仍保留；本轮 Settings 使用截图读取，不枚举 Settings UIA。主窗口枚举成功，不把它扩大为 Settings 问题已修复。
- 插件错误面板保留 QA 准备阶段时间 `2026-10-12 05:33:03` 的 example/cloud-translation disabled 与 Image Preview disabled 信息；本次后续 source/preview、Notes 和 query 实际正常。没有删除历史记录或把它说成当前新故障。

## 正常停止及磁盘结果

分别向开发控制器和 QA server 输入公开 `stop`，两终端均实际 exit0；没有强杀。2026-10-11T22:00:26.9721095Z 的 `../target01/07-native-final-cleanup-result.json` 核对 exact Host/任务QA关联进程0、18907/18906监听0；Sky窗口列表也已无 Nanobug。

`07-native-workspace-before-final.json` 与最终结果的四个任务自有文件 byte/hash 全部一致：

| 文件（相对 QA 根） | bytes | 前后 SHA256 |
| --- | --- | --- |
| `workspace-a/review.nbexpr` | 56 | `AE546B394B5E9BA60FBED16E1754C00FBE1916423E9FDEA5C68D28627EF3479C` |
| `workspace-a/review.nbkv` | 49 | `9F2F8814B1909D15F9D2F6489C2EC54F70421BF9C19C0C3850B70BDEA3A18639` |
| `workspace-b/scope.txt` | 76 | `718AEC54EEC1B10A5F61E2050D5F66A25A56AC871051AC41D7CE916D6FD95947` |
| `workspace-a/cold-preview.svg` | 324 | `E61619522934D2D90E4C3E6242477EB3B270A39172B4F65854E6493AF83418DC` |

缓存位于隔离 profile 的 `runtime/data/code-host-query/workspaces/b80c72346465a7bd752cc56d30b2c8ca25fca33342c6f5da43571f5c55caee7a/files/.cache/last-query.txt`，31 bytes，SHA256 `2BCF8DA5E999AD6B137CB09A6F18C18708B909A4680445C4AFAB7DE7D890F0C0`，严格 UTF-8 解码后等于两块原文。缓存未写入项目目录；未输出或持久化 Git/云服务凭据。

实际平台范围为本机 Windows。Linux/macOS、物理英文界面及 Settings UIA 专项不在本轮物理通过范围；中英文和主题的新增 schema 表单已有上述实际 GPUI 验收。历史平台限制随各前单记录保留。
