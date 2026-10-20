# TypeScript 共享源上下文选择验证

日期：2026-10-20（Asia/Shanghai）。状态：实施中。本文记录 #122 / #125 的上下文选择接缝及真实失败历史，不将新包可构建或测试源码存在记为交互验收通过。其余场景见[连续实施记录](vue-typescript-remaining-2026-10-19.md)。

## 代码与产物身份

- `H`：`C:/Users/Tmiracle/.codex/worktrees/vue-typescript-s1/Editor`；`TS`：`C:/Projects/RustProjects/TypeScript-for-Nanobug`。两仓仍含本轮未提交实现；本文不是其初始 HEAD 的验证结果。
- 公开构建入口：`H/target/s6-sdk-4af-host/editor-app.exe`，SHA-256 `d587836c7a97be4b22eec9fd95ba3778226da8ca877b906d8426d5f819264412`。实际 SDK 为 `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`，54 个输入及摘要见 `H/target/s2-context-sdk-inputs-frozen.json`；未修改缓存或添加宿主私有源码依赖。
- 实际交互程序：`H/target/s5-updates-unified/deps/editor_app.exe`，SHA-256 `c535ea71de04e1cfe9b9162172585800c7899a1606c3011b1e4cbe421ef9eee7`。它包含原生 QuickPick、固定交互准备、注册上下文、来源继承和独立 source epoch 接线。

| 输入 | SHA-256 / 身份 | 状态 |
| --- | --- | --- |
| `TS/target/s2-context-package-011/typescript-0.1.11.zip` | `d1c582b64262cd56bd4c09c22868f8d88f697b83791d9006027b24ce6185b427`；22 entries | 保留实际交互失败输入，未覆盖 |
| `TS/target/s2-context-package-012/typescript-0.1.12.zip` | `f869529d1906d0a06c8a6c90225dfa09377092c1ae6654f20273a5ebb4a1c3a5`；22 entries，WASM `becfaeacf031da235832f6b586e145c826de56cbc5b4e0eda626b3b84cc37885` | 修正偏好键的候选，实际交互尚在 F12 断点 |
| 两包内原生服务归档 | 真实服务版本仍 `0.1.11`，SHA-256 `1d89ed0c86f7e9b98b0bca55441e1b80db5c0b8b8029200da46a1620796770ad`，40,312,628 B | `.12` 只改 guest/元数据，原生字节未改变 |
| `H/target/s2-context-green-fixture/context-selection-fixture-0.1.1.zip` | `cf01bfc49fffedb36bc7bd57ed93783bf92e3234c4d2e2ce137e1cb0abfd54a0` | 当前公开 typed SDK 消费者已构建；扩展注册/生命周期测试待运行 |

两份 TS 包的逐 entry 摘要、组件及清单信息分别保存在 `TS/target/s2-context-package-{011,012}-proof.json`。构建前的 `cargo metadata --no-deps --format-version 1` 已核对 TS workspace_root，以及公开构建器实际采用的独立 guest target。独立 fixture 同样核对自身 workspace_root，未误用宿主 workspace。

## 公开契约与原生选择

宿主公开 `language.lsp 1.6` 的注册上下文引用，引用绑定实际 owner、workspace scope、规范化配置根及配置代次；根路径本身不授予文件权限。原生导航可交出一次性来源提示；显式 Refresh 仍检查原始 DocumentVersion、当前提供者、取消及授权。TS 插件拥有候选项目、选择顺序和本机偏好，不存在宿主 TS/Vue ID 分支。

固定 interactive prepare profile 允许直接宿主准备中的 QuickPick 和私有偏好 CAS。只有完整匹配该 profile 的声明可包含 `storage`，普通 service 方法仍拒绝；嵌套调用也不继承第一跳的私有 namespace。公开 `.11` Package 首次因旧白名单拒绝该 profile；`H/target/s2-context-interactive-profile-{red,green}.log` 保存真实定向 RED→GREEN，后续新 SDK 才成功打包。

显式重选只撤销所选源的已发结果，不重启健康项目；未知工作区文件/config 事件仍可保守地撤销已发结果和预览，而保留健康绑定与 transport。本文不承诺 B 的旧 preview 跨未知文件事件保留。

## 偏好键的实际 RED 与修复

持久化键使用 `PreferenceKey { file_type: "", name }`，只保存逻辑 project key 和原始 source path；不保存 live token、编译器内存或源码。

1. 旧 `DefaultHasher` 不构成固定持久化编码。golden 回归首先实际失败，45.64s；`TS/target/s2-context-preference-key-red.log`。
2. 初次改为 `project.v1.` 加完整 SHA-256 hex 后，golden 通过，但没有校验公开键长度。该 `.11` 包在真实宿主中 **FAIL 203.26s**：`InvalidRequest: Invalid private preference key`；`H/target/s2-shared-context-first-011.log`。未达到 QuickPick，更没有证明后续 F12、重选或偏好保留。
3. 最小定位：该编码为 75 bytes，公开 `PreferenceKey::validate()` 明确只允许 `name <= 64`；空 file_type 合法，缺少 grant 则应为 PermissionDenied，不能用扩大宿主上限掩盖插件缺陷。
4. 给 golden 增加真实公开 `validate()` 调用，**RED 0.93s → GREEN 0.76s**；`TS/target/s2-context-preference-length-{red,green}.log`。修复编码为 SHA-256 的完整 64 个小写 hex，输入固定为 `UTF8("typescript.project.v1") || 0x00 || UTF8(host_portable_path)`。不转换路径大小写，不做 Unicode 规范化，不截断摘要；`sha2` 精确锁定 `0.10.9`。
5. 升为 `.12` 后，同一公开 SDK 的 golden **1/1 PASS 3.48s**、WASM `check --locked --offline --release --target wasm32-wasip2 -j1` **PASS 0.20s**、Package **PASS 13.82s**。日志分别为 `TS/target/s2-context-preference-key-012-green.log`、`s2-context-guest-012-check.log`、`s2-context-package-012-build.log`。这些结果不是宿主交互通过。

`shared.ts` 的稳定键 golden 为 `73b150774612b970e524746544636b2c0e14fbdc748154c6cb3dbb7648ec1128`；另含中文路径、大小写和组合 Unicode 区分，且每个返回值实际经过公开键校验。

## 真实交互的验收条件

`extensions::worker::shared_context_tests::shared_typescript_choice_is_native_scoped_and_retains_navigation_provenance` 从独立 ZIP 安装进入 production Worker、原生 DocumentSession 和标准 LSP。每个配置用 paths alias 把同一源的类型分别指向 `"left"` 与 `42`，不是手填根路径替代项目选择。

原有断言及期限保持：取消歧义选择不发布部分 binding；QuickPick 选择 left 后实际 hover；选择 right 使 A 的旧 capture/preview 失效而 B 的原 Arc/capture/preview 保留；非空私有偏好逐字节保留；关闭重开读取保存的 right；真正焦点输入和 F12 从 left 的变量使用处跳原始 shared 声明且临时沿用 left；再次重开仍回 right；磁盘原文和 checked shutdown 保持。

不兼容更新场景通过实际 ZIP 的 `process >=99` 元数据及 Worker Inspect → Package::read 拒绝，保留候选 ZIP/hash/原因与前后非空 preference bytes，随后检查 installed digest、A/B 原 Arc 和真实类型。它证明宿主 API admission 拒绝，不冒充 TypeScript engine 或 adapter 版本语义拒绝；后者由独立恢复矩阵验证。

`.12` 按同一 C535 程序、原有断言和期限复跑，实际 **FAIL 63.45s**，日志 `H/target/s2-shared-context-second-012.log`。本轮已经走过取消、left/right 选择、A/B 旧 capture/preview 隔离、不兼容 ZIP 拒绝后的原 Arc/类型/非空偏好字节、关闭重开 right；真正 F12 也已跳到 shared 原始声明。但紧接着的 hover 仍未符合临时 left 的断言，整条用例不计通过。

不兼容 ZIP SHA-256 为 `0a3a6c474a85b4c7ed50521e3885e8b160683171068d8ab790de232185a77e1b`，真实失败原因为 `Required capability unavailable: process >=99, <100`。第二次运行的 ZIP 与前后 preference bytes/installed digest 收据已在后续复跑前原字节归档到 `TS/target/s2-context-package-012/shared-context-evidence-second-012/`；`metadata-admission.json` SHA-256 为 `188fb5fd0cac8655a3ca8773398593ea19f759a3316eedb7978f35aba0b8fcf1`。非空记录 revision 为 2，不以空目录代替偏好保留。

定向观测使用 `H/target/s6-native-format-context-locale-red/deps/editor_app.exe`，SHA-256 `8a5bf17237c3716fff31488f8ab106a5a9dc0667b6b4a0fa8c93186a812fb6e3`；生产未变，保留原有 30s 导航期限。先区分目标焦点先发布而 observer 读到旧 right Arc、来源缺少注册 provenance、提示消费后旧 alias 仍被复用。第三轮实际 **FAIL 93.87s**，`H/target/s2-shared-context-third-012-observation.log`：源 plan 为规范化 left、目标旧 plan 为 right，注册 binding 与两根一致；打开正确声明后 30s 目标仍为旧 right Arc，所有 native 状态 Ready。因此不能以延长 hover 等待掩盖该缺陷。

沿真实 F12 路由定位到 `main.rs::request_definition`：它直接调用 `open_definition_uri`，没有经过 `documents.rs` 的 show-document 来源继承回调。最小修复让 F12 捕获请求时的实际 source plan，投递时保留 request/path/revision 校验，并核对 owner 活性与当前 source 仍使用原 service；随后与 show-document 共用 `open_definition_with_source`。目标先从文件 URI 规范化并验证为实际文件，再交出一次来源提示，不修改持久偏好或用路径替代授权。引用结果列表仍通过原 sealed origin 与 DocumentVersion 校验后使用同一来源继承机制。新回归增加目标实际 plan.root 为 canonical left 的断言；修复后的实际宿主重放待跑，尚未计作通过。

## 原生服务工具证据

### F12 修复后的实际交互

上述最小修复已实际 **GREEN 70.95s**，`H/target/s2-shared-context-f12-green-012.log`；宿主冻结 `fe5919…a075242f`、TS `.12`。取消、两次选择与源级隔离、真实 F12 继承 canonical left、非空偏好 revision 2 逐字节保持、后续直接重开仍 right、实际类型、磁盘与 checked shutdown 均通过。此前三次失败保留，不改写。本结果证明该冻结宿主和 `.12` 的交互，最终 `.14` 联合归属由主验收记录另列。

同一 `1d89…70ad` 闭包已有 source **44/44、0 skipped、51.079s**，`TS/target/s2-context-typescript-node-source-final.log`；解包后的 sealed **44/44、0 skipped、49.689s**，`TS/target/s2-context-typescript-node-sealed.log`。它们是相同测试的两种加载身份，不是 88 个不同场景，也不替代上述宿主交互。新增 project-reference 回归及后续完整 45 项工具结果由[主记录](vue-typescript-remaining-2026-10-19.md)单列，本段 44 项保留当次真实输入身份。
