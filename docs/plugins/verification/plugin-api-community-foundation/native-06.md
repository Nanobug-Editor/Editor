# 06：主代理 Windows 原生验收记录

日期：2026-10-09，续验及交付 2026-10-11。状态：两类查询插件的原生连接、结果文本辅助语义、最大合法响应和当前主分支/源码 SDK/原生终端连接均已通过，固定候选双轴审查和远端交付也已完成；交付点为 `1c5cbe697ebf37f1497bac1b7b8d08ca7f972876`，#98 已读回 `closed/completed`。原生操作和观察由根代理完成，实施代理按授权补记交付状态；历次输入及失败观察保留原时点。

## 输入与隔离

- 原生宿主副本：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-cloud/native-qa-06/bin/editor-app-cloud-native.exe`，SHA256 `2a3f5d8ea339e1fc6ef43d3c46f5ee96208810079eaa53a226f7debaace02c36`。
- 内嵌 SDK 内容摘要：`7923d3ec1435371c2de736686e04c423b4434fcd9201a46aa134c61811290d2f`，wire 7、SDK 0.2.0。
- 首轮实际消费者：cloud-translation 0.1.0，以宿主 `--plugin-dev plugins/cloud-translation --release` 构建并激活；开发候选 `758d6b74436e4c1ae5801cd3ea82d32a57b5fb1976476988beaea1eb73afb3ff`。
- 原生候选 WASM SHA256 `4b86c45de6a41caa1c6010ce992c1ba17f62df1270d8716fa65027f54ae2b5c3`，与正式 ZIP `67c9323d22818b543670dfdbe030b23d336fbcefcce1b8f31fea3393ac97fdfd` 内的 WASM 字节相同。
- 新配置 `native-qa-06/profile-translation`、空工作区 `native-qa-06/workspace-translation`；TTY 开发控制器日志 `native-qa-06/translation-controller.log`。
- 授权逐项为 `assets.read`、`network.http`、`network.http:http://127.0.0.1:18906`、`network.http:http://127.0.0.1:18907`、`secrets`、`storage`、`ui.interaction`。没有操作日常配置或其他桌面上的验收实例。
- 本机端点只监听 loopback 18906/18907。夹具在工作树外 `native-qa-06/NativeCloudFixture.cs`；首块后等待控制器继续，支持 503 和真实 socket FIN 计数，不打印原始 Authorization。
- 仅输入固定的合成 ASCII Token。清理辅助限定本次 QA 根、两个明确插件与 `api-token` 键，按 Windows final path 构造与 Rust canonicalize 相同的目标；只在 bytes 等于预期假值时删除，不枚举其他凭据。

## 首轮实际观察

1. 从云翻译面板菜单打开“安全输入密钥 / Set token”。原生 Edit 名称为“API 密钥”，键入假值后画面只显示密码圆点，UIA 不暴露明文 Value。Return 确认后面板显示“密钥已安全保存”。
2. 精确 Windows Credential API 读取该 QA 命名空间，结果 `present=true; fakeMatched=true`；同一 QA 根的 code-host-query 键不存在，未读取真实用户密钥。
3. 实际菜单发起 POST 查询，进度有可访问的 Status/取消 Button。端点尚未继续时，画面显示 `HTTP 200 · 17 bytes` 与 `first 世界😀`，第二块尚未出现；端点计数为收到 1、完成 0、假 Authorization 匹配 1、POST 1。
4. 继续端点后画面显示 `HTTP 200 · 31 bytes · 已完成`、首块和 `second 分块`，进度撤下。端点完成计数 1。通过“读取缓存 / Read cache”，画面只显示相同的两块文本；缓存位于宿主私有 `.cache/last-query.txt`，空工作区没有新增内容。
5. 再次以首块后等待的端点发起查询，点击进度中的真实取消按钮。进度撤下，下一稳定画面显示“已取消”；端点观察到真实断开，计数为收到 2、完成 1、断开 1，没有第二块或迟到完成覆盖取消结果。
6. 将受控端点改为 503 后再次查询，画面显示 `HTTP 503 · 30 bytes · 已完成` 和 `controlled service unavailable`；非 2xx 状态没有伪装成网络成功状态 200。三次请求都携带预期假 Authorization，均为 POST。
7. TTY 发送 `stop` 后开发控制器退出码为 0，精确自有宿主副本的进程列表为空。停止后 OS 假键仍存在；精确 API 删除返回成功，再次读取 missing。普通 profile 全部文件及控制器日志的二进制文本扫描没有合成 Token；工作区仍为空。

## 原生发现与后续范围

首轮截图能看到结果内容，但多次稳定画面的完整 UIA tree 只暴露按钮、菜单、密码 Edit 和进度，没有云结果 Text/Value。实施代理当时确认通用 `ui::Kind::Text` 渲染没有 Role/label，并计划沿公开通用文本接缝修复及补属性红绿回归，不加入云插件专属分支；后续完成结果见下节。

中间候选 `7727b514be6cb917ac8c6e0231c7af5694a790289ac8f4b96e3fd0eaa1534ba7` 增加了 Text 的 Label role/label。Windows 能枚举“文本”，其 Name/Value 仍为空，不能作为辅助语义通过证据。该候选只完成 code-host-query 的原生密码输入；正常停止后沿同一隔离 profile 继续复核，没有重复写入密钥。

## 修复后 GET 与辅助语义复核

- 最终复核宿主副本 `native-qa-06/bin/editor-app-cloud-value.exe`，SHA256 `86d4199cfe2bc73527b9260fa4b8275cec8882f226008f9bcb05a0c2231fa54b`。通用 Text 通过公开 `aria_value` 提供与绘制内容相同的值；锁定的 Windows AccessKit Label 名称来自 value。
- SDK 摘要仍为 `7923d3ec1435371c2de736686e04c423b4434fcd9201a46aa134c61811290d2f`，两个正式包与首轮的 WASM 均未改变。第二消费者 code-host-query 0.1.0 实际开发候选为 `1f7cfc6cbe2dac06b06d58852938cb31b3047471d115ffe08ffe8c2cfef9a58c`，WASM SHA256 `a156e212e86db3b45ea8e9e08bba9293793bda5170f866e54fbb1fbab3c9d2ca`。
- 复用独立 `profile-code-host` 与空 `workspace-code-host`，TTY 日志 `native-qa-06/code-host-value-controller.log`；Windows 精确 API 确认该命名空间假键存在。同根 cloud-translation 键不存在。

1. 新宿主就绪后，Windows UIA 实际枚举 `文本 云查询已就绪`，Name 内容与画面相同。
2. 真实菜单发起第二类 GET 查询。端点继续前，画面显示 `HTTP 200 · 17 bytes` 与 `first 世界😀`；端点计数为收到 1、完成 0、假 Authorization 匹配 1、GET 1、POST 0。
3. 继续端点后画面显示 `HTTP 200 · 31 bytes · 已完成`、首块与 `second 分块`，进度撤下；端点完成 1。随后菜单焦点遍历的完整 UIA tree 读出 `文本 HTTP 200 · 31 bytes · 已完成 first 世界😀 second 分块`。
4. 受控 loopback 监听器正常停止、退出码 0 后，再从真实菜单发起请求。稳定画面与下一次菜单遍历的 UIA Text 均为 `OperationFailed: HTTP transport failed`，进度撤下；错误没有泄露 Token 或覆盖为成功。
5. 开发控制器正常 `stop`，退出码 0，精确宿主副本进程列表为空。OS 假键在停机后仍存在；精确删除成功，再读 missing。整个普通 profile、两份控制器日志的二进制扫描无合成 Token；空工作区没有新增文件。私有缓存保留成功 GET 的两块文本。

即时异步画面后的首次 UIA tree 曾保留前一帧文字与进度节点；上述正确结果来自随后实际菜单焦点遍历。这里证明通用结果文本具有可读 Windows Name，未将这一观察扩大为每次异步更新均已验证实时播报。完整控件矩阵引用 02；未改变的密码输入、POST、缓存与取消接缝复用首轮同 SDK/包证据。HTTP/TLS/代理、来源权限、计时/私有状态及失败更新详测归逐单自动矩阵；该历史阶段不作为最终双轴审查与整单交付证据，后续固定点结果见“2026-10-11”部分及逐单交付记录。

## 审查修复后：最大合法响应与滚动

- 新宿主副本 `native-qa-06/bin/editor-app-cloud-reviewed.exe`，SHA256 `95c21c8a1d72d9c8f3dc991fb040fe607ba7c886aad920363168f76bf0f6f86b`；SDK 摘要仍为 `7923d3ec1435371c2de736686e04c423b4434fcd9201a46aa134c61811290d2f`。
- code-host-query 0.1.1 使用同一公开 `--plugin-dev ... --release` 入口，独立 `profile-code-host-reviewed` 和空 `workspace-code-host-reviewed`。实际候选 `74847847fe64410687e1a09090f97d79f6283946519179fd5383bbd0f1a4ea21`；WASM SHA256 `df63441354924b653906417f293f429b449b94b07d8a07178bc72b0ed73e59ac`，与正式 0.1.1 ZIP `ef05f4b947ff01f8de024fcce7b84ff40078354119b766c155179e54c10dca5a` 的记录一致。
- 受控端点通过四个 16 KiB 网络块返回恰好 65,536 字节 UTF-8 正文，包含中文、非 BMP 字符、首尾标记和多行；网络块边界不保证与 Unicode 或 UI 分片边界一致。本轮没有写入 Token，端点的假 Authorization 匹配数为 0，符合预期。

1. 真实插件菜单发起 GET。完成画面为 `HTTP 200 · 65536 bytes · 已完成`，首段显示 `BOUNDARY_FIRST 世界😀`；进度撤下，端点计数收到 1、完成 1、GET 1、POST 0。
2. 沿公开 `Scroll(result)` 滚动到末尾，实际画面能看到完整末尾文字。状态前缀使末尾标记跨两个 Text 分片显示为 `BOU` 和 `NDARY_END 65536 bytes 已完成😀`，没有丢失字符。稳定 Windows UIA tree 枚举三个非空 Text Name，包含完整首尾；输出仅摘录首尾和长度，未倾倒大正文。
3. 完成后真实菜单仍可打开；点击“读取缓存 / Read cache”后，UIA 的两个 Text Name 包含原始正文首尾，未将状态前缀写入缓存。停止后严格 UTF-8 解码成功，缓存长度恰好 65,536；SHA256 `8955d95861d723df83fcaf8be5a28f8f89e041133ef1d6895fc9e3425f19f0c3` 与按夹具规则重新生成的全部原始 bytes 完全相同。
4. 开发控制器和两个自有监听器均正常 `stop`、退出 0，精确宿主副本进程数量为 0，工作区条目为 0。精确 OS API 确认新 QA 命名空间两个键均不存在；普通 profile 与控制器日志扫描无假 Token（`rg` 退出 1，表示无匹配）。

共享桌面验收期间有其他独立 QA 窗口获得焦点，以及 Computer Use 的过期元素/截图或 active-request 提示。主代理丢弃旧输入目标并重新选择精确自有 EXE 后观察，没有操作其他实例；这些工具状态不算应用功能失败。原生大响应覆盖新增滚动接缝，两个消费者的近限额、满限额、无效 UTF-8、取消与完整缓存矩阵由真实 SDK 针对测试承担，未重复未变化的密码输入或网络权限主矩阵。

## 2026-10-11：主分支整合、源码 SDK 与原生终端的当前连接

本轮生产输入固定为 `f2a907a6a6348aedd3b2f1a37681c4fda1f8da7c`；纯文档候选 `f10100b05d74434d2aa4b823c6fc72ba8c406fb5` 的 `crates/`、`plugins/` 与该点完全相同。最终独立双轴审查覆盖 `de5397645d05c5d91e8cd74108d46ad9d7a88ff3...f2a907a6a6348aedd3b2f1a37681c4fda1f8da7c`：Spec 遗漏/范围外/错误为 0/0/0；Standards 硬项 0、判断性 P3 重复实现 1，已按逐单记录保留两个独立示例的复用权衡，没有未解决的行为违反。

- 宿主副本：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-cloud/native-qa-06-main/bin/editor-app-cloud-main.exe`，SHA-256 `4B220DC26957AFE0E181D31187A4F82FF452C7AF495AEF255F81D63CC4003131`。
- 当前内嵌 SDK 为 42 项源码资源、0 Markdown，摘要 `aa89f9a713a00439bf79c329b9db51758e24d9a1a6b3417f15df963da3208f76`。SDK、正式包和三门禁的完整输入见[逐单记录](06-cloud-and-private-state.md)。
- 正式 `code-host-query` 0.1.1 ZIP 为 `8E245135A2EC3745B8D5195E3C4F48378DC0746B3156D91B6FEB7E438776A6C4`，WASM 为 `45FE738FDAF9A9EF8A5356F86C0397EC7713F632EEB750C949C4E7EBBA9850CD`；静态开发项目只描述 ZIP 内原始清单与相同 WASM 的资源复制，实际候选 digest 为 `43945449b3a1babc9eb85c5cf43720444c8f157612b31a73dd147ca656c92bd3`。
- 同一实例通过既有 EXE 相对 `bin/plugins/` 引导正常安装 `cloud-translation` 0.1.1，ZIP 为 `C1D6D01DD5D725F1686E71BEBFFCE8F1CDBD8CFFB8CAED64665314D096A0C67C`。两个面板在同一个公开 Manager、`native-qa-06-main/profile` 和 `native-qa-06-main/workspace` 中共存；不添加安装桥、空组件或专属 API。
- 单项目开发入口为 `--plugin-dev native-qa-06-main/query-project --release --profile native-qa-06-main/profile --workspace native-qa-06-main/workspace`，逐项授权清单声明的 assets/network/secrets/storage/interaction 权限。控制器使用 TTY 和 `controller-main.log`。受控端点仅监听 loopback 18907，并且只输出计数。

实际操作和读回：

1. 通过正常菜单的密码 Input 输入同一明确合成 Token，遮罩显示，确认后 UIA 和画面均显示“密钥已安全保存”。精确 OS API 后续确认查询插件键 `present=true;fakeMatched=true`，同根翻译插件键不存在。第一轮输入曾在思考期间超过真实 60 秒截止，Dialog 自动退役并显示“密钥输入已取消”；旧元素点击不可用后重新观察、重新发起，未放宽超时或沿用退役句柄。
2. 真正发起 GET 后，画面和稳定 UIA 显示 `HTTP 200 · 65536 bytes · 已完成`、`BOUNDARY_FIRST 世界😀`；滚动后能看到 `BOU` 与 `NDARY_END 65536 bytes 已完成😀`，分片相接保留所有字符，未出现替换字符。菜单保持可用，服务计数为收到 1、完成 1、假 Authorization 匹配 1、GET 1、POST 0。
3. 正常 `Read cache` 后画面显示无状态前缀的原始正文，稳定 UIA 包含完整 `BOUNDARY_END 65536 bytes 已完成😀`。请求数保持 1，未再发网络请求。缓存位于宿主私有 `files/.cache/last-query.txt`，长 65,536 字节，SHA-256 `8955D95861D723DF83FCAF8BE5A28F8F89E041133EF1D6895FC9E3425F19F0C3`，与前述独立生成的全部正文相同。该次组合 UI 工具调用曾挂起；停止读取、重置工具绑定、重新枚举唯一自有 EXE 后，同一窗口状态读取正常返回。以上通过依据是恢复后的真实界面、文件和服务计数，不是挂起调用。
4. 端点首块后等待，再次发起 GET，实际画面显示 `HTTP 200 · 17 bytes`、`first 世界😀` 和具有“取消”名称的进度按钮。点击该按钮后进度撤下、稳定画面/UIA 为“已取消”；服务计数收到 2、完成 1、断开 1、假 Authorization 匹配 2，无迟到完成。成功缓存仍保持上述 hash。
5. 从本地终端按钮打开原生 PowerShell 标签，实际终端显示当前 QA 工作区的 `PS ...>` 提示符；两插件面板仍共存、查询取消终态仍可见。没有通过 GUI 向 Shell 输入命令。
6. TTY 控制器和受控监听器均正常 `stop`、退出 0；随后自有 EXE 进程数和窗口数都为 0。精确 OS API 仅在 bytes 匹配合成值后删除查询键，读回两个明确键均 missing；6 个私有数据/控制器日志文件的扫描中，合成 Token 明文匹配文件为 0。

工作区唯一文件 `query-current.txt` 的前后 SHA-256 均为 `134CDC78027819E07CDAF7D3FF2D69337F03BE7F64B03B7C9F4DF770104448E8`。实际中文和 emoji 输入/滚动已观察；未变的完整 IME、深浅主题、HTTP/TLS、权限负面与更新矩阵依照各自固定输入的主责记录复用，本轮不冒称重新执行。该当前连接已经通过，交付点 `1c5cbe697ebf37f1497bac1b7b8d08ca7f972876` 的远端完整 SHA 和 #98 `closed/completed` 也已读回，关闭时间为 `2026-10-11T06:28:28Z`；详见[逐单记录](06-cloud-and-private-state.md)的“远端交付读回”部分。此后仅补记 Markdown，不改变本轮生产输入、产物或原生观察。
