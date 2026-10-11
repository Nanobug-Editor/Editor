# 03：工作区计划与当前会话恢复验收

日期：2026-10-11。议题：[#95](https://github.com/T-miracle/Nanobug/issues/95)。状态：候选测试与本单新增流程的 Windows 原生 C03 验收已完成；最终审查汇总、整合及远端交付由根代理记录，不提前关闭议题。既有 Settings 退役窗口的 UIA 崩溃已在基线复现，未修复。

依据：[已批准工单](../../tickets/plugin-api-community-foundation/03-workspace-transactions.md)、[共同完成条件与测试责任](../../tickets/plugin-api-community-foundation/README.md)、[方案](../../specs/plugin-api-community-foundation.md)。用户恢复后新增的源码 SDK 规则替代历史站点正文要求，终端沿用已交付宿主原生模块。

## 固定输入与独立构建

- 工作树：`C:/Users/Tmiracle/.codex/worktrees/plugin-api-transactions/Editor`，分支 `codex/plugin-api-transactions`；原项目已有改动不计入本单。
- 单文档预重构 `45e8af5` 和原生 Atomic 修正 `3b740f8` 保留在普通历史中。恢复时 HEAD 为 `29e0709`，完整在制品保存为 `d30f8c1a421ee2379b1ffd53c7335cd17a6f198b`。
- 按根代理整合指引，普通合并已验证基线 `de5397645d05c5d91e8cd74108d46ad9d7a88ff3`，合并点 `b5cf47a64ec2287b93f47c9b908d5b203c6453ea`。唯一人工冲突在 SDK 导出清单，保留基线的 38 项输入及本单新增 `workspace.rs`、`api/edits.rs`；没有整体采用任一侧或恢复旧终端插件。
- 包与宿主首次冻结的候选为 `b1fd89f4c4d0b6c2820620245c5d4f0f3d60d204`；它仅补公开包准入/写权限测试，与合并点的生产源码、SDK 和访客组件相同。该输入的宿主 SHA-256 为 `3B1313F61EC2464ACA335CDDC04FB43961DBE24F7BA2FC02A462397431FFC838`。后续原生无障碍发现的宿主修正另记输入，不能用旧宿主的观察冒称新宿主全部通过。
- Rust `1.98.1`、Cargo `1.98.1`，已安装 `x86_64-pc-windows-msvc` 与 `wasm32-wasip2`。VS Community 18 临时开发环境仅作用于本次命令，`CARGO_TARGET_DIR=../target03`。
- 首轮宿主 `cargo build -p editor-app`：退出 0，78 秒；独立包通过本次宿主 `--plugin-package` 正常 release 构建。接口用法、坐标、权限、错误、取消、释放、限额与平台边界在协议源码注释中。
- 首轮 SDK 指纹：`3cf0642fa835a8f78a259b2cda48055d1429626a488b08eebf402b34fce05249`。实际 `--export-plugin-sdk` 新目录共 40 项，Markdown 0 项，与对应协议源码逐文件差异 0；源包检查覆盖完整导出、并发发布、缓存复用及损坏修复。下表属于首轮完整行为矩阵输入，不能与后续修正产物混用。

| 独立包 | 协议与能力 | ZIP 字节 | SHA-256 |
| --- | --- | --- | --- |
| `workspace-scaffold 0.1.0` | 协议 7；`editor.documents 1.1`、`editor.edit 1.1`、`workspace.files 1.2`、`files.selection 1.1`、`ui.native 1` | 477610 | `58C8410F97A549ADBF4FAE63D8C880828CBE670AF81DC06F1EC2AFB2F31B5971` |
| `workspace-converter 0.1.0` | 同上，独立分页转换与 Save 消费者 | 479520 | `1691C96E27726A301E4693852D8A428E22576A0B6C4493A3742B6CEFE086E6B8` |
| `capability-example 0.19.0` | 当前基线消费者，用于受影响的选择授权回归 | 691733 | `9E9EC5F0A67A7A8E20D0C39CF5C73145B600A02E110596901E319B52BD404D03` |

两个本单新包均通过发布 SDK 缓存独立编译，没有仓库 `crates/` 路径依赖或附带 SDK 源码。所有测试数据在独立临时目录；根代理的桌面验收夹具由根代理独占。

正式包内组件指纹：脚手架 2594282 字节，SHA-256 `313C05DC7286E7D2433041F18A10677C183025C18201F1C783F3A1537ED7E74D`；转换器 2596172 字节，SHA-256 `22D330F688F89824E214F17C7CFCAEF132EFA0671D12F3D2974A5E38D723DB17`。原生开发验收仅复制这些组件和清单，不另编译访客。

## 原生命名与源码指导修正后的固定输入

SDK 注释普通提交 `70edad0f25ec04cdf3068d500276a3c1440912d9` 准确区分 `Stat` 的磁盘 stamp 与 `Read`/文档快照的当前原生版本，没有修改操作实现。无障碍普通提交 `0318970594dbb99e7fdf784de803d4d6bc9b1c3b` 给预览取消/应用与批次 Undo 提供同文本本地化名称，并把标题声明为语义标题；共享控件保持原实现。这个提交是本轮生产源码固定点。

组合宿主构建退出 0，17.94 秒，SHA-256 `074D7B4415A3B6CE8374A2EF8F0AE9F212145E5EAD58B14C86D4C6AFD7630D00`。当前 SDK 指纹 `bdb62a0a048bc64e171bc84e59170a1887106592c1ad4e49ff2258e5e89fe943`，实际导出 40 项、0 Markdown、逐项源码差异 0。SDK 只改说明，但缓存输入已变，仍确实重编三个正式包；两个业务消费者分别 20.52/21.19 秒，既有选择消费者 32.69 秒。

| 当前独立包 | ZIP 字节 / SHA-256 | 组件字节 / SHA-256 |
| --- | --- | --- |
| `workspace-scaffold 0.1.0` | 477430 / `206C5334E7DBA07B8337D2E977620B92E608F9EFE69B8EEBD6CA38105695DAE7` | 2594302 / `398384CF18DFC7DD586D76B67FAD3A018E2A748D0E0BBC6266BAB5FE0E7E9DC7` |
| `workspace-converter 0.1.0` | 479422 / `DF243CC4F0F51CA93B222F4E63FE5B3A4047389E762406A79F1C97C2F152229A` | 2596178 / `70016C867DB4EAA04A2FE47B80C626ABBE06E572EA7ADAA6DACC543B4FC2FD61` |
| `capability-example 0.19.0` | 691712 / `293D83B29AD0280653D3EE791F4892758C428E0521EC23E59C035213D731EDB9` | 3582921 / `80864CC9123AA30084B31D1BE63530E881BF7A422BE50D5D4D56314F7034E005` |

元数据与全部绝对路径保存在 `../target03/workspace-a11y-native-inputs.json`。首轮 ZIP 原样保存在 `../target03/workspace-before-a11y-packages/`，旧宿主静态副本由根代理保留。当前 SDK 两个消费者的代表连接均通过，涵盖脚手架完整八步/恢复和转换分页/Escape/guest 取消/混合修改/恢复，增加预览取消/应用/Undo 的真实 Base 绘制 `role/label/visible` 与 Dialog role 断言；没有重复未改行为的全部负面矩阵。UIA 的最终实体观察另由根代理执行。

## 可观察行为与测试责任

| 验收 | 接缝与断言 | 当前结论 |
| --- | --- | --- |
| T11 | 真实 SDK `ApplyEdits` 非相邻中文/CRLF/非 BMP 范围合成一次原生 Atomic；前后键入分别 Undo/Redo；磁盘不保存 | 实际包矩阵通过；原生 Atomic 回归通过 |
| T12 | 脚手架一个计划完成创建目录、未打开文件写入、复制、连续两次改名、新建文件与删除；转换器分页发现并读取当前原生缓冲区；外部 ReadWrite 与 Save 的必要操作边界 | 实际包矩阵通过；02 受影响的选择授权回归 5/0 |
| T13 | 打开脏文本保持未保存，磁盘文件按已确认计划落盘；明确原生预览确认，Escape 或 guest 取消不会提交等待中的计划 | 实际包矩阵通过；桌面取消、两步混合 Apply/Undo 与 Native Save 通过，见[原生记录](03-workspace-transactions-native.md) |
| T14 | 预览后的磁盘变化、revision、目标关闭/重开阻止执行；大小写别名仍读取和修改脏缓冲区；真实目录 junction、设备名、ADS、越界、私有根与同号外实例句柄被拒绝 | 实际包矩阵及公开 Manager 边界用例通过 |
| T15 | 整组 Undo 恢复文本、被删除文件和改名链，移除本批创建内容；后续键入及局部 Undo/Redo 使整组自动恢复在任何效果前停止 | 实际包矩阵通过 |
| T16 | Windows 真正的文件占用导致提交和 Undo 中途失败，保留真实完成索引与部分恢复状态，解除占用后可重试；文本步骤后取消保留已发生效果；文件观察的创建/修改/删除、配对或明确未配对改名、合并、溢出重扫和根释放 | 原生 watcher 3/0，实际包恢复矩阵和公开 Manager 事件用例通过 |

宿主只保留有限不可变恢复前像与协调状态；文本修改和恢复始终调用唯一 `DocumentSession/EditorState`。普通 Undo 仍局部执行。取消不表示自动回滚，恢复限于当前会话，不承诺全磁盘原子性或重启后整组 Undo。部分恢复状态保持观察到的实际磁盘状态，供再次校验与显式重试。

## 本轮真实失败与修正

- 最初新建文件错误复用了 Windows 父目录属性，导致错误 87。计划中新文件改用文件自身默认属性，已有文件和复制仍保持实际属性；真实脚手架完整成功与恢复已通过。
- 文件观察调用曾因正式方法表漏列而返回 `UnknownMethod`；补入通用协商方法表后两个真实 SDK 消费者可订阅。
- 大小写别名实际读到了原磁盘文本，暴露绕开脏会话问题。授权路径使用现有对象的规范拼写，缺失尾路径仍逐祖先检查；真实别名读写与恢复已通过。
- 两次复制 16 MiB 源文件原先进入预览，但 48 MiB 计划后像超出观察与恢复捕获预算。现在每个计划中间态都执行相同内容/条目/深度预算，预览前报告 `LimitExceeded`，完成索引和影响目标均为空，原磁盘不变。
- Windows notify 的真实改名没有 tracker，不能凭猜测报告配对改名。测试接受可验证的配对改名，或明确 delete/create 两个实际转移；原生工作线程的创建、修改、改名与删除已通过，溢出与边界独立验证。
- 选择测试曾提前读取宿主内部临时 `Unit`，后又读取尚未完成的 `Accepted`。消费者现在只跟踪自己明确发起的 picker 请求，从正式终态事件观察实际授权句柄；不改宿主授权机制，不增加测试专用宿主调用。
- 新增准入测试最初假定不支持的 required 版本能进入 Manager，却被正式 `Package` 提前拒绝，测试夹具的 `unwrap` 因此失败。修正夹具保留公开包准入错误，只重跑这一用例；缺失 `workspace.write`/`editor.write` 的真正包请求被明确拒绝，宿主请求与磁盘效果均为零。
- 首次最后门禁在链接阶段报告 `LNK1104 msvcrt.lib`，未开始非 UI 行为测试。该失败 shell 没有记录 `LIB`，因此没有确定根因；后续成功命令在同一 shell 的 Cargo 调用前记录临时 VS `LIB`、库绝对路径与 `Test-Path=True`，使用两并行 Cargo 作业，没有安装或改变全局环境。
- 原生 UIA 看见预览两个按钮但没有名称，虽然屏幕文字存在。先增加真实 Base 绘制属性断言，红灯准确观察到 `None` 而预期 `Cancel`；修正后当前 SDK 两个业务连接 2/0。标题初次补语义时误用了按钮的 `accessibility_label` 方法，编译指出普通 GPUI div 应使用 `aria_label`，该中间失败日志也保留。
- 独立 Spec 审查发现 `Stat` 使用说明错误，现已修正为准确的磁盘/当前缓冲区版本读取来源；源码缓存指纹改变，因此重新构建当前 SDK 包，不以旧包替代当前输入。

这些中间失败日志保留在本工作树外的 `../target03/`，没有计入通过结果。debug 中间包和旧 SDK 指纹仅保留为失败定位证据，本表使用合并后的正常 release 包。

## 命令、结果与剩余验收

命令均在本单工作树执行，使用上文临时 VS 环境和独立 target；构建与打包直接调用 Cargo 和当前宿主，没有调用历史脚本。

| 命令或检查 | 日志（`../target03/`） | 结果 |
| --- | --- | --- |
| `cargo build -p editor-app` | `workspace-merged-host-build.log` | 退出 0；现有未使用项和 Tree-sitter/Wasmtime 链接警告保留 |
| `editor-app --plugin-package plugins/workspace-scaffold plugins/workspace-converter --output target/community-transactions` | `workspace-merged-release-packages.log` | 退出 0；两包均使用本表 SDK |
| 当前宿主重建 `capability-example` | `workspace-merged-selection-fixture.log` | 退出 0；当前 `0.19.0` 包 |
| 当前宿主导出 SDK，并逐项比较源文件 | `workspace-merged-sdk-metadata.json` | 40 项、0 Markdown、0 源码差异 |
| `cargo test -p editor-app workspace_transaction -- --test-threads=1` | `workspace-merged-native-observations.log` | 3 passed、0 failed、10 ignored；跳过的真实包测试由下一行独立执行 |
| `cargo test -p editor-app workspace_transaction -- --ignored --test-threads=1` | `workspace-merged-actual-sdk.log` | 10 passed、0 failed、0 ignored；245.54 秒 |
| `cargo test -p plugin-runtime --test workspace_transactions -- --ignored --test-threads=1` | `workspace-merged-runtime-authority-watch.log` | 初轮 2 passed、1 failed、0 ignored；失败属于上述夹具准入假设 |
| 仅重跑 `workspace_transaction_missing_write_grants` | `workspace-merged-write-grants-green.log` | 1 passed、0 failed、0 ignored；23.67 秒。两个已通过的边界/事件用例没有重复执行 |
| `selected_resources` 的 5 项受影响真实包授权回归，显式 `--ignored --test-threads=1` | `workspace-merged-selected-resource-regressions.log` | 5 passed、0 failed；226.39 秒 |
| `cargo test -p editor-app -- --test-threads=1 text_transactions sdk_export` | `workspace-final-atomic-sdk-regressions.log` | 6 passed、0 failed、0 ignored；1.67 秒 |
| `cargo fmt --check`，两独立消费者格式检查 | 终端输出 | 退出 0 |
| 首次 `cargo test --workspace --exclude editor-app` | `workspace-final-nonui-workspace.log` | 退出 101，MSVC 报无法打开 `msvcrt.lib`，根因未确定；该次未执行 `cargo check` |
| 命名断言红灯：实际 SDK 脚手架用例 | `workspace-a11y-red.log` | 0 passed、1 failed；真实按钮名称 `None`，预期 `Cancel` |
| 标题初次命名的中间构建 | `workspace-a11y-host-build.log` | 退出 101；已修正普通 div 的语义方法，无全局环境修改 |
| 组合新宿主 `cargo build -p editor-app -j 2` | `workspace-a11y-host-build-green.log` | 退出 0；17.94 秒 |
| 当前 SDK 重建两消费者、选择消费者与导出比对 | `workspace-a11y-release-packages.log`、`workspace-a11y-selection-fixture.log`、`workspace-a11y-native-inputs.json` | 退出 0；指纹按上表，40 项 / MD0 / 源差异0 |
| 当前 SDK 脚手架及转换器代表连接，显式 `--ignored --test-threads=1` | `workspace-a11y-current-sdk-green.log` | 2 passed、0 failed、0 ignored；55.49 秒。新增无障碍断言随真实业务入口执行 |
| `cargo fmt --check` | `workspace-a11y-gates-fmt.log` | 退出 0 |
| `cargo test --workspace --exclude editor-app -j 2` | `workspace-a11y-gates-nonui.log` | 退出 0；241 passed、0 failed、206 ignored。ignored 不计通过，相关本单案例在上文独立执行 |
| `cargo check --workspace -j 2` | `workspace-a11y-gates-check.log` | 退出 0；48.77 秒 |

当前构建、门禁与应用 Green 的临时工具环境分别保存为 `workspace-a11y-build-green-environment.log`、`workspace-a11y-gates-environment.log` 与 `workspace-a11y-green-environment.log`。最后的三门禁对应组合生产输入 `0318970`，验收 Markdown 不参与 Rust/SDK 编译。基线已有未使用项与 Tree-sitter/Wasmtime 链接警告保留，未进行无关清理。

原生桌面验收由根代理使用同一冻结宿主、两个正式 release 组件和 `profile-final/` 完成：公开 dev 只运行脚手架一个主项目，正式 Converter 作为 bundled 包进入同一 Manager。一次历史双项目 dev 启动被 CLI 明确拒绝，退出 1、没有 GUI 或数据效果；独立 profile 的早期准备方案不是最终成功路径。原生八步预览/Escape、带未保存中文与 emoji 的 live 和未打开 disk 两步 Apply/整组 Undo、Native Save 选精确完整路径后再明确 Apply 均已通过。报告是严格 UTF-8 无 BOM、30 字节，Apply 前不存在；完整指纹和逐步结果见[原生验收记录](03-workspace-transactions-native.md)。删除和完整恢复由上述真实 SDK/GPUI 临时目录矩阵覆盖，桌面没有重复执行删除。

同一 profile 重启后，根代理通过实际 Settings 按钮切换 Dark 并将字号从 14 调到 16；新八步预览完整可见，中文取消/应用操作和中英标题有实体完整 UIA 名称，Tab、Escape 与返回菜单焦点均有稳定回读，最后控制器正常 `stop` 返回 0、所属窗口与进程均为 0。英文按钮名称由实际 GPUI/Base 测试覆盖，没有宣称实体英文 locale 切换。第一次关闭 Settings 时遇到的失效 HWND/COM abort 也在没有 03 事务或消费者的已验证基线重现；本轮只调整验收时对该窗口的查询方式，没有修复上游缺陷。主题和字号是当次内存状态，不承诺本次重启持久化。C03 相关结果与限制均按原生记录区分。

Standards 和 Spec 两轴由根代理组织固定候选独立复审。C01/C02/C04/C05 的源码 SDK、包准入、权限与清理证据、C06 三门禁及受影响应用测试见上表；C03 见新增原生记录，相关 Dark/字号布局、新预览和正常停止待项已结清。最终双轴汇总、整合、推送、远端 SHA 核对和议题状态由根代理交付时记录。本记录不提前宣称工单交付完成。
