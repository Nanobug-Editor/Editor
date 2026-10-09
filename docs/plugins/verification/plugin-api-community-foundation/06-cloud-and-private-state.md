# 工单 06：云服务查询、密钥与后台刷新验收

日期：2026-10-09。对应 [工单 06](../../tickets/plugin-api-community-foundation/06-cloud-and-private-state.md) / [#98](https://github.com/T-miracle/Nanobug/issues/98)，来源为[社区生态方案](../../specs/plugin-api-community-foundation.md)。

状态：实现、最终自动验证与根代理 Windows 原生连接验收通过；固定候选独立双轴审查尚待执行，尚未将本单标记完成或关闭。本文只记录实际执行结果。

## 工作树、版本与构建入口

- 唯一实现工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-cloud/Editor`，分支 `codex/plugin-api-cloud`。
- 实现基线：`8506cc8f1f9baee3fca030d59b7d1d21b5227763`，已交付的 02 能力；原主目录与 01/02 其他工作树未修改。
- 工具链实际读数：rustc 1.98.1（`48a229cea`，2026-09-01）、cargo 1.98.1（`797e8a9bc`，2026-08-05）；`rust-toolchain.toml` 选择已有 stable，清单 minimum 为 1.95。已有 `x86_64-pc-windows-msvc`、`wasm32-wasip2`；Node 24.19.0、npm 11.17.0、PowerShell 7。未安装新工具或调用历史脚本。
- 公开 SDK crate `plugin-protocol 0.2.0`，wire `protocol = 7`、`api.base = ^1`；一般 HTTP、系统密钥、计时器能力分别为稳定 1.0.0，私有目录/缓存/临时区为稳定 `storage.private 1.2.0`。
- 两个新独立消费者首次清单版本均为 0.1.0；各自 Cargo 仅声明 `plugin-protocol = =0.2.0` 的 `guest` feature，未依赖仓库 `crates/` 或附带 SDK 源码。
- 所有 Rust 宿主命令设置 `$env:CARGO_TARGET_DIR = '../target06'`；插件 ZIP 使用宿主正式 `--plugin-package`，由内嵌 SDK 缓存编译实际 WASM。

最终重建命令：

```powershell
$env:CARGO_TARGET_DIR = '../target06'
cargo build -p editor-app
& ../target06/debug/editor-app.exe --plugin-package plugins/cloud-translation plugins/code-host-query --output target/cloud-api-test
cargo test -p plugin-runtime --test cloud -- --ignored --nocapture --test-threads=1
```

宿主首轮构建通过，耗时 56.27 秒；两包独立 release WASM 构建通过，分别 15.21 / 15.09 秒。十项 SDK 回归为 **9 通过、1 失败，508.00 秒**（`tests-cloud06-final.log`）；失败是服务测试 helper 对已经自动释放的终态 handle 再次关闭。修正 helper 后只重跑 T28，**1 通过、0 失败，53.71 秒**（`tests-cloud06-service-final.log`），并补充 caller 无 secrets 授权时直接读取拒绝。因此十个不同的实际 SDK 场景已有绿色结果，未将首次整套执行记录成十项全绿。

| 产物 | SHA-256 |
| --- | --- |
| 当前 `../target06/debug/editor-app.exe` | `86D4199CFE2BC73527B9260FA4B8275CEC8882F226008F9BCB05A0C2231FA54B` |
| 首轮原生宿主副本 | `2A3F5D8EA339E1FC6EF43D3C46F5EE96208810079EAA53A226F7DEBAACE02C36` |
| `target/cloud-api-test/cloud-translation-0.1.0.zip` | `67C9323D22818B543670DFDBE030B23D336FBCEFCCE1B8F31FEA3393AC97FDFD` |
| `target/cloud-api-test/code-host-query-0.1.0.zip` | `BD40C3669DF36654689FFDD91D9422405804BFEC82EF6C8BED582D9F362BC2BD` |
| `cloud_translation_guest.wasm` | `4B86C45DE6A41CAA1C6010CE992C1BA17F62DF1270D8716FA65027F54AE2B5C3` |
| `code_host_query_guest.wasm` | `A156E212E86DB3B45EA8E9E08BBA9293793BDA5170F866E54FBB1FBAB3C9D2CA` |

SDK cache key：`7923d3ec1435371c2de736686e04c423b4434fcd9201a46aa134c61811290d2f`，实际目录为 `C:/Users/Tmiracle/AppData/Local/MeEditor/plugin-sdk/<key>`。独立 build key 分别为 `d392df44fcad6573`、`b37b624e1acd5ac3`，WASM 在 `C:/Users/Tmiracle/AppData/Local/MeEditor/plugin-build/<key>/windows-x86_64/release/wasm/wasm32-wasip2/release/`。

完整本地日志均保留在工作树外、不会提交的 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-cloud/target06/logs/`。文中日志名均相对此目录；源码与验收记录仍正常提交。历史诊断日志和最终日志分开保存。

构建/包输出为 `build-host06-final.log`、`package-cloud06-final.log`；工具链、当前 EXE/两个 ZIP 与 SDK 文档 hash 的实际读数为 `fingerprints06.log`。本文的 SDK/WASM key 与 hash 来自实际构建缓存及正式产物，不以清单版本代替内容核对。

## 测试接缝与可观察结果

`crates/plugin-runtime/tests/cloud.rs` 及其 `cloud/` 子模块使用两个真实独立 SDK 构建包，经生产 `Package` 与公开 `Manager` 调用。Socket 夹具是受控 localhost TCP/真实 TLS/CONNECT，不替换 HTTP、凭据或资源清理实现。

| 场景 | 主责与断言 |
| --- | --- |
| 两消费者查询 | T25：POST Unicode 文本、GET 托管查询、原生密码输入完成消息→系统 Token、首次分块可见后再完成、缓存读取、日志/快照/整个隔离宿主根无假 Token、空工作区无私有数据 |
| 一般 HTTP | T26：PATCH 请求体、503 普通状态、缺失可靠 Content-Length 的字节限额、截止时间、未授权目标和重定向在连接前拒绝 |
| TLS/CONNECT | T26：内存 ephemeral CA、真实 TLS、正确主机名成功、错误主机名与未信任 CA 拒绝、HTTPS 降级拒绝、代理不接收 origin Token、TLS 阻塞读取消后物理 producer 回零 |
| 重定向/离线 | T26：已授权跨 origin 307 禁止非空 body 转发；GET/303 POST 清除所有 custom headers；离线返回安全传输失败 |
| 私有隔离 | T27–T29：系统凭据按插件隔离、显式删除、禁用保留、旧 handle 失效、目录 list/stat/rename/delete、共享字节/条目/名称配额、缓存持久与临时字节退役清理、真实 Windows junction 不能重定向访问 |
| profile 别名 | T27：同一 runtime 根相对/绝对/Windows extended spelling 重新打开仍读同一系统键，其他 profile 无该键 |
| 服务来源授权 | T28：原调用方缺 base/target 时不能借 provider grant；服务不得访问 provider Token/私有根；允许的一般 HTTP 不附带 Token；调用方禁用后 provider 所分配 stream 被撤销 |
| 计时/防抖 | T29：32 个重复 timer 都有 tick，第 33 个拒绝；reset 推迟旧 tick，one-shot 释放；两个 guest 的 refresh→cancel 无迟到请求且资源回基线 |
| HTTP 清理/回压 | T30：四个实际 producer，第五个准入拒绝；暂停消费导致队列回压；CancelRequest、CloseResource、disable、trust revoke 都使物理 worker 及时回零 |
| 失败更新 | T30：候选 Activate 实际开始 HTTP/timer、暂存写入后失败；旧版本和旧私有字节恢复，旧 handles 无效、候选 worker 清零、系统 Token 保留，不重放 HTTP |

生产机制复用现有实例 roots 与服务来源 context/lifetimes；没有第二套权限或所有者系统。HTTP 每次接受至多四个共享 worker，队列八个事件，每块至多 32 KiB；全程使用原截止时间。实际 TCP socket 由同一个 `Arc<TcpStream>` 保留，底层 IO 最多每 50 ms 检查撤销，复用锁定的 ureq 3.4.2 Rustls 与 CONNECT 接缝。

系统密钥调用真实 Windows Generic Credentials，所有自动测试使用独立 UUID 临时 profile/命名空间、假 bytes 和 RAII 单键清理，未枚举或读取用户真实凭据。明确平台失败没有明文回退。敏感 Debug 省略 request URL/body/header values、响应 payload 和 secret bytes；协议回归同时检查原文和 byte-array 形式没有进入 Debug。

## 真实失败、归因与修复

- 最初协议/系统凭据测试分别因公开模块和实现不存在而编译失败，随后补机制得到绿色；没有将红灯记录改为成功。
- 第一批 SDK 网络测试超时。读到实际 socket 夹具的 accepted socket 仍是 nonblocking，首个请求读取返回 WouldBlock；修正受控夹具为 blocking，不放宽生产 deadline/权限。
- `tests-cloud06-network-fixed.log` 曾为 **4 通过、4 失败，301.41 秒**：通过一般 HTTP、profile 别名、32 timers、普通 HTTP 取消；失败分别为 Windows TLS 阻塞取消、fixture 方法声明超出 caller 清单、全根扫描尚锁住 runtime 文件、私有测试配额低于合法清单最小值。后面三项分别修正测试声明、drop Manager 后扫描整个根、使用合法 1024-byte 清单配额。
- `tls-cancel06-unit.log` / `tls-cancel06-phases.log` 记录真实 TLS worker 在取消后仍为 1。Windows 复制 socket handle 的 shutdown 没有及时结束该 handle 上 pending recv；修正为同一 native socket 的 Arc，并在 Transport 内部进行 50 ms 轮询，保留原逻辑/global deadline。`transport-owned06-fixed.log` 随后为 **2 通过、0 失败，3.66 秒**，包含真实 TLS CONNECT。
- `tests-cloud06-refresh-red.log` 记录实际 SDK 的 refresh→cancel 留下 **3 个资源，基线为 2**，未把 disable 清理代替取消成功。两个 guest 的 release 现在也释放 pending debounce/input；finish_query 仅结束当前传输，保留后来安排的新刷新。最终十项 SDK 回归包含两消费者顺序验证。
- `workspace-tests06-framing-red.log` 记录普通 workspace 的 HTTP socket 回归失败：测试把夹具 framing receipt 的第三位当作完成标记，实际定义为 chunked framing。改为检查 GET/PATCH 的真实 Content-Length、非 chunked 标记及实际方法/body，`transport-owned06-final.log` 为 **2 通过、0 失败，3.66 秒**，包含真实 TLS CONNECT 取消。
- `credentials06-uuid-red.log` 记录测试 namespace 改为原生 UUID 时的编译失败：Windows GUID 没有 Display。使用其公开 Debug UUID 格式后，真实系统读取、替换、删除和隔离回归 **1 通过、0 失败，0.01 秒**（`credentials06-final.log`），RAII 精确清理自有键。
- 私有状态 SDK 夹具随后也加入显式 UUID profile，只重新运行该受影响场景：**1 通过、0 失败，73.20 秒**（`tests-cloud06-private-uuid-final.log`）。实际系统凭据/跨插件拒绝、目录配额、Windows junction 和退役清理仍通过，没有重跑另外九项未变化的 WASM 场景。
- 首轮原生 UIA 无结果 Text；公开 Role/aria_label 修复后，同源角色属性绿色，但第二轮真实 Windows Text 仍无 Name。锁定 `accesskit_windows 0.34.0` 的 `NodeWrapper::name` 对 Label 调用 `write_value`，`accesskit_consumer 0.38.0` 对 Label 规定 `label_comes_from_value`；这证实只有 label 不足。补 node.value 回归先红（Unicode 结果为 None，`native-text06-value-red.log`），生产通用 text_leaf 通过公开 aria_value 同时暴露绘制文本后 **1 通过、0 失败**（`native-text06-value-green.log`）。重建宿主 **21.83 秒**（`build-host06-a11y-value.log`）；根代理随后在 86D4 宿主实际 Windows UIA 读回 Ready 与 GET 完成的 Unicode Name，截图内容一致。异步 UIA 偶有前帧，下一次菜单焦点遍历读回当前值，未宣称每次调用立即实时。

## 门禁与网站

最终 Rust 三门禁均通过。`cargo fmt --check` 退出 0（`fmt06.log`）；非 UI workspace **233 通过、0 失败、209 ignored**，包含 doc-test 的 64 个结果 batch（`workspace-tests06.log`）；`cargo check --workspace` 退出 0，53.12 秒（`workspace-check06.log`）。编译输出保留已有 unused/dead-code 与 Windows 链接 warning，未把它们改报成无警告。普通 workspace 的 ignored 不算实际 WASM 通过；本单十项另以 `--ignored` 明确执行。

已执行针对性检查：协议 cloud **2/0**（`protocol-cloud06-final.log`），cloud capability admission **1/0**（`admission-cloud06-final.log`），应用 SDK 导出 **6/0，0.35 秒**（`sdk-export06.log`）。一般 HTTP/密钥/目录/timer 全部包含在导出的 SDK；后续只修改通用文本 accessibility 和测试 UUID，没有改变 SDK key、正式 ZIP 或 guest WASM。

两独立 guest 的 `cargo fmt --manifest-path plugins/<id>/Cargo.toml --check` 也分别退出 0；它们是 workspace 排除的正式独立项目，不能用宿主 fmt 覆盖其检查。

```powershell
cargo fmt --check
cargo test --workspace --exclude editor-app
cargo check --workspace
cargo test -p editor-app sdk_export::tests -- --test-threads=1
cargo test -p editor-app native_text_metadata_exposes_current_result_for_every_plugin -- --test-threads=1
cargo test -p platform-windows --test credentials
cargo test -p plugin-protocol --test cloud
cargo test -p plugin-runtime --test capability_packages cloud_capability_versions_are_admitted_independently_from_package_version
cargo test -p plugin-runtime --lib owned_
cargo test -p plugin-runtime --test cloud t28_services_cannot_borrow_network_targets_secrets_or_private_roots -- --ignored --nocapture --test-threads=1
cargo test -p plugin-runtime --test cloud t27_t29_private_directories_credentials_isolation_and_temporary_retirement -- --ignored --nocapture --test-threads=1
```

网站只在 `website/src/content/docs/en/sdk/cloud.md` 和中文对应页手写读者正文；SDK 导出同一英文源为 `CLOUD.md`。仓库 `docs/` 保留方案/工单/执行证据，不复制读者正文。

已有 Node/npm 执行 `npm ci` 恢复锁定项目依赖，通过；`npm test` 初次 **15 通过、2 个 build/index 前置跳过、0 失败**；随后 `npm run build` 真实构建和搜索索引完成，测试 **17 通过、0 跳过、0 失败**。对应 `website-install06.log`、`website-test06.log`、`website-build06.log`。未使用初次跳过冒充搜索通过。

## 原生连接验收与限制

根代理负责共享 Windows desktop 的顺序验收；本执行代理没有操作桌面。实际记录见[原生连接证据](native-06.md)：首轮云翻译密码遮罩/系统凭据/POST 分块/缓存/进度取消与真实 FIN/503/退出及精确凭据清理通过；86D4 宿主通用 Text 的真实 UIA Name、第二消费者 GET 两块结果和离线安全失败随后通过。所有监听器与宿主控制器正常 stop、退出 0，自有进程为空；两个明确测试键精确 API 清理读回 missing，普通 profile/控制器日志扫描无假 Token，工作区为空。固定候选双轴审查仍待根代理执行。Token 输入、进度、IME 与控件完整矩阵引用 [02 的验收记录](02-commands-and-interaction.md)，本单仅补连接与可见结果，不重复 UI 主矩阵。

两个包正常命令菜单含 set-token/query/refresh/cancel/read-cache/delete-token，测试端点分别 `http://127.0.0.1:18906/translate`（POST）和 `http://127.0.0.1:18907/repositories`（GET）。假的 ASCII Token 通过密码输入，不能直接写普通设置。原生隔离运行：

```powershell
& ../target06/debug/editor-app.exe --plugin-dev plugins/cloud-translation --release --profile <QA-profile> --workspace <empty-QA-workspace> --grant assets.read --grant network.http --grant network.http:http://127.0.0.1:18906 --grant network.http:http://127.0.0.1:18907 --grant secrets --grant storage --grant ui.interaction
```

`Manager` 根为 `<QA-profile>/runtime`。系统凭据目标为 `Nanobug/plugin/{SHA256(canonical-runtime-root UTF-8)}/{SHA256(plugin-id UTF-8)}/{SHA256(key UTF-8)}`，均小写 hex，key 为 `api-token`。路径哈希来自成功的 Rust canonicalize，Windows 保留其 extended prefix；原拼写保持 Manager.root 公开语义。`HostResources.secret_namespace` 只有可信宿主配置可覆写，插件/普通 settings 不可指定。原生辅助只可精确读取/清理自有隔离 QA 根的这两个键，不使用枚举或 GUI 删除确认。

OS DNS 是同步调用：消费者能立即收到 Cancelled/TimedOut，但进行中的 OS DNS 可能占用四个有界 worker 中一个，直至 OS 返回；前后与 connect 接缝检查撤销，禁止迟到连接，未创建无界 DNS helper。不能宣称 OS DNS 本身即时终止。Windows 以外的系统密钥接口明确返回 UnsupportedOperation，本次没有其他平台原生交叉验收。外部 HTTP 副作用不可回滚，恢复只还原受管私有数据和逻辑状态。未扩展 OAuth、AI、WebSocket 或后台任务系统。

本单实现代理不推送、不关闭 #98；根代理完成真实原生验收、固定候选双轴审查、推送后读回关闭。父设计 #92 保持开放。
