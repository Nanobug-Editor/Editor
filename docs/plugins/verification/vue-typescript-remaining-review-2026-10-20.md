# Vue / TypeScript 剩余场景双轴审查

> 最新：下列原始审查和暂停记录保留原时点。恢复后的 Worker、悬浮、配色、键盘与最终支持元数据增量均已完成独立双轴复核，无未解决的可行动发现；最终结果见本页末节及[交付记录](vue-typescript-remaining-delivery-2026-10-20.md)。

日期：2026-10-20，Asia/Shanghai。按 `code-review` 的两个独立代理分别审查，均未参与被审实现；只读审查不替代实际测试。新 #122–#126 仍在联合验收，旧工单与父设计议题不变。

## 比较点与候选

固定点沿用已交付 #121 的三仓基线：Editor `fc9dc531ef991315d73e21c8a092f12a0f1ff78b`、TypeScript `873e8c3d3ebe3a86a8bb1e028a3c4bcd1c557b94`、Vue `f8e5880bf89161f16723ae7be1c76458d50a676c`。

三仓 HEAD 此时仍等于对应固定点，`git log <base>..HEAD --oneline` 为空，没有把未提交实现伪称已提交。审查实际工作候选使用 `git diff <base> --`，并以 `git ls-files --others --exclude-standard` 纳入每个相关新文件；仅运行 `git diff <base>...HEAD` 会漏掉全部本轮实现。

原始候选路径与逐文件 SHA 清单在宿主 `target/s6-review-inputs-2026-10-20.json`，分别 161 / 38 / 26 个文件；清单 SHA-256 `c0551a646f33a96afe007fe50f32a1d1dcac7b4ff61020f953739de3db76a68f`。Standards 代理已独立读回全部原始 SHA 匹配。随后只对本轮文件应用机械格式化，两个代理均知情并以当前源码行号定位；后续实质修复须另做增量审查。

## Standards

硬性规范违规：未发现确认项。判断性坏味道：无值得单列的可行动项。

已检查宿主上下文授权、revision/owner/connection 失效、跨文档事务、补全 resolve、格式化、恢复及 SDK 导出；TypeScript 项目候选、选择与私有偏好；Vue 外部脚本视图与项目隔离；新增测试、夹具入口和验证文档。大型测试体重点核对权限、时序和清理。

未发现新增宿主语言白名单、插件专属 API、第二份可写文档/Undo 或明显授权扩大。未找到更细的 AGENTS、CONTRIBUTING 或 CODING_STANDARDS。机械格式化和已公开 pending 的运行结果未作为发现。

限制：未构建、运行测试或启动窗口；最终原生行为、实际包与 SDK 一致性由联合验收证明。

## Spec

**P1：内层 TS/Vue 后端退出后无法进入有限恢复。** [S5-AC4](../tickets/vue-typescript/scenarios/05-recovery.md) 和[多后端规格](../specs/vue-multi-backend.md)要求受影响兼容组合恢复。TypeScript `native/engine.mjs` 的子进程退出仅设置 `closed` 并拒绝 pending；外层 `native/transport.mjs` 捕获错误后保留 stdio。宿主 `language/navigation.rs` 将远端错误转 typed Failure，而 `navigation/recovery.rs` 为保护合法业务错误直接忽略这类错误，因此不会产生连接故障代次或重新准备适配器，后续仍指向已关闭 Engine。

既有 `native_vue_recovery_tests/scenario.rs` 终止的是外层受管服务，未覆盖仅 tsserver/adapter 子进程退出。建议由插件把内部致命故障传播为所属受管服务退休，并补实际内层故障、B 项目保持、有限恢复及旧引用失效测试；不能把所有 typed 业务错误当作崩溃。

该发现来自只读代码路径推导，审查者未动态复现。根代理已接纳，正在补真实 RED→修复→GREEN，**尚未标为解决**。未发现明确范围蔓延。已读上下文注册/选择、补全 resolve、rename/preview/Undo、映射贡献及恢复核心路径。

本轮总计：Standards 0 项；Spec 1 项，最高 P1。此结论不是最终交付门禁通过声明。

## 增量审查（01:12）

Spec 代理已独立复核 `.13` 的已准入 Engine 致命退出传播和候选失败区分，代码路径已修正，尚待真实 Windows 内层故障复验。必要服务只读 preflight 放在旧实例退休之前且 cutover 再检查，未见新增规格缺陷；反向 TS provider major 变更另补实际包测试。

Standards 代理复核此生产增量，未发现确认的生产规范缺陷；并独立核对 `.13` native archive 的 16 个文件与源码逐 SHA 一致。另有两项测试防护 P2，实施中，未标为解决：

- **硬性资源清理**：`native_vue_recovery_tests/scenario/inner.rs::powershell` 的 `Command::output()` 无自身超时，外层 `h.job` 停止等待不能终止 blocking helper；应持有 Child，在期限内退出或 kill/wait，避免迟到故障注入。
- **判断性安全**：TS `test/inner-exit.test.mjs` 枚举后只保留 PID 并直接终止；应保存创建身份、PPID 和命令，在终止前再次核对归属，避免 PID 复用误伤。

审查者没有构建或运行测试；Node / Windows 证据由实施验收另行记录。没有把尚未执行的结果记为通过。

### 两项 P2 修复复核

实施后由原 Standards 独立代理再次只读复核，**两项 P2 均在源码层解决，无新增发现**。Rust helper 保存 Child，以 10 秒限期和所有返回路径的 kill/wait 回收；输出保存在独立私有临时文件并检查长度，避免管道阻塞。终止叶进程前持有精确进程句柄，在同一受控 helper 中核对创建身份再 Kill/WaitForExit。

Node helper 通过 JSON stdin 传递身份，复核 owner/leaf 的 PPID、创建时间、命令和目标种类；新增错误创建身份拒绝后原服务仍返回真实类型的负例。两项实际 Node 用例 **PASS** 5.870s、0 skipped，`TypeScript/target/s5-inner-exit-helper-guards-final.log`。冻结文件 SHA-256：Rust `d5f7024c2a9ca7ae019524667739fcb0d48420571bdb750f60cb6b5c47771e2f`、Node `20bc7564b4a66bbff279a8c65134bb88d70ff4b32dae1449af185d0586f50b53`。此处不将源代码复核和 Node 通过冒称为实际 Windows GPUI 内层故障已通过。

## 后续实际结果与增量 Spec 复核

原 P1 现有真实 Windows **GREEN**：`target/s5-native-inner-adapter-014-second.log`，parent 122.54s / child 122.38s，宿主 `0b8e4ac4499afa05e134eedb9e85b7bc97c71806a4f25fd9d8217148d26e8c39`，TS `.14` / Vue `.11`。真实补全及具体源码诊断建立后，仅终止 A 已准入的 adapter 子进程；外层 stdio 自然 EOF、正常有限恢复、脏类型变化、旧结果撤销、B/C 原服务及 capture 保持、撤信任与 checked shutdown 均通过。原 Node RED 与修复后工具结果保留。此前 `.12/.13` 两次 Windows 运行只在初始 UI 投影就绪前置超时，并未触及故障或更新，不能当作该 P1 或反向更新的 RED。

S4 另发现真实 Node 模块加载问题：宿主规范 Windows 路径用于贡献模块时 `EISDIR lstat C:`。Spec 独立代理复核 `.14` 的 `engine-bootstrap.cjs`：primary、贡献和 `ts.sys.require.modulePath` 都采用各自 `verifyClosure` 已校验入口，闭包、摘要、归属与 loader 检查保留，无截断或权限扩大。新增实际 stdio 回归断言入口超过 260 字符，并检查贡献改变真实诊断、悬浮及磁盘保持；未伪造语义。增量未发现新问题或范围蔓延。完整 source / sealed 均 48/48、0 skipped；此工具结果不外推 S4 映射事务或反向 TS provider 更新已经通过。

独立代理也复核初始等待改为真实 native plan / active server 后，注入前真实 completion/diagnostic 及全部故障断言仍保留。反向兼容更新和最后增量审查继续按实际结果记录。

## 取消需求的增量 Spec 审查（01:53）

独立 Spec 代理只读复核 `manager/language/delegation.rs` 和公开 SDK 回归，未发现可行动问题。旧等待者非空且全部终止时才丢弃旧 Pending；有效等待者和刚准备但尚未加入等待者的 Pending 继续复用。没有按业务错误类型重启或替换服务，符合 S2-AC2、S5-AC6。

实际 RED 66.17s → GREEN 68.78s 及其边界见[独立取消回归](vue-typescript-deferred-control-cancel-2026-10-20.md)。该测试未启动原生进程，也不验证脏文档同步或 Snapshot 持久化；本次静态复审不扩大这些证明范围。

## 冻结增量 Standards 审查（02:13）

原独立 Standards 代理再次只读复核，未发现确认的硬性违规或可行动坏味道。覆盖 `.14` verified closure 路径与真实 stdio 回归（16/16 native 源码/归档 SHA 一致）、正向依赖预检、回滚 materialized 状态、取消 Pending、F12 来源，以及历史/格式化/生态/外部脚本测试。两个事务场景共用唯一收据 helper `d7a8…e776f4`，保留滚轮和实际指针执行 Undo。

显式 SDK 拒绝回归允许尚未发布 plan 的早期拒绝；若有 plan 仍要求唯一 TypeScript owner 和精确选定 SDK，phase 0、结构化错误、原文及退出断言保留。审查者没有构建或运行测试。尚在实施的反向依赖与 Manager/Actor 共同预检不纳入本轮结论，冻结后继续增量复审。

## Manager/Actor 预检增量审查（02:49）

Standards 独立复核无确认的硬性违规或可行动坏味道：有效契约提取复用平台/配置门控，反查当前解析、scope 和 mandatory contract，公共准入前缀保留授权、取消、替换同意与 dormant 限制，Actor 在预检拒绝后不撤销 epochs，commit 再校验。此为静态审查，未执行测试。

Spec 独立发现 **P1：纯资源同 ID 候选绕过反向校验**。`installation.rs` 的 resource-only 分支只检查原 data_format，随后 `install_declarative` 可撤销旧实例和全部出口；没有 data_format 的 TS 允许这种候选进入，活 Vue mandatory consumer 将丢失提供者。WASM major 更新受 `validate_prepared_installation` 保护，但此入口没有 Prepared token。该发现引用 S5-AC2，当前为静态推导，已接纳并补真实公开更新 RED；未标为解决。修复应对候选有效契约快照执行同一反查，不创建声明式 guest。其余已审 WASM 预检与 Actor 顺序未见新增规格缺陷。

## 上述 P1 与原生回执增量的后续结论（03:32）

纯资源候选已取得真实公开 Manager **RED 27.90s → GREEN 28.06s**，两次更早的夹具 quota 前置错误另存，不计目标 RED。WASM 和声明式候选现在生成同一种只读有效契约快照，共用当前 mandatory consumer 的反向校验；没有创建空 guest。Actor 在退休公开请求代次之前调用预检，提交时再次检查。原有反向边界回归 **GREEN 57.98s**，运行时普通 lib **104 PASS / 3 ignored、13.58s**，程序 `target/s5-resource-only-preflight-final/deps/plugin_runtime.exe` SHA-256 `788049cd007660c25668143b106b17e60e0ebb2ae2c300aea82365a14a925628`。原 Spec 与 Standards 代理独立读回十项源码摘要并复核，**该 P1 已解决，无新增可行动问题**。这组 lib 结果不代替仍在运行的真实 Vue/TypeScript 外层更新联合验收。

回执原生 Undo 曾实际点击到底层 Explorer。最小生产修复仅为回执容器使用 GPUI 公开 `.occlude()`，保持子按钮与既有事务政策；Spec 和 Standards 独立复核均无新问题。修复后的实际历史八行 **GREEN 144.24s**、外部脚本两种组合 **GREEN 104.93s**、生态四角色跨文件事务及替换/卸载 **GREEN 152.95s**，三组均使用宿主 `703c8ba44d5b6927ef8b5a748e734ffe8b8f99d5d7ced2a2f79bc5a516d4c04a`、TS `.14`、Vue `.11`。详见[单一原生验收记录](vue-typescript-s3-s4-native-acceptance-2026-10-20.md)。

新增生态夹具确实产生 SFC 与 `state.ts` 的两文件计划，要求三个原始引用位置、预览取消、完整应用和回执分组 Undo，未为测试改变单文件直接应用策略。原四冲突恢复仍有失败证据；最新仅补测试对正常贡献同步边界的驱动，待完整重跑，不据静态复核提前标记通过。

当前生产源码的强制门禁已实际通过：`cargo fmt --check`；`cargo test --workspace --exclude editor-app -j1` 共 **319 PASS / 0 FAIL / 327 ignored**；`cargo check --workspace -j1`。ignored 未计入通过。正常宿主 `target/s6-final-native-host/editor-app.exe` SHA-256 `ff92755696cb68a3b00aac8111077b90908f0b50019beaeb6bdf56b8545c2776` 已分别通过两插件公开 SDK check，导出仍为 SDK `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`。最终包元数据与尚未完成的行为门禁仍需继续审查、验收后才交付。

## 插件元数据与失败缓存首版复核（03:49）

两独立轴均读回元数据 24/24 文件、保留资源 47 项及 Vue 十一项封印文件，摘要匹配。TS 插件 `.15` 保留 native `.14`；Vue 插件 `.12` 保留 adapter `.11`。manifest 除插件版本外结构相同，自有 license 版本归属与适配器版本一致，上游文本未改；`candidate` 协议态保留。双语 README 和独立 support 均为 pending，没有提前宣称剩余验收通过。新的 Native 只读观测 `9b63a5312ccfbe759b1e4db23f74d9179f67cd13667c49f86229676fa835487a` 未改变预算、分发或请求。

**Standards P2：异步准入错误会触发逐帧重试。** `ContextRecord` 首版将所有 `DemandWait::Err` 标为已发布准备失败，但异步的 queue/waiter/collection quota 拒绝也由同 receiver 返回，可能尚无 reservation 或失败 publication。下一帧若因“publication 不存在”撤回该错误，便会在输入未变时再次 demand。已接纳，修复必须确认同一 intent 曾发布终态错误或保留明确阶段，不按错误字符串猜测。

作者另查实跨插件依赖缺口：Vue `.11` 只直接声明 TS 普通服务，collection `language.contribution` 在 TS `.14` 声明。首版只捕获 Vue 直接 collection 的空输入，无法修复已复现的 S4 失败。后续改为捕获原 scope 中已公开提供者声明的 collection 加源的直接声明，复用既有 epoch 与限额；只允许失败记录在相关作用域集合变化后重新准备，健康及 pending 服务不受此保守重试影响。

Spec 首轮对该缓存首版曾给出无新增发现；其中“不会逐帧重试”和直接集合覆盖结论已被上述具体调用链事实取代，不能作为该部分通过依据。新修订须再次独立复核。构建 session 53367 在 non-UI 编译中按精确进程身份停止，旧输出保留；它不是最终通过。没有在编译过程中修改冻结源码，后续以新冻结输入重新门禁。

## 04:11 回归与最后终态分支

原准入重试 P2 已取得真正 **RED 1.15s**，`target/s6-admission-retry-red-aligned-profile.log`：两项为 1 PASS / 1 FAIL，旧无条件错误标记明确触发 `admission rejection was requeued`；真实已准备错误的“先撤回、后送达”及先观察路径通过。程序 `9dd2ec0479c6ed9496b854d70db7436134aaf0f45f014956e1a1e824811283f8`，夹具 `45e2e4a9be8974a803b7229bb47199607f926ed0973fda6256ed649d48bf0d3c`。更早缺 RefCell、非法 deferred 声明及插件目录不一致的三类前置失败各自保留，均不计目标 RED。

恢复 `&& admitted` 后，Standards 独立读回 `contexts.rs` 为 `60c899fec9694978e48978dbf2503bcec59714d137c0a8abbc14b8263656d338`，确认原异步准入 P2 在源码层修正；无新增规范问题。Spec 同样确认原分支及传递集合捕获，另发现新的 **P2：已接受请求的超时／取消仍可自动重新准备**。`DemandRequest` 的 Cancelled 被转换为同一 Err，但 admitted 并不证明曾有完成的缓存 Err；Manager 丢弃全终止的 pending 后，原生界面会将 None 误认作失败撤回，并可每 60 秒重复同一目的。

该新 P2 已接纳，要求明确区分 Completed prepared error 与取消／超时终态，并补真实 accepted→cancel、没有 published Err 时多帧不重发的回归；不得仅根据错误字符串猜测。当前源码尚未获最终通过。门禁 session 89804 在 non-UI 编译中按精确所有权停止，保留部分输出，后续不将其计为完整通过。最终门禁改在本组回归与增量复核完成后执行。

### 04:18 取消路径的明确 RED

新增回归通过公开 `Manager.release_language_document` 取消已接受的原始文档请求，保持来源、Availability 与正常原生帧不变。初次 `target/s6-cancelled-demand-red.log` 为 **2 PASS / 1 FAIL、1.48s**，停在首帧没有保留 Failed 状态的断言；该位置本身不冒称已检查重新排队。随后将首帧队列断言提前，实际 `target/s6-cancelled-demand-explicit-red.log` 为 **2 PASS / 1 FAIL、1.50s**，明确报 `cancelled admitted preparation was requeued without a published error`。程序 SHA-256 `5ff367a1027924d85dddb3e1e7c4558808f076dd80bc5becd8511dab025f5c61`，测试源码 `c86a134ccf333a3538df4f08ceee3605ba2d4c10a625c302864e83c94edf9527`。两个此前通过的准入拒绝和真实错误撤回路径均保留。

不能简单将 CompletedErr 等同实际发布：activation pruning、Pending drop 和 deferred 准入拒绝也可能生成 CompletedErr。最终修复改为由原始 reservation 持有独立的历史发布标记，仅在同一 intent 实际缓存 Err 时置位，删除缓存后旧请求仍保留该证据，新 reservation 使用新标记。此时仍在实施，尚未记录 GREEN 或最终双轴通过。

### 04:22 最终发布证据增量复核

Spec 与 Standards 独立只读复核八项生产文件和三项回归，均为 **0 项新增可行动问题**；Standards 同时为 0 项硬性违规、0 项判断性坏味道。此前两项重试 P2 的源码路径已修正：原 reservation 的 Arc 仅在实际发布 Err 时置真，Worker 仅转交 CompletedErr 对应的原始证据，取消／超时／准入拒绝／receipt 校验失败不置位。先撤回后送达保留旧证据，同名新 reservation 不污染旧请求。集合输入继续使用原 scope、revision、Arc 与 alive，只有输入过期的终态 Err 被撤回；健康及 pending 计划不因该快照失效而退休。

两轴读回冻结摘要：runtime contexts `5c5e46417c88e8835e04ca70af567027855e653193bb16556e11f912cdf515be`，driver `9c7856b2ea93f69152527af30af838e99e5c2955906e51a81e571157d0983b2c`，App contexts `d0713367380655fe2d82ffc243add55427eca4ce65e388603bd0114cfecf3a5e`，回归 `c86a134ccf333a3538df4f08ceee3605ba2d4c10a625c302864e83c94edf9527`。审查者未运行构建、测试或 UI；此结论不代替正在编译的 GREEN、完整生态冲突恢复及最终原生／桌面门禁。

### 最终三项回归 GREEN

`cargo test -p editor-app --no-run -j1` **PASS 3m28s**，随后冻结 `target/s6-publication-provenance-green/deps/editor_app.exe`，SHA-256 `918351a79c4cbf0055d283b2c68c717747df49925feeef6afb95d74bfcead5a1`。直接运行该程序的 `extensions::worker::worker_tests::language_failures:: --nocapture --test-threads=1`，实际 **3 PASS / 0 FAIL / 0 ignored、1.66s**，`target/s6-publication-provenance-green.log`。两项真实 RED 对应的准入拒绝及取消均转为 GREEN；真实准备错误的两种撤回／送达顺序继续通过。完整生态冲突恢复与真实 Windows 故障场景另行执行，不外推此三项结果。

### 04:35 原生帧驱动的单变量审查

真实 Windows 场景原帧驱动在每次循环无条件调用贡献同步，与 `EditorApp.render` 按 provider generation／pending flag 的正常边界不同。该驱动产生大量尚未完成的重复 grammar 准备。当前仅修改测试 `Harness.frame`：保留 panel poll，改用公开 `window.refresh()` 与原 draw，让真正 render 执行其边界；不复制生产门控，不改产品、请求期限、10ms／20s 节奏、观察或任何类型／更新／故障／清理断言。

Spec 与 Standards 独立复核均为 **0 项新增可行动问题**，并在内存逆转唯一 hunk 后确认 SHA-256 精确恢复 `9b63a5312ccfbe759b1e4db23f74d9179f67cd13667c49f86229676fa835487a`；当前为 `4ea09524a20e0730a85bd20d6411ad1175d73d8cb3ebd8d51315395086ac0e2d`。没有其他字节变化。no-run **PASS 1m53s**，冻结程序 `target/s6-native-normal-frame/deps/editor_app.exe` 为 `706332eba442bf2ccb266fbe23b33b7f49c84d2cb43be487a25e0c2ebcabb399`。同 TS `.14` / Vue `.11` 的独占对照正在执行，不能由静态审查提前推断 GREEN。

后续实际对照已 **GREEN parent 164.27s / child 164.09s**，`target/s5-native-outer-normal-frame-solo.log`。三版本真实类型、两类正向候选与反向 TS 更新拒绝、旧包／分析／私有记录保持、外层 EOF 与有界重连、脏类型变化、旧结果撤销及健康上下文保持、撤信任和 checked cleanup 全部走完；完整验收收据存在。首个 20s 日志由约 1,778 条降至 10 条，Vue／TS 编译分别 26,127ms／24,849ms 被完整保留。原 222.60s 前置失败不删除，该 GREEN 与静态复核分别记证，不声称产品期限被扩大或独立 S4 已通过。

### 最终 Worker 测试文案增量

Worker 普通错误归属回归仍等待旧英文文案；相同来源在取消准备 ignored 屏障另有一处残留。只把两处期望改为当前 `plugins.command_owner_changed` catalog 和 `StaleRevision` 分类，保留原资源、归属、旧版本、撤权、日志及 checked shutdown 断言。Spec 与 Standards 对最终文件 `ff6902558fd918d86c8953a740dc251cc445bd4bac069ed2a0df950d641e460f` 独立复核，均为 **0 项新增可行动问题**，原残留 P2 在源码层解决，相关旧 literal 为 0。

实际普通全过滤 **6 PASS / 0 FAIL / 8 ignored、1.62s**；中英文安装进度 ignored 显式 **1 PASS、0.14s**。取消准备原入口旧版实际通用 publication 超时 126.70s；没有 backtrace，不把它冒称为精确的最后屏障 RED。修正后的同原例实际 **FAIL 129.11s**，`target/s6-worker-trust-catalog-green.log`；其 green 文件名不代表通过，`RUST_BACKTRACE=1` 仍没有调用者帧，失败阶段未知。该用例裸 ID 实际选择 capability `.17.1`（SHA-256 `20f5d815b109f46ab54e22bf56ccd89497c83fd4b2619250294e1a02cad249d8`），不是另行检查过的 `.19.1`。静态 0 发现不代替这项失败的行为验收。详见[实施记录](vue-typescript-remaining-2026-10-19.md)。

### 05:08 暂停状态

所有实施与诊断已暂停，代理已停止；未增加诊断代码、未延长期限、未打包或交付。三仓未提交进度、完整 S4 / S5 GREEN、生产门禁及当前失败均保留在[暂停检查点](vue-typescript-paused-2026-10-20-0508.md)，恢复须收到新的继续指令。

## 恢复后的最终增量与证据

用户已明确继续执行，05:08 状态仅为历史。原 Worker 失败来自测试缺失真实 UI 在 SetTrust 消息前更新 atomic 的交接；只修正 cfg(test) 夹具，没有放松生产信任门禁、期限或断言。最终源 `46d4d155ae45d3396ff95ae5013de727710699eb1110f602e73700d37c605577` 分别取得显式撤权 **1 PASS / 67.94s**、正向更新 **1 PASS / 67.84s**，普通过滤 **6 PASS / 8 ignored**。Spec 与 Standards 均独立复核为 0 项新增可行动问题；旧 129.11s / 127.43s FAIL 保留，未按 green 文件名改写。

普通桌面发现的通用悬浮链路随后分别补实证与独立双轴增量审查，详见[恢复记录](vue-typescript-resumed-2026-10-20.md)和[桌面记录](vue-typescript-desktop-2026-10-20.md)：

- 暖缓存持父更新租约时再次读 EditorApp，真实三项目回归 **RED 116.40s → GREEN 97.62s**。缓存选择和关联上下文读取推迟到后续前台轮次，保留取消隔离、revision 与 owner 检查；关联源关闭／重开回归另 **PASS 26.25s**。两轴各 0 项，原始报告 `target/s6-cached-hover-{spec,standards}-review.md`。
- 暗色卡片真实绘制对比度 **RED 1.31 → 六项卡片组 GREEN**；行内代码独立样式 RED→两项 GREEN，主题前景／背景显式进入基础 Markdown，消费者后置覆盖仍保留。两轴分别 0 项，`target/s6-hover-{colors,inline}-{spec,standards}-review.md`。只检查样式的用例未冒称直接测量文字 glyph。
- 同符号 MouseMove 清掉显式卡片、pending Escape 后迟到结果复活分别取得真实 RED。第一次修复 **6 PASS / 1 FAIL** 不计完成；最终共享 generation、显式键盘抑制清除和既有 shell capture 撤销 pending 后，hover／keyboard **7 PASS**、卡片 **6 PASS**、文字 **2 PASS**、Escape **9 PASS / 1 ignored**。组间重叠不相加；普通窗口实际 Ctrl+I → Escape → 同位置 Ctrl+I 可重开。两轴各 0 项，`target/s6-hover-reopen-{spec,standards}-review.md`。

最终九项支持元数据由独立作者实施，另两位代理分别按 Spec、Standards 审查，均 **0 项可行动发现**。限定 Windows x86-64 / TS 5.9.3 / Node 24.19.0 / 三个精确 Vue 版本及 26 个固定组合；协议 candidate、历史原始证据、版本与能力边界不变。Spec 报告 `target/s6-final-metadata-spec-review.md` SHA-256 `8296c0486bfba61d9732e4efbb5675a4c3d1633c210774a4eb352e2c21e357ff`，Standards 报告 `target/s6-final-metadata-standards-review.md`。两者均未将静态审查冒称最终归档运行。

最终包随后实际构建与运行：T .15 `beab4419…038b` / V .12 `5f6a14c2…1b73`；最终精确双包 **1 PASS / 0 FAIL / 0 ignored、97.62s**，policy **1 PASS / 179.74s**，versions **5 PASS / 0 skipped**。独立归档实读 T 22 / V 39 entries、29 / 44 冻结源、47 保留资源及 11 模块为 **0 项意外差异**；manifest 原始字节重序列化与深层语义相等分别记账。完整账本 `target/s6-final-supported-packages.json` SHA-256 `9f956d536d0ce054a600f929a379afc78677305f664216e6087e178af4e4697f`。

最终生产门禁 fmt、非 UI **319 PASS / 327 ignored**、workspace check 均 exit0；实际普通宿主 `1cae48c9…9f61` 与测试程序 `beed53fd…40f1` 分别冻结。已知 AccessKit 退出 COM abort 作为已有平台限制明确保留，不报告普通退出 exit0，也不扩大为本轮未做的上游修复。技术审查无未解决的可行动项；Git 与新工单关闭仍按交付回执独立核对，不以审查代替推送。

最后提交范围审查补充两处注释门禁项：H `editor/language_edits/completion.rs`、T `src/selection.rs` 新增的内联测试模块缺模块说明。现仅各加 `//!` 与空行；H fmt、T selection rustfmt 及 diff 检查通过，不改逻辑或重跑语义。最终归档与 checkout 全等结论限定打包核验时点；T 当前 selection.rs 比冻结源多这一测试说明，原冻结源与包未变，见 `target/s6-final-comment-only-postlude.{json,md}`。范围和最终文档审查将发现及解决分别保存在 `target/s6-final-git-scope-standards-review.md`、`target/s6-final-delivery-docs-spec-review.md`，不隐去修复前发现。

最后独立读回结论：范围 Standards **0 未解决项**；交付文档 Spec / Standards **各 0 未解决项**。索引过期状态 P2 已修，35 条 AC 原正文不变、34 条技术要求勾选而 S6-AC7 仍待实际 Git 回执。报告及 SHA 分别为 `s6-final-git-scope-standards-review.md` / `4b4143cee8981fe763e0ac1d9edfb1360049f1b293ba6eefdbcdd67f285880f6`，`s6-final-delivery-docs-spec-review.md` / `2a6a33835dfccb62385671295af3062784fa0ce185c8461db85215c347e37253`，`s6-final-delivery-docs-standards-review.md` / `2d1a81ef08f985197fbb2dfab98be334327ccacef1ce4f335f6e31eb57c4045c`，均在 H/target。两位分别核对 266 / 326 个相对本地链接有效，计数按各自审查范围保留，不相加。此后的真实 Git 回执独立追加，不冒称已被静态审查预先认证。
