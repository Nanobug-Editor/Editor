# 工单 06：云服务查询、密钥与后台刷新验收

日期：2026-10-09，续验 2026-10-11。对应 [工单 06](../../tickets/plugin-api-community-foundation/06-cloud-and-private-state.md) / [#98](https://github.com/T-miracle/Nanobug/issues/98)，来源为[社区生态方案](../../specs/plugin-api-community-foundation.md)。

状态：首候选的自动验证与 Windows 原生连接验收通过；固定候选 `69478ced8238e8f2e9ce67d2909e901387ac9109` 的独立审查为 Standards 0 项、Spec 4 项（最高 P1）。四项已在 `d92f79c8957063d1f77a5600f4480baed50859e0` 修复并通过复审，Standards 仍为 0 项；Spec 新发现一项 P2：失败配置候选清理旧实例的 Temporary。续修候选 `a248ccfb1ce9281a989f4888f469a0a25bf47c87` 已补实际 SDK 红绿回归与纯源码出口，固定点双轴复审为 Spec P1/P2/P3 = 0/0/0，Standards = 0/0/1，仅一处注释措辞。注释在 `54272a6261a3cd7c9b726aa6578d3a4392d94c47` 修正，独立 Standards 读回为硬项 0 / smell 0，Spec 保持 0/0/0。现整合根代理验证的 `de5397645d05c5d91e8cd74108d46ad9d7a88ff3`；整合候选的固定点复审与当前产物原生短连接仍由根代理完成。本单尚未标记完成或关闭，本文保留各阶段的实际证据。

## 工作树、版本与构建入口

- 唯一实现工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-cloud/Editor`，分支 `codex/plugin-api-cloud`。
- 实现基线：`8506cc8f1f9baee3fca030d59b7d1d21b5227763`，已交付的 02 能力；原主目录与 01/02 其他工作树未修改。
- 工具链实际读数：rustc 1.98.1（`48a229cea`，2026-09-01）、cargo 1.98.1（`797e8a9bc`，2026-08-05）；`rust-toolchain.toml` 选择已有 stable，清单 minimum 为 1.95。已有 `x86_64-pc-windows-msvc`、`wasm32-wasip2`；Node 24.19.0、npm 11.17.0、PowerShell 7。未安装新工具或调用历史脚本。
- 公开 SDK crate `plugin-protocol 0.2.0`，wire `protocol = 7`、`api.base = ^1`；一般 HTTP、系统密钥、计时器能力分别为稳定 1.0.0，私有目录/缓存/临时区为稳定 `storage.private 1.2.0`。
- 两个新独立消费者首次清单版本均为 0.1.0，本轮修复交付均提升为 0.1.1；各自 Cargo 仅声明 `plugin-protocol = =0.2.0` 的 `guest` feature，未依赖仓库 `crates/` 或附带 SDK 源码。协议及能力版本不变。
- 所有 Rust 宿主命令设置 `$env:CARGO_TARGET_DIR = '../target06'`；插件 ZIP 使用宿主正式 `--plugin-package`，由内嵌 SDK 缓存编译实际 WASM。

首候选构建与十项回归命令：

```powershell
$env:CARGO_TARGET_DIR = '../target06'
cargo build -p editor-app
& ../target06/debug/editor-app.exe --plugin-package plugins/cloud-translation plugins/code-host-query --output target/cloud-api-test
cargo test -p plugin-runtime --test cloud -- --ignored --nocapture --test-threads=1
```

宿主首轮构建通过，耗时 56.27 秒；两包独立 release WASM 构建通过，分别 15.21 / 15.09 秒。十项 SDK 回归为 **9 通过、1 失败，508.00 秒**（`tests-cloud06-final.log`）；失败是服务测试 helper 对已经自动释放的终态 handle 再次关闭。修正 helper 后只重跑 T28，**1 通过、0 失败，53.71 秒**（`tests-cloud06-service-final.log`），并补充 caller 无 secrets 授权时直接读取拒绝。因此十个不同的实际 SDK 场景已有绿色结果，未将首次整套执行记录成十项全绿。

| 产物 | SHA-256 |
| --- | --- |
| 首候选通用 Text 原生复核宿主 | `86D4199CFE2BC73527B9260FA4B8275CEC8882F226008F9BCB05A0C2231FA54B` |
| 首轮原生宿主副本 | `2A3F5D8EA339E1FC6EF43D3C46F5EE96208810079EAA53A226F7DEBAACE02C36` |
| `target/cloud-api-test/cloud-translation-0.1.0.zip` | `67C9323D22818B543670DFDBE030B23D336FBCEFCCE1B8F31FEA3393AC97FDFD` |
| `target/cloud-api-test/code-host-query-0.1.0.zip` | `BD40C3669DF36654689FFDD91D9422405804BFEC82EF6C8BED582D9F362BC2BD` |
| `cloud_translation_guest.wasm` | `4B86C45DE6A41CAA1C6010CE992C1BA17F62DF1270D8716FA65027F54AE2B5C3` |
| `code_host_query_guest.wasm` | `A156E212E86DB3B45EA8E9E08BBA9293793BDA5170F866E54FBB1FBAB3C9D2CA` |

SDK cache key：`7923d3ec1435371c2de736686e04c423b4434fcd9201a46aa134c61811290d2f`，实际目录为 `C:/Users/Tmiracle/AppData/Local/MeEditor/plugin-sdk/<key>`。独立 build key 分别为 `d392df44fcad6573`、`b37b624e1acd5ac3`，WASM 在 `C:/Users/Tmiracle/AppData/Local/MeEditor/plugin-build/<key>/windows-x86_64/release/wasm/wasm32-wasip2/release/`。

完整本地日志均保留在工作树外、不会提交的 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-cloud/target06/logs/`。文中日志名均相对此目录；源码与验收记录仍正常提交。历史诊断日志和最终日志分开保存。

首候选构建/包输出为 `build-host06-final.log`、`package-cloud06-final.log`；该轮工具链、EXE/两个 ZIP 与 SDK 文档 hash 的实际读数为 `fingerprints06.log`。本文的 SDK/WASM key 与 hash 来自实际构建缓存及正式产物，不以清单版本代替内容核对。

## 固定候选审查与修复

根代理对 `69478ced8238e8f2e9ce67d2909e901387ac9109` 完成两条独立审查轴。Standards 无发现；Spec 确认以下四项。原候选未重写，修复追加普通提交 `d92f79c`；根代理随后在该固定点确认四项解决、Standards 0 项，并发现下面续修节的 Temporary P2。评审结论分别对应实际固定点，不跨候选复用。

| 发现 | 可观察红灯与修复 |
| --- | --- |
| T29 / P2：回调内 reset 已捕获的另一 timer | 独立 `timer-callback` SDK guest 先把 A/B 都设为 due，A 的 Timer 回调公开调用 `Reset(B, 300)`。旧实现同一 poll 仍递送 B，实际事件为 `a-reset-b, b-tick`；`timer-callback06-red.log` 为 0/1、12.95 秒。现在递送前重新解析当前 owned handle、核对实例 handle 与当前 deadline，再增加序号或释放 one-shot。`timer-callback06-green.log` 为 2/0、60.11 秒，覆盖 one-shot/repeat 回调 reset 及原有 32 timer 公平性、防抖取消。 |
| T29 / P2：cache/temp 相对深度绕过共享 root 深度 | 旧实现接受 `.cache` 下相对 16 层目录，使整个 data root 深度变成 17；`private-depth06-red.log` 为 0/1、19.21 秒。现在新增入口和必要子树在变更前统一按 canonical data owner 计深度，rename 复用相同计算。`private-depth06-green.log` 为 1/0、24.63 秒：Data 16 层、Cache/Temporary 15 层可用，下一层 CreateDirectory 与普通 WriteFile 均拒绝，原树仍可 list/write/delete。 |
| T27/T30 / P1：candidate Snapshot 能借已 flush 的 data 状态修改 OS Token | 独立 `secret-candidate` SDK guest 在候选 Snapshot 中调用 Write/Delete，随后 Snapshot 失败或在公开 Committing barrier 取消发布。旧实现实际替换了原 Windows Generic Credential；`secret-publication06-red.log` 为 0/1、42.60 秒。增加仅可信 Manager 可打开的 committed activation 状态；prepare、Activate、commit_data、Snapshot 与 data swap 都不打开不可回滚的密钥修改。仅 durable registry/data transaction/settings publication 成功后开放，quiesce/restart 先撤销；恢复已发布版本完成重新激活后再开放。`secret-publication06-green.log` 为 1/0、165.02 秒，四种失败组合的系统键均保持原值，恢复、disable/enable、settings 更新和重新打开后正常 Write/Delete 仍可用。没有将 Token 备份到普通事务文件。 |
| T25 / P2：合法 65,536-byte body 加状态前缀或 lossy UTF-8 膨胀超过单 Text 配额 | 旧 guest 对 65,530-byte Unicode body 已被宿主 UI 配额拒绝并退役，`response-display06-red.log` 为 0/1、40.21 秒，明确记录 `UI text quota exceeded`。两个 0.1.1 guest 按 Unicode 边界分为至多 32 KiB 的 Text，保持完整 raw cache；大结果使用公开 `Scroll(result).grow → Column(result-body) → Text(result-part-N)`。初步 Column 版本完整内容回归 1/0、44.55 秒，但新可滚动契约断言实际红灯 0/1、59.13 秒（`response-scroll06-red.log`）。最终 Scroll 版本为 1/0、59.95 秒（`response-scroll06-green.log`），两个消费者均覆盖 near 65,530、max 65,536、非 UTF-8 65,536-byte body，比较完整 Unicode 显示、SDK 读取的完整 raw 缓存、实例保持活动及随后 Cancel/资源回落。未放宽 UI 配额。 |

两新增验收 guest 也通过正式宿主 `--plugin-package` 独立构建，均为 0.1.0，使用同一缓存 SDK。`timer-callback` / `secret-candidate` 的 build key 为 `00f4e484a15467cf` / `8f26becd04ca5d95`；构建输出分别保留在 `package-timer06-review.log`、`package-secret06-review.log`。先前夹具清单缺 quota、缺 Snapshot 和滚动断言缺 ui import 的准备错误分别保留在 `package-timer06-manifest-red.log`、`timer-callback06-fixture-red.log`、`response-scroll06-fixture-red.log`，没有把它们当作行为红灯。

`d92f79c` 修复宿主 `cargo build -p editor-app` 退出 0，51.59 秒（`build-host06-review.log`）。两个正式 0.1.1 包由首候选的 86D4 宿主内嵌、未变化的 SDK 编译；最终 Scroll 包 release 构建分别 9.30 / 8.62 秒，记录在 `package-cloud06-scroll-review.log`。从正式 ZIP 直接读取 WASM 并核对的 hash 在 `package-fingerprints06-review.log`：

| `d92f79c` 修复产物 | SHA-256 |
| --- | --- |
| `../target06/debug/editor-app.exe` | `95C21C8A1D72D9C8F3DC991FB040FE607BA7C886AAD920363168F76BF0F6F86B` |
| `target/cloud-api-test/cloud-translation-0.1.1.zip` | `70882C90C8E37F7292E6C07B762F028E1A195CC96CAC52AB85D459F22BB9EDC7` |
| ZIP 内 `cloud-translation.wasm` | `D81BF24C22E856EEFB616B394906ECB3FCC43DB6CDD993968B434FE93911E1B4` |
| `target/cloud-api-test/code-host-query-0.1.1.zip` | `EF05F4B947FF01F8DE024FCCE7B84FF40078354119B766C155179E54C10DCA5A` |
| ZIP 内 `code-host-query.wasm` | `DF63441354924B653906417F293F429B449B94B07D8A07178BC72B0ED73E59AC` |

SDK cache key 仍为 `7923d3ec1435371c2de736686e04c423b4434fcd9201a46aa134c61811290d2f`，两个消费者 build key 仍为 `d392df44fcad6573` / `b37b624e1acd5ac3`。本轮没有改变公开契约、SDK/网站源或通用 Text presenter；不重复未变化的完整 HTTP/TLS、Token、UI 主矩阵与 SDK/站点输出矩阵。首候选原生 86D4 结果只作为对应旧内容的证据，新的大结果/滚动连接另由根代理以实际新 EXE/WASM 验收。

新宿主另执行一次 `--export-plugin-sdk ../target06/sdk06-review`，退出 0；导出的 **63 个文件**全部与 7923 SDK 缓存对应文件 hash 相同，0 差异。`CLOUD.md` SHA-256 为 `87D524154427FCF1A0B3D920A747A6FE26283A3FE052AF3D84CD08C496C98CA3`；新 EXE、正式包、SDK 全文件比较结果保留在 `fingerprints06-review.log`，出口日志为 `sdk06-review-export.log`。这次只核对未变化的实际内嵌内容，没有重复 SDK 六项/网站十七项测试。

最终 Scroll 包就绪后，针对改变的消费/生命周期接缝再次执行 T29 timer 两项 **2/0，79.13 秒**（`timer-callback06-final-green.log`），共享 private depth 一项 **1/0，28.85 秒**（`private-depth06-final-green.log`），正式 0.1.1 → 合成候选 0.1.2 的失败升级一项 **1/0，52.00 秒**（`update06-review-green.log`）。后一项保持旧数据/OS Token、旧 handles 失效、候选 producer 回零及不重放 HTTP，未因正式包升版让测试退化为相同版本重装。P1 专用夹具的 165.02 秒绿色之后没有再修改对应密钥/Manager 代码，复用该结果，不重复真实系统键四种失败组合。

实际追加命令如下，所有 Rust 命令仍使用 `../target06`，本节提及的 `--ignored` 均明确执行；没有把普通门禁中的跳过算作 WASM 通过：

```powershell
& ../target06/debug/editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/timer-callback --output target/cloud-api-test
& ../target06/debug/editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/secret-candidate --output target/cloud-api-test
& ../target06/debug/editor-app.exe --plugin-package plugins/cloud-translation plugins/code-host-query --output target/cloud-api-test
cargo test -p plugin-runtime --test cloud t29_timer -- --ignored --nocapture --test-threads=1
cargo test -p plugin-runtime --test cloud t29_private_area_depth_overflow_is_rejected_before_poisoning_shared_storage -- --ignored --nocapture --test-threads=1
cargo test -p plugin-runtime --test cloud t27_t30_unpublished_snapshot_and_publish_failure_preserve_system_token -- --ignored --nocapture --test-threads=1
cargo test -p plugin-runtime --test cloud t25_response_limit_and_invalid_utf8_preserve_full_display_cache_and_cancel -- --ignored --nocapture --test-threads=1
cargo test -p plugin-runtime --test cloud t30_failed_cloud_update_retires_candidate_and_recovers_data_without_replaying_http -- --ignored --nocapture --test-threads=1
& ../target06/debug/editor-app.exe --export-plugin-sdk ../target06/sdk06-review
```

## Temporary 归属续修与纯源码 SDK（2026-10-11）

`d92f79c` 的复审确认四项旧发现均已解决，同时新增一项 P2：`update_setting` 在旧实例私有根上准备配置候选，失败候选的 stop/Drop 无条件删除共享 `.temporary`，使仍在运行的旧实例丢失文件。此次固定复审基线为 `d92f79c8957063d1f77a5600f4480baed50859e0`，不将旧候选的绿色评审当成续修最终候选已通过。

根代理对 `d92f79c8957063d1f77a5600f4480baed50859e0` → `a248ccfb1ce9281a989f4888f469a0a25bf47c87` 完成独立双轴复审：

| 审查轴 | P1 | P2 | P3 | 结论 |
| --- | --- | --- | --- | --- |
| Spec | 0 | 0 | 0 | Temporary 归属、发布、权限与纯源码 SDK 行为符合规格，无新增行为发现 |
| Standards | 0 | 0 | 1 | `private.rs` 注释误写空 staged_writes 证明未提交，未准确区分 flush 与 committed activation |

唯一 P3 已仅修正为“空 staged_writes 不能证明 activation 已提交”；实际守卫仍是 `activation_committed`，没有修改代码行为、协议源码、SDK 内容或包资源。注释与本记录通过 `git diff --check`，复用上一候选对应行为和 SDK 证据，不重复运行 ignored、SDK 或完整 Rust 套件。根代理安排独立 Standards 在 `54272a6261a3cd7c9b726aa6578d3a4392d94c47` 读回，硬项 0 / smell 0，原 Spec 0/0/0；该注释项已归零。此阶段未 merge、push 或关闭议题；随后仅收到已验证固定点才开始下述普通主分支整合。

修复复用实例现有 roots 与 committed activation，未新增宿主测试专用 API、独立权限层或 UUID 存储目录：

- 公开私有目录操作需可信 Manager 在 durable publication 后授予的 committed activation；Prepare、Validate、Apply、Activate、flush 后的 unpublished Snapshot 都不能获得 Temporary。
- 只有成功 OpenTemporary 的实例记录目录归属，关闭 handle 仍保留清理责任；quiesce、stop、Drop 至多领取一次清理权。失败候选没有归属，不会删除旧实例目录。
- 配置持久写入成功后，Manager 先移除并 stop 旧实例，再开放候选 committed activation 并插入。Manager 串行调用，Broker 只持有调用元数据及队列、不持有 WASM Store；没有两个可执行 owner 同时获得同一临时目录的窗口。
- Data 的文件、目录、编辑器入口不能访问保留 `.temporary`，同时拒绝 Windows 大小写、尾点、尾空格及现存 8.3 别名。校验只作用于 Data，Cache/Workspace 内同名相对路径仍可用；editor 原生路径解析在协商与 grant 检查之后执行。
- Data 公共根深度仍为 16，Cache/Temporary 各占一个保留层，没有增加额外 UUID 层。预览 data refresh 可复制旧 Temporary；最终 commit 先 quiesce/snapshot/stop 旧实例，再 refresh 清除预览副本，最后 publish 才 rename backup。恢复公共 Data List 在重新 OpenTemporary **之前**断言没有 `.temporary`，避免重新打开掩盖孤立数据。

新增独立 `temporary-candidate` 0.1.0 由正式 SDK 构建，Prepare/configuration 及 Snapshot 的实际 reply 经生产 `Package` 和 `Manager` 验证。以下失败日志保持原样，准备夹具失败另保留 `temp-publication06-fixture-red.log`，不与行为红灯混记。

| 接缝 | 真实红灯 | 首次绿色与断言 |
| --- | --- | --- |
| 失败配置候选与 owner 清理 | `temp-owner06-red.log`，0/1、46.28 秒；旧 handle 读回 NotFound | `temp-owner06-green.log`，1/0、182.66 秒；Prepare/Validate/Apply 失败均保留旧文件、读写和资源数；成功替换旧 handle 无效、旧文件消失；CloseResource 后 disable 仍清理，restart、host drop/reopen 均无旧临时文件 |
| unpublished Snapshot / 发布取消 | `temp-publication06-red.log`，0/1、43.84 秒；候选实际得到 `temporary=allowed` | `temp-publication06-green.log`，1/0、109.13 秒；候选 Snapshot 返回 InvalidState，Snapshot 失败与 Committing 取消后旧 handle 无效、Data/Cache 保留；恢复 List 不含 `.temporary`，再打开的 Temporary 无旧文件 |
| Data 旁路与其他 root | `temp-data-boundary06-red.log`，0/1、21.89 秒；Data CreateDirectory 创建了未归属 Temporary | `temp-data-boundary06-green.log`，1/0、25.73 秒；Read/Write/Stat/List/Create/Rename 两端/Delete/OpenDataFile 九种入口拒绝保留名及别名，旧 owned/persistent bytes 保持原值；Cache/Workspace 同名目录正常 |
| 编辑器入口权限顺序 | `temp-editor-authority06-red.log`，0/1、18.79 秒；未授 storage 先返回 InvalidPath | `temp-editor-authority06-green.log`，1/0、49.34 秒；实际两个正式 guest 的无 consent / 未协商调用，对保留名和缺失文件分别先返回 PermissionDenied / CapabilityUnavailable，资源回零 |
| SDK 资源出口 | `sdk-source-only06-red.log`，0/1、0.09 秒；公开 export 仍含 CLOUD.md | `sdk-source-only06-green.log`，3/0、1.14 秒；独立 export 仅源文件，保留完整 Rust/WIT/JSON 契约；并发 publication、缓存复用和损坏修复均通过 |

新的 `sdk_export` 移除 22 项 Markdown 的嵌入、frontmatter/站点 route 改写和 reader 专用测试，保留 41 项 `crates/plugin-protocol/` Rust/Cargo/WIT/JSON 原始字节。四个公开模块的源码注释已说明最低协议与能力版本、required/optional 缺失处理、权限、用法、错误、限额、取消与释放；临时状态归属说明位于 `storage.rs`。稳定版本的通用兼容/弃用政策由根代理在公共 `lib.rs` 整合，本工作树未独立合并尚未验证的 main。

实际新宿主导出 `../target06/sdk06-source-final`，**41 个文件、0 项非源码资源、与内容寻址缓存 0 hash 差异**（`sdk06-source-final-compare.log`）。新 SDK key 为 `2d178260e227669265b7773709805e2c02ef50878f0b92a0415e7c13997460f4`，仍是 SDK crate 0.2.0 / wire 7 / 既有稳定能力版本。正式 `--plugin-package` 为两个 0.1.1 消费者和 Temporary/secret 两个 0.1.0 夹具构建实际 WASM，release 分别 **21.00 / 19.59 / 18.63 / 16.90 秒**（`package-cloud06-source-final.log`），每个 compiler-artifact 均指向该新 SDK；各 build key 仍为 `d392df44fcad6573`、`b37b624e1acd5ac3`、`7ae39388857952ad`、`8f26becd04ca5d95`。

新 SDK 源与缓存路径改变实际 WASM 身份，即使两个正式 guest 的业务代码与 UI 未改，也不将 d92 的原生记录改记为新 WASM hash 通过。完整 HTTP/TLS、Token 与 UI 主矩阵继续复用对应阶段证据；续修只复验被改变的 Temporary、配置/发布、权限顺序和独立 SDK 出口。

使用 **2d178 SDK 的实际新包**复验 Temporary 的归属、Data 旁路和 Snapshot/发布失败三项，**3/0、372.66 秒**（`temp-owner-boundary06-source-final.log`）；另行完成权限顺序一项 **1/0、49.34 秒**，最后三个场景执行时通过 `--skip` 排除已绿色且未变化的权限顺序场景，四个不同场景均有实际绿色结果。Windows fixture 实际取得并验证 `TEMPOR~1`，没有将可选短名分支跳过记作覆盖。

配置 publication 调整同时影响已有密钥授权切换，故仅复验相关的实际 SDK `secret-candidate` 一项，**1/0、205.35 秒**（`secret-publication06-source-final.log`）。候选 Snapshot 的 Write/Delete 与 Snapshot 失败/Committing 取消四种组合，原 OS 键仍保持原值；成功配置、恢复后 Write/Delete、disable/enable 与重新打开均可用。测试采用隔离 UUID profile、真实 Windows Generic Credentials、假 bytes 与精确单键 RAII 清理，没有枚举用户凭据。

续修最终自动门禁和受影响应用测试：

- `cargo fmt --check` 退出 0（`fmt06-temporary-delivery.log`）。新 fixture 的独立 fmt 检查也退出 0（`fmt-temporary-fixture06-final.log`），之后只补 README 构建说明，没有修改 guest Rust。
- 默认并行 `cargo test --workspace --exclude editor-app` 在 runtime lib 为 **73 通过、1 失败**（`workspace-tests06-temporary-delivery.log`，编译 2 分 27 秒）。未修改的 `owned_http_socket_returns_status_chunks_and_finished` 在原 1,000 ms deadline 返回 TimedOut，worker 为 1、受控 fixture 尚无 capture；没有把未执行完的矩阵记为全绿。
- 不改源码、生产 deadline 或测试参数，隔离同一场景 **1/0、0.53 秒**（`owned-http06-temporary-delivery-isolated.log`）。随后完整非 UI workspace 使用 `-- --test-threads=1` 得到 **233 通过、0 失败、217 ignored、64 个结果 batch**（`workspace-tests06-temporary-delivery-serial.log` / `workspace-counts06-temporary-delivery.log`）。相同代码隔离与串行通过，调度负载是可能解释，尚未证明该次并行超时的确切根因；这项限制随真实失败记录保留。
- `cargo check --workspace` 退出 0、3.79 秒（`workspace-check06-temporary-delivery.log`）；最终宿主 build 退出 0、23.20 秒（`build-host06-temporary-delivery.log`）。既有 unused/dead-code 与 LNK4217 warning 未消除，也未报告成无警告。
- `cargo test -p editor-app sdk_export::tests -- --test-threads=1` **3/0、0.22 秒**（`sdk-source-only06-delivery.log`，编译 35.38 秒）。源码 Rustdoc 构建退出 0、2.67 秒（`protocol-doc06-source.log`）；仅保留未修改 `ui/visual_viewport.rs` 的两条 `[0,1]` 链接 warning。本轮没有新增可见 UI 控件或修改两个 guest 的业务/UI，原生证据仍对应前节实际 95C21 / d92 包。

最终 B3E2 宿主再次导出 `../target06/sdk06-temporary-delivery`：41 项资源、0 非源码文件、与 **2d178 SDK** 缓存 0 hash 差异。最后的 README 构建说明作为 fixture 资源再次正式打包（`package-temporary06-delivery.log`）；Cargo release **0.19 秒**，全部 artifact fresh，ZIP 内 WASM 与执行上述回归的 `991301…` 完全一致，因此没有重复执行相同 guest 字节的测试。正式两个消费者和 secret 夹具的业务源码/包资源未再改动。

最终指纹由 `Get-FileHash` 与直接读取 ZIP 内 WASM 的 SHA-256 得到，保存为 `fingerprints06-temporary-delivery.log`；先前 source 包指纹保存在 `fingerprints06-source-packages.log`，不会用最新值覆盖历史阶段。

| 续修最终产物 | SHA-256 |
| --- | --- |
| `../target06/debug/editor-app.exe` | `B3E2882B19E7F6047E44DCF649F5BF28DAC1F39041B8753EC10D64192342EA9A` |
| `cloud-translation-0.1.1.zip` | `AD8BEE67205EA5E11A6208BD9AADBCBB75D43BE5659F19D58EC6CBC0ABC18E33` |
| ZIP 内 `cloud-translation.wasm` | `81C2D09579C51A2A7070A247BA9AC5B2C3A356A65464C4B53914CEC7F13A7754` |
| `code-host-query-0.1.1.zip` | `A08EE666C2E4DB059F673E4EB97CC34BD97DA8FFBDBEB8D548F96B8BEDDC7975` |
| ZIP 内 `code-host-query.wasm` | `9A6F3F7C1A505EF249E384FA1F1050DD4C4391FDECE05F0519A535B6FD8F1AA3` |
| `temporary-candidate-0.1.0.zip` | `262313FEB1769E70BF2427BF1EE408CD158B301F523E4402DC99DA21249E123A` |
| ZIP 内 `temporary-candidate.wasm` | `9913015DE1A41DB1939D875994F12F94ABDE9BDE3C1472AB4926E7DF5C09F613` |
| `secret-candidate-0.1.0.zip` | `D74403FC1AAADB160AECE38A9C4284F233C5746DCE63464C76DC276BC4E35C6B` |
| ZIP 内 `secret-candidate.wasm` | `BB6906A7A13853A7785D98C4CC219D768782AD3BD007FC6BBF72C7A960847FA7` |

实际续修命令，仍在唯一工作树使用 `../target06`；不安装新工具、不调用历史 scripts，也不修改全局 VS 环境：

```powershell
cargo test -p plugin-runtime --test cloud t29_private_editor_navigation_checks_authority_before_path_lookup -- --ignored --nocapture --test-threads=1
cargo test -p plugin-runtime --test cloud temporary_ownership -- --ignored --nocapture --test-threads=1 --skip t29_private_editor_navigation_checks_authority_before_path_lookup
cargo test -p plugin-runtime --test cloud t27_t30_unpublished_snapshot_and_publish_failure_preserve_system_token -- --ignored --nocapture --test-threads=1
cargo test -p editor-app sdk_export::tests -- --test-threads=1
cargo fmt --check
cargo test --workspace --exclude editor-app
cargo test -p plugin-runtime --lib owned_http_socket_returns_status_chunks_and_finished -- --nocapture --test-threads=1
cargo test --workspace --exclude editor-app -- --test-threads=1
cargo check --workspace
cargo build -p editor-app
& ../target06/debug/editor-app.exe --export-plugin-sdk ../target06/sdk06-temporary-delivery
& ../target06/debug/editor-app.exe --plugin-package crates/plugin-runtime/tests/fixtures/temporary-candidate --output target/cloud-api-test
```

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

## 前两阶段门禁与历史网站输出

首候选的 Rust 三门禁均通过。`cargo fmt --check` 退出 0（`fmt06.log`）；非 UI workspace **233 通过、0 失败、209 ignored**，包含 doc-test 的 64 个结果 batch（`workspace-tests06.log`）；`cargo check --workspace` 退出 0，53.12 秒（`workspace-check06.log`）。编译输出保留已有 unused/dead-code 与 Windows 链接 warning，未把它们改报成无警告。普通 workspace 的 ignored 不算实际 WASM 通过；本单十项另以 `--ignored` 明确执行。

修复候选最终三 Rust 门禁也通过：`cargo fmt --check` 退出 0（`fmt06-review.log`），非 UI workspace **233 通过、0 失败、213 ignored，64 个结果 batch**（`workspace-tests06-review.log` / `workspace-counts06-review.log`；编译 23.32 秒），`cargo check --workspace` 退出 0、26.53 秒（`workspace-check06-review.log`）。额外四项 ignored 是本轮新增的实际 SDK timer/private-depth/secret-publication/display 回归，均已在上节明确执行得到绿色。两个正式 guest 与两个新夹具的独立 `cargo fmt --manifest-path <Cargo.toml> --check` 分别退出 0，日志为 `fmt-cloud-translation06-review.log`、`fmt-code-host-query06-review.log`、`fmt-timer-fixture06-review.log`、`fmt-secret-fixture06-review.log`。

本轮全量门禁先在 examples 链接时遇到 `LNK1104 msvcrt.lib`，测试尚未开始（`workspace-tests06-review-env-red.log`）。已安装库存在；当前 PowerShell 导入 VS18 的 `Microsoft.VisualStudio.DevShell.dll` 后以 `-Arch/-HostArch` 初始化，仍在 examples 遇到同样错误（`workspace-tests06-review-vs-arch-red.log`）。仅在随后构建进程改用以下公开 cmdlet 参数，实际 test/check 得到上面的绿色；库路径读数保留在 `vs-libraries06-review-devcmd.log`。这是环境初始化差异，没有修改源码、安装工具或改变全局环境。

```powershell
Import-Module 'C:/Program Files/Microsoft Visual Studio/18/Community/Common7/Tools/Microsoft.VisualStudio.DevShell.dll'
Enter-VsDevShell -VsInstallPath 'C:/Program Files/Microsoft Visual Studio/18/Community' -DevCmdArguments '-arch=x64 -host_arch=x64' -SkipAutomaticLocation | Out-Null
```

首候选已执行针对性检查：协议 cloud **2/0**（`protocol-cloud06-final.log`），cloud capability admission **1/0**（`admission-cloud06-final.log`），应用 SDK 导出 **6/0，0.35 秒**（`sdk-export06.log`）。一般 HTTP/密钥/目录/timer 全部包含在导出的 SDK；首候选最后的通用文本 accessibility 和测试 UUID 调整没有改变当时 SDK key、正式 ZIP 或 guest WASM；本轮两个 0.1.1 包和 hash 另见上表。

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

2026-10-09 的历史交付曾在 `website/src/content/docs/en/sdk/cloud.md` 和中文对应页维护读者正文，并导出英文源为 `CLOUD.md`。2026-10-11 用户更新的 AGENTS 明确替代这个出口：公开使用方法写在源码注释，独立 SDK 不内嵌或导出 Markdown。本次将 HTTP、密钥、私有区域与 timer 说明迁到源码，不继续修改网站正文；仓库 `docs/` 保留方案、工单和执行证据。

已有 Node/npm 执行 `npm ci` 恢复锁定项目依赖，通过；`npm test` 初次 **15 通过、2 个 build/index 前置跳过、0 失败**；随后 `npm run build` 真实构建和搜索索引完成，测试 **17 通过、0 跳过、0 失败**。对应 `website-install06.log`、`website-test06.log`、`website-build06.log`。未使用初次跳过冒充搜索通过。

## 当前主分支与 01/02 普通合并连接（2026-10-11）

普通合并的两父基线为 `54272a6261a3cd7c9b726aa6578d3a4392d94c47` 与根代理已验证的 `de5397645d05c5d91e8cd74108d46ad9d7a88ff3`。不改写历史；保留原生终端主线及 01/02 的文档、比较、菜单和交互。冲突逐段合并，SDK 资源为两侧并集：42 项 Rust/Cargo/WIT/JSON，源码包含当前文档、配置、运行目标、调试与本单四个模块。公开 `lib.rs` 兼容/弃用政策保持根源码注释；SDK 不恢复 Markdown。原双语 cloud 读者稿按主分支目录迁移保留在 `documentation/`，仅修迁移入口，不修改正文或恢复站点构建。

自动合并后的实例初始化与退休同时保留 HTTP、timer、document stream、virtual document 和原生终端资源。`Manager::open_with_resources` 保留 canonical profile secret namespace 及主分支的受管私有数据锁；配置仍先 durable write、再移除并 stop 旧实例、最后 commit_activation 并插入新实例。`data_transaction.rs`、private owner 与 credential mutation 行为未变化；编辑器新增文档操作没有改变 OpenDataFile 的协商/grant 后 reserved-path 校验。

本阶段先 `cargo fmt --check` 退出 0（`fmt06-main-integration.log`），最后源码注释补齐后格式再次退出 0（`fmt06-main-integration-final.log`）；`cargo check --workspace` 首次退出 0、35.10 秒（`check06-main-integration.log`），源码注释补齐后最终检查退出 0、8.01 秒（`check06-main-integration-final.log`）。新宿主 `cargo build -p editor-app` 退出 0、4 分 26 秒（`build-host06-main-integration.log`），仍直接使用当前 Cargo/VS 工具，未安装工具或调用历史脚本。

实际 fresh export 为 `../target06/sdk06-main-integration`：**42 文件、0 非源码资源、与当前 protocol 源和实际 compiler SDK 缓存分别 0 hash 差异**。SDK key 为 `aa89f9a713a00439bf79c329b9db51758e24d9a1a6b3417f15df963da3208f76`；四个 release compiler-artifact 均引用该 key（`export-sdk06-main-integration.log`、`packages06-main-integration.log`、`fingerprints06-main-integration.log`）。正式 0.1.1 translation/query 与 0.1.0 secret/Temporary 夹具分别构建 **21.78 / 21.00 / 20.05 / 20.68 秒**，原 build key 及包版本保持不变，此阶段未修改消费者业务。新增文档类型及其源码字节改变 WASM 身份；当前 Native 短连接必须核对下表，不能复用旧 hash 作为新产物已验收。

| 当前主分支整合产物 | SHA-256 |
| --- | --- |
| `../target06/debug/editor-app.exe` | `4B220DC26957AFE0E181D31187A4F82FF452C7AF495AEF255F81D63CC4003131` |
| translation 0.1.1 ZIP | `C1D6D01DD5D725F1686E71BEBFFCE8F1CDBD8CFFB8CAED64665314D096A0C67C` |
| ZIP 内 translation WASM，2,459,516 bytes | `D287CB6FB0C5312301121E07C28C7D8959E67367FB639EC7CE3B95D2E7EE674E` |
| query 0.1.1 ZIP | `8E245135A2EC3745B8D5195E3C4F48378DC0746B3156D91B6FEB7E438776A6C4` |
| ZIP 内 query WASM，2,455,832 bytes | `45FE738FDAF9A9EF8A5356F86C0397EC7713F632EEB750C949C4E7EBBA9850CD` |
| secret-candidate 0.1.0 ZIP | `30CEEFF6614A3328CFEE13BF94991CAC82F64558DFEEE53BE5CF1025E6D77CBA` |
| ZIP 内 secret WASM，2,380,393 bytes | `53EEECEE94B4D64FBD5EE449473248E6218015E178476F873E9E8AF0CC119836` |
| temporary-candidate 0.1.0 ZIP | `3A1CE3175FF705DCBA1B680F992FC48A10AC0F78E947894AAFA1C75211BEA92E` |
| ZIP 内 Temporary WASM，2,394,916 bytes | `8FD1F60218112276EB4708A73AB9AB12632E1AD071A3062D6E3AB5E43E5BD387` |

当前 SDK 出口、并发 publication 和缓存修复测试 **3/0、0.12 秒**（`sdk-tests06-main-integration.log`，编译 2 分 14 秒）。实际新 Temporary 包的一条 owner 回归 **1/0、169.65 秒**（`temp-owner06-main-integration.log`，编译 1 分 14 秒），同时验证受管根初始化、Prepare/Validate/Apply 失败候选保留旧文件、正常配置 old.stop→new.commit、CloseResource 后 disable 清理及 host drop/reopen。密钥发布四失败组合的一条新 SDK 回归 **1/0、183.99 秒**（`secret-publication06-main-integration.log`，编译 8.94 秒），覆盖 Snapshot write/delete 失败与 Committing write/delete 取消，保持原系统 Token、退役旧 handle、恢复 Data/Cache 和资源回落。默认并发非 UI 门禁 **247 通过、0 失败、221 ignored，66 个结果 batch**（`workspace-tests06-main-integration.log`；编译 2 分 49 秒），原 HTTP 单元测试也通过；本次没有改其超时或断言，也没有用串行执行代替默认结果。旧阶段的默认 HTTP 时间波动失败和后续串行结果继续保留，未重报为本阶段结果。未重复未变化的完整 Temporary、HTTP/TLS、timer 或 UI 主矩阵；这里只复用它们对应旧阶段的完整证据，不把 ignored 跳过计为通过。

本阶段在同一已初始化的 VS 环境中直接执行以下入口，包构建使用宿主默认 release 与 SDK 缓存，不启动桌面：

```powershell
cargo fmt --check
cargo check --workspace
cargo build -p editor-app
& ../target06/debug/editor-app.exe --plugin-package plugins/cloud-translation plugins/code-host-query crates/plugin-runtime/tests/fixtures/secret-candidate crates/plugin-runtime/tests/fixtures/temporary-candidate --output target/cloud-api-test
cargo test -p editor-app sdk_export::tests -- --nocapture
cargo test -p plugin-runtime --test cloud t29_t30_temporary_owner_survives_rejected_configuration_and_cleans_on_retirement -- --ignored --nocapture --test-threads=1
cargo test -p plugin-runtime --test cloud t27_t30_unpublished_snapshot_and_publish_failure_preserve_system_token -- --ignored --nocapture --test-threads=1
cargo test --workspace --exclude editor-app
```

固定普通合并候选 `f2a907a6a6348aedd3b2f1a37681c4fda1f8da7c` 的 Standards 复审指出一项 P3 判断性 `Duplicated Code`：`plugins/cloud-translation/src/lib.rs:299` 和 `plugins/code-host-query/src/lib.rs:297` 各自显式处理 Headers/Chunk/Finished/Failed 流步骤。根代理接受本轮保留：两个包是独立构建、可复制的公开消费者，公开 SDK 已提供通用传输能力；目前不为两个示例增加新的 SDK 抽象，真正形成稳定复用后再提取。此项没有行为违反，也没有通过改源消除重复；记录为已接受的复用权衡。

本次补记只修改验收 Markdown；相对上述固定候选的 `crates/` 与 `plugins/` 差异为零，宿主、SDK 与包指纹及本阶段门禁证据保持对应关系。没有重建产物或重复行为测试。根代理已完成当前正式包的原生短连接，见[原生记录](native-06.md)的“2026-10-11”部分；最终推送与议题关闭按远端读回记录。

本执行代理未启动桌面、推送或关闭 issue。根代理完成候选固定点复审及当前正式包的必要原生短连接：新宿主上的云查询/缓存与分块结果、取消资源回落，并与其已验证的原生终端共存；Token、IME、两主题等未变完整 UI 主矩阵维持原证据。

## 原生连接验收与限制

根代理负责共享 Windows desktop 的顺序验收；本执行代理没有操作桌面。首候选的实际记录见[原生连接证据](native-06.md)：首轮云翻译密码遮罩/系统凭据/POST 分块/缓存/进度取消与真实 FIN/503/退出及精确凭据清理通过；86D4 宿主通用 Text 的真实 UIA Name、第二消费者 GET 两块结果和离线安全失败随后通过。所有监听器与宿主控制器正常 stop、退出 0，自有进程为空；两个明确测试键精确 API 清理读回 missing，普通 profile/控制器日志扫描无假 Token，工作区为空。修复候选的大结果原生滚动复核及固定点双轴复审另由根代理完成，不把旧 EXE 的首候选结果记成新 hash 原生通过。Token 输入、进度、IME 与控件完整矩阵引用 [02 的验收记录](02-commands-and-interaction.md)，本单仅补连接与可见结果，不重复 UI 主矩阵。

根代理已追加同一原生记录的“审查修复后”部分：**95C21 宿主 + code-host-query 0.1.1** 实际 GET 完成 65,536 bytes，滚动到末尾可见完整末尾标记，稳定 Windows UIA 三个 Text Name 包含全部首尾；正式 Read cache 后两个 Text Name 保留原始正文，菜单仍可用。实际 dev candidate `74847847fe64410687e1a09090f97d79f6283946519179fd5383bbd0f1a4ea21` 的 WASM 与正式 ZIP 内 `DF6344…` 相同；缓存严格 UTF-8、65,536 bytes，全部 bytes SHA-256 `8955d95861d723df83fcaf8be5a28f8f89e041133ef1d6895fc9e3425f19f0c3` 等于独立重建夹具。控制器/监听器正常 stop、退出 0，自有 EXE 进程和工作区条目均为 0，两个精确测试 OS 键均 missing，普通 profile/日志无假 Token。共享桌面工具的过期目标和焦点干扰按记录重新选择精确自有 EXE 后验证，未操作其他实例。这里只把新滚动接缝记为新 hash 原生通过，未重复 Token/POST/权限主矩阵；`d92f79c` 双轴复审已完成，新增 Temporary P2 及续修候选固定点复审状态见上节。

两个包正常命令菜单含 set-token/query/refresh/cancel/read-cache/delete-token，测试端点分别 `http://127.0.0.1:18906/translate`（POST）和 `http://127.0.0.1:18907/repositories`（GET）。假的 ASCII Token 通过密码输入，不能直接写普通设置。原生隔离运行：

```powershell
& ../target06/debug/editor-app.exe --plugin-dev plugins/cloud-translation --release --profile <QA-profile> --workspace <empty-QA-workspace> --grant assets.read --grant network.http --grant network.http:http://127.0.0.1:18906 --grant network.http:http://127.0.0.1:18907 --grant secrets --grant storage --grant ui.interaction
```

`Manager` 根为 `<QA-profile>/runtime`。系统凭据目标为 `Nanobug/plugin/{SHA256(canonical-runtime-root UTF-8)}/{SHA256(plugin-id UTF-8)}/{SHA256(key UTF-8)}`，均小写 hex，key 为 `api-token`。路径哈希来自成功的 Rust canonicalize，Windows 保留其 extended prefix；原拼写保持 Manager.root 公开语义。`HostResources.secret_namespace` 只有可信宿主配置可覆写，插件/普通 settings 不可指定。原生辅助只可精确读取/清理自有隔离 QA 根的这两个键，不使用枚举或 GUI 删除确认。

OS DNS 是同步调用：消费者能立即收到 Cancelled/TimedOut，但进行中的 OS DNS 可能占用四个有界 worker 中一个，直至 OS 返回；前后与 connect 接缝检查撤销，禁止迟到连接，未创建无界 DNS helper。不能宣称 OS DNS 本身即时终止。Windows 以外的系统密钥接口明确返回 UnsupportedOperation，本次没有其他平台原生交叉验收。外部 HTTP 副作用不可回滚，恢复只还原受管私有数据和逻辑状态。未扩展 OAuth、AI、WebSocket 或后台任务系统。

本单实现代理不推送、不关闭 #98；根代理完成真实原生验收、固定候选双轴审查、推送后读回关闭。父设计 #92 保持开放。
