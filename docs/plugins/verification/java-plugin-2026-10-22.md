# Java 插件实施记录

日期：2026-10-22

状态：候选交付验证中。S1–S4 关键行为、五代 JDK 矩阵、S5 生命周期与现行构建门禁已通过；Java 25 `import module` 语法补丁仍待构建工具安装授权，整体尚未验收。依据：[已确认方案](../specs/java-plugin.md)。

## 最新验收结果

以下结果替代历史增量中的待验证状态；历史失败日志继续保留。忽略项不计为通过。测试使用隔离插件／数据根，没有安装到用户真实配置。

| 范围 | 实际结果与证据 |
| --- | --- |
| Java 8／11／17／21／25 | 0.1.16 包的分析、编译运行、完整启动调试共 15 项通过；`target/java-s5-final-jdk*-*.log`。另以 0.1.17 在独立目标工作目录复验五代完整调试，5 项通过，`target/java-s5-cwd-jdk*-debug.log` |
| 原生 Java 编辑与运行 | 0.1.19 包、当前 GPUI 0.8：真实 grammar、未保存 Unicode 分析、快速修复／整理 import、跨文件与类文件重命名／整组 Undo、过期预览、原生编译／运行生命周期共 8 项通过；`target/java-final-native-language-suite-0119.log` |
| 原生 Java 调试 | 0.1.18 包、当前 GPUI 0.8：F9、线程／调用栈、对象／数组分页、三种单步、暂停／继续、UTF-8 控制台及字面参数、单次启动与实际停止退出通过；`target/java-native-gpui08-debug-0118.log`。0.1.19 仅作格式与元数据整理，未改变此行为 |
| 普通项目与主类 | 0.1.19／JDK 11：多源码根、包结构、工作区外本地 JAR、中文路径／诊断、失败编译不返回旧产物通过；另一个主类的目标参数与 class 生成已验证，准备阶段不执行主类；`target/java-final-multiple-main-jdk11.log` |
| JDK 失败与离线 | 实际包缺失 JDK 时的离线、HTTP 503、取消三种失败路径通过，未启动编译器、信任撤销后拒绝；`target/java-sdk-failure-lifecycle.log`。已有受管 JDK 8 离线复用并完成编译运行通过，`target/java-final-sdk-offline-reuse.log` |
| 生命周期 | 暂停调试时停用、卸载通过；`target/java-final-debug-{disable,uninstall}.log`。0.1.18 的真实热更新及同时保有编辑后端与暂停调试目标的更新通过，旧来源不可再次启动、原生进程退出、资源归零；`target/java-update-lock-green-0118.log`、`target/java-update-with-live-lsp-0118.log` |
| 权限与旧消费者 | 实际 SDK 只读依赖选择边界、原生上下文归属／授权通过；`target/java-readonly-selection-pass.log`、`target/java-native-context-grants.log`。旧 `debug.session 1.1` 消费者与 Rust 原生调试通过；`target/java-debug11-consumer-regression.log`、`target/java-legacy-native-debug-cold-budget.log` |
| 普通回归 | `cargo test --workspace --exclude editor-app -j 2`：513 通过、375 忽略、0 失败；`target/java-s5-workspace-final.log`。应用运行模块另有 108 通过、11 忽略，SDK 导出 6 通过；`target/java-final-run-regression.log`、`target/java-final-sdk-export.log` |
| 独立插件与构建 | 最终公开 SDK 上的 Java guest 3 项、原生索引互斥 1 项通过；`target/java-final-{guest,native}-tests.log`。`cargo check --workspace -j 2`、完整 `cargo build -p editor-app --release -j 2` 及最终主程序的 `--plugin-cargo … check --target wasm32-wasip2` 通过；`target/java-s5-workspace-final-check.log`、`target/java-final-host-link.log`、`target/java-final-host-sdk-check.log`。0.1.20 包再经公开 Manager 安装、冷配置表单和字面参数／构建绑定验收通过，`target/java-final-candidate-0120-smoke.log` |

原生验收是 GPUI 真实输入事件、布局与可见内容检查，并运行实际 Java 服务／目标；没有物理桌面人工验收。Windows 之外尚未构建或验收。当前文件编辑桥不支持包／目录级重构与资源创建、删除。

最终匹配宿主副本为 `target/java-host/20261022-185900/editor-app.exe`，SHA-256 `068576EA9C162DF4BB7BD0915EC4B4745225F2EB3051CD7C09E9C5054E3045D0`，与完整 Release 主入口一致。导出的 SDK 缓存身份为 `228de968a1f2efd7fef9e9f3ee67632799e5d105cc46d6790be871453c1fe3ca`。最新候选包为 `C:/Projects/RustProjects/Java-for-Nanobug/dist/java-0.1.20.zip`，SHA-256 `EE8B7916BEFAD69DD6D5E461205A4F0B2CCDB54441E0B0697280387730BC4BF5`，大小 55,748,621 字节。0.1.20 仅同步双语说明和包／原生适配器／bundle 版本，业务行为与 0.1.19 相同。包校验确认 151 个条目均无路径穿越，原生文件摘要与清单一致，grammar／查询／许可证／说明与源码一致；SDK 缓存中没有 Markdown 或宿主 crate。

唯一已复现的首版语法缺口是 Java 25 `import module`：现有 Tree-sitter Java 0.23.5 能解析 compact source，不能解析模块导入，见 `target/java-grammar-module-import-red.log`。补丁已保存于独立工程 `licenses/tree-sitter-java-module-import.patch`，但 Tree-sitter CLI 和 Emscripten 不在现有工具链中，私有安装授权尚未回复。没有将这个缺口记为支持或交付完成。

## 实施入口与范围

用户要求开始执行，按既有 S1–S5 顺序推进。当前工作区存在大量已有修改，实施保留这些内容。独立插件源码位于 `C:/Projects/RustProjects/Java-for-Nanobug`；不在宿主仓库恢复随附插件。

用户随后确认沿现有方案继续执行，本轮保持顺序实施，不另拆正式工单；没有创建或发布议题，也没有选择并行实施。

Lombok、Maven、Gradle、JUnit 仅列入后续计划；本轮不实现它们的工程识别、导入、依赖、测试或启动扩展逻辑。

## 测试接缝

沿用方案中实际插件包、公开管理器与原生编辑／运行入口。插件公开 SDK 语言准备入口的针对性测试验证普通项目配置及服务边界；完整安装、进程与原生交互仍由真实包验收，针对性测试不能替代它们。

## 历史阶段记录

- 已核对宿主 `cargo metadata`：workspace 为 `C:/Projects/RustProjects/Editor`，输出为该仓库 `target`。
- 已核对现有 Java/Javac 为 25.0.2，已安装 Rust `wasm32-wasip2` 和本机 Windows MSVC target。
- `cargo build -p editor-app --release` 已通过，存在工作区既有的编译／链接警告。
- Java 公开语言准备入口的两条针对性测试已通过：普通项目配置及关闭 Maven／Gradle 导入；输出路径越界在启动服务前拒绝。两项均记录了先失败、后实现通过的过程。
- 独立包已通过宿主 `--plugin-package` 产出 `Java-for-Nanobug/dist/java-0.1.0.zip`。固定来源为 Tree-sitter Java 0.23.5（ABI 14）与 JDT LS 1.62.0-202610131109，摘要位于独立插件 `licenses/sources.json`。
- 已显式运行 `cargo test -p plugin-runtime --release --test java_package -- --ignored --nocapture`：实际包经公开 Manager 安装后返回真实 Java 类型错误，停用后旧服务不能重新启动，且未在工作区生成 `.project`；1 项通过，日志 `target/java-package-s1.log`。
- 该真实包回归先暴露并修复了 WASM 中 Windows verbatim 路径拼接，以及无文档意图的工作区准备错误发布项目身份的问题。
- 新增通用 `tools.inspect 1.0`，实际消费者通过只读工具元数据选择 JDK。独立 WASM 夹具经公开 Manager 显式运行，验证不会执行故意失败的版本探测参数，以及目录穿越和超量读取均拒绝；2 项通过，日志 `target/java-tool-metadata-test.log`。
- 新增显式选择的 `language.lsp 1.7` 依赖配置路径解析：首次准备依赖后，在初始化选项和配置中解析完整的依赖路径字符串，继续校验租约、真实路径边界和大小；普通字符串保持字面含义。实际 Java 8 场景先暴露未准备、首次配置缺少真实路径，修复后通过。
- Java 11 的真实固定发行包先触发原 128 MiB 下载限额。`dependencies 1.3` 将通用源文件限额设为 256 MiB、ZIP 解包总量设为 512 MiB，下载、摘要校验与解包均检查真实字节数；保持固定摘要、文件数量、链接边界及取消检查。Java 清单要求该最低能力版本。
- 项目 Java 8、11、17、21、25 的真实安装包选择、项目编译器文件、语义诊断及停用撤销分别通过。日志为 `target/java-package-jdk8.log`、`java-package-jdk11.log`、`java-package-jdk17.log`、`java-package-jdk21.log`、`java-package-jdk25.log`。8/11/17/21 使用隔离私有发行，25 复用现有本机 JDK；这不是完整编译／运行／调试兼容矩阵。
- 声明式禁用本机工具发现后，真实包自动准备独立的语言运行时 Java 25 和项目 Java 8，完成语义诊断；日志 `target/java-package-managed-runtime.log`。未修改全局 PATH 或用户真实插件目录。
- 支持本机项目 `java.jdkHome` 覆盖及独立 `java.runtime`；无效的显式项目 JDK 明确报错，不自动替换。对应回归先因缺少设置声明失败，随后通过。
- Java 25 的真实连接返回补全、悬浮、定义、引用、重命名及格式化结果；与无效显式 JDK 回归共 2 项通过，日志 `target/java-package-editing.log`。这些结果尚未代替跨文件事务与原生呈现验收。
- S1 阶段 `cargo fmt --check`、`git diff --check`、`cargo check --workspace` 均通过。完整 `cargo test --workspace --exclude editor-app -j 2` 在本进程指定已安装的 MSVC／Windows SDK 库目录后通过：455 通过、356 忽略、0 失败；忽略项不计为通过。日志 `target/java-workspace-tests-final.log`。首次 MSVC 链接失败及随后旧 `language.lsp` 版本断言失败均保留在阶段日志中；没有修改全局环境或安装工具。
- 原生 Unicode 验收暴露 JDT 推送诊断缺少文档版本，使用别名亦不能可靠关联实际源码。现由插件内独立 OSGi bundle 验证工作副本 SHA-256，再通过标准拉取诊断返回结果；原生适配器只保存摘要和请求关联，不保存文档文本，不为无版本推送补造当前版本。宿主继续使用通用 revision 和连接代次校验。
- 实际 Java grammar 与原生未保存 Unicode 输入的 GPUI 验收通过：2 项通过，日志 `target/java-native-pull-final.log`；检查实际错误范围选中 `"wrong"`，以及插件退出和状态快照。此项不替代物理桌面 IME、主题、滚动或运行／调试验收。
- 同项目不同文档发现代次必须共享一个项目身份；真实包经 Manager 验证关闭首个文档仍保留另一文档的同一服务，停用后撤销。与语义诊断和无效显式 JDK 共 3 项通过，日志 `target/java-package-pull.log`。
- 拉取诊断路径下重新验证 Java 8、11、17、21、25，五项分别通过：补全、悬浮、定义、引用、重命名、格式化、错误诊断及服务撤销。日志 `target/java-package-pull-jdk{8,11,17,21,25}.log`。修改后先等待新源码的诊断轮次，避免与 JDT 异步工作副本更新竞争。这仍不是编译／运行／调试矩阵。
- S1 最新候选包为 `Java-for-Nanobug/dist/java-0.1.1.zip`，SHA-256：`5961d9371da78ddd788958a42072ca939f654d86ece8a335486fb2bac39d1d98`。该包的显式本机 JDK 和缺失时私有准备回归也分别通过，日志 `target/java-package-pull-explicit-home.log` 与 `target/java-package-pull-managed-runtime.log`。
- S2 已接入通用 `language.content 1.0`：插件声明非文件 URI 和内容请求，宿主复用只读原生文档、来源身份、严格位置换算、原始服务权限与撤销流程。0.1.1 实际包先因缺少 JDK 定义失败（`target/java-library-content-red.log`），0.1.2 修复后通过（`target/java-library-content-green.log`）。原生 F12、内容呈现和拒绝输入用例亦通过（`target/java-native-library-2.log`，1 项）；新增的关闭源文档后生命周期检查尚待重跑。
- 真实本地 JAR 用例通过：从工作区外单独编译夹具，将带 `-sources.jar` 的库及纯二进制库交给实际包；前者返回原始源码标记，后者返回含实际方法体的反编译结果。两者均保留 `jdt:` 来源，日志 `target/java-local-jar.log`。没有把夹具源码作为工作区源目录供语言服务直接读取。
- JDT 的普通源码项目支持仍会调用 Maven 源码下载器，关闭 Maven 导入本身不足以阻止该回退。Java 自有 bundle 通过上游 buildSupport 扩展，仅覆盖普通项目的远程源码发现，保留既有本地 classpath 行为；并关闭 Maven／Eclipse 源码下载设置。依据：[上游普通项目支持](https://github.com/eclipse-jdtls/eclipse.jdt.ls/blob/main/org.eclipse.jdt.ls.core/src/org/eclipse/jdt/ls/core/internal/managers/InvisibleProjectBuildSupport.java)、[源码下载器](https://github.com/eclipse-jdtls/eclipse.jdt.ls/blob/main/org.eclipse.jdt.ls.core/src/org/eclipse/jdt/ls/core/internal/managers/MavenSourceDownloader.java)。Maven 能力继续留给独立插件。
- 新公开模块首次独立构建暴露 SDK 导出遗漏，实际 `--plugin-package` 因模块文件缺失失败；已补齐 SDK 的公开源码导出。候选包随后重新经宿主 `--plugin-package` 构建成功，日志位于独立项目 `target/package-s2-content.log`。临时使用同一个公开 development builder 的构建回归亦通过，未在访客工程加入宿主源码依赖。
- S2 首次原生跳转用例未能执行，编译时发现工作区其他改动中的 `ActionGesture: Debug` 缺失；随后 workspace 检查又遇到 `WorkspacePresentation` 枚举匹配缺失，后者已由并行工作区更新补齐。本轮未改动这两处实现；将以恢复后的实际源码重新执行检查。
- 用户授权最小 `Debug` 修复后，画布已有工作自行移除了该格式化依赖，故未添加多余派生。原生菜单和 glyph 布局的后续编译错误也随其工作区更新恢复。宿主 Release 已重新构建通过（`target/java-content-sdk-build.log`）。全仓格式检查曾检出其他工作区文件的问题（`target/java-content-fmt.log`），Java 独立工程格式检查通过。按仓库默认换行设置执行的 `git diff --check` 已通过（`target/java-diff-check.log`）；此前覆盖 `core.autocrlf=false` 导致的 CRLF 空白误报已排除，不计为仓库缺陷。当前全仓门禁仍需最终重跑。
- 两个已打开且未保存源码的跨文件重命名，先在实际原生 F2 用例中因“目标是另一文档”失败（`target/java-native-rename-red.log`）。新增公开 `language.editing 1.2` opt-in 声明及精确源码向量后，0.1.3 实际包通过同一用例：确认前均未改变，确认后一起修改，磁盘不变，整组 Undo 恢复两份未保存源码。日志 `target/java-native-rename-green.log`。此处未把尚未打开文件及资源重命名算作完成。
- 未打开目标的原生用例随后因“请先打开相关源码”失败（`target/java-native-rename-closed-red.log`）。现在首轮结果只发现目标，按原始工作区权限有界读取并保留路径 pin，交给唯一的原生 DocumentSession 后重新计算完整重命名；没有直接套用首轮磁盘范围。每轮至多 64 份、合计 8 MiB，最多四轮发现，原始连接／配置／文件事件及源选择条件均保留。两条打开／未打开目标用例通过（`target/java-native-rename-closed-green.log`，2 项），预览、磁盘不变和整组 Undo 均已检查。资源文件改名仍待验证。
- 补充只读源码生命周期验收：先实际保存并关闭原先有未保存内容的源文档，断言它确实退出原生标签集合，再检查库资源仍存活。0.1.3 实际包通过（`target/java-native-library-lifecycle.log`）。只读视图没有第二份可编辑 Rope 或 Undo。
- 快速修复真实菜单起初只有普通重构选项，没有 Java 错误修复（`target/java-native-quick-fix-red.log`）。原因是标准拉取诊断只交给显示层，未进入同一 LSP 诊断上下文；现将精确源码的完整拉取报告纳入原存储，推送补充独立保留以避免重复累加。修复后，原生菜单选择类型修复、确认前不修改、确认后只更新缓冲区的用例通过（`target/java-native-quick-fix-green.log`，1 项）。整理 import 与过期预览尚待后续用例。

- 整理 import 随后经同一真实包和原生菜单验收通过：移除未使用的 `ArrayList`，保留 `List` 导入与使用，确认前文本不变，确认后缓冲区更新、磁盘不变。日志 `target/java-native-imports-keyup.log`（1 项，含前述类型快速修复）。第一次重跑遗漏 Base 按钮依赖的 Enter 松键事件；补齐完整键盘事件后通过，未修改产品选择逻辑。过期预览与资源改名仍待验证。

- 顶层类改名的实际原生用例先暴露静默缺项：未声明资源操作时，JDT 返回并立即应用类名文本修改，却不改文件名（`target/java-native-type-rename-red.log`）。已实现待验收的通用 `language.editing 1.3` 文件凭据桥，按原始工作区读写授权签发并复用文件事务，分析前保存磁盘版本；源码、文件名、标签路径和 Undo 交给同一原生事务。`cargo check -p editor-app --tests --jobs 2` 已通过（`target/java-type-rename-check.log`），行为尚待 0.1.4 实际包重跑。
- [JDT 客户端能力判断](https://github.com/eclipse-jdtls/eclipse.jdt.ls/blob/main/org.eclipse.jdt.ls.core/src/org/eclipse/jdt/ls/core/internal/preferences/ClientPreferences.java) 将三种资源操作绑定判断，仅声明 `rename` 仍会省略文件名。Java 自有适配器在内部接收完整资源提案，再按宿主真实声明逐项校验整份重命名回复；不允许的创建／删除返回整项错误，不丢弃后继续应用文本前缀。宿主继续只声明实际支持且已授权的文件操作，没有 Java 专属分支。

- 0.1.4 新包切换资源提案后，JDT 同时序列化空 `changes` 与实际 `documentChanges`，原来的通用解码器把空伴随表示误判为两套修改冲突。现在只移除空表示，两个非空表示、注解及不支持操作仍整项拒绝；旧混合表示回归的输入同步改为两边都有实际修改。随后类文件改名还暴露异步入队前释放文件凭据的问题，现由 `LanguageFixOrigin` 随原始提案保留凭据到 Manager 签发正式请求，生命周期不再依赖 UI 入队函数的栈。
- 修复后类名、物理文件名、标签路径与整组 Undo 的实际原生用例通过（`target/java-native-type-rename-lease-green.log`，1 项）；快速修复和整理 import 也恢复通过（`target/java-native-actions-empty-green.log`，1 项）。三个跨文件用例全部通过（`target/java-native-renames-s2-final.log`）：已打开未保存源码、未打开目标重新纳入原生状态，以及预览期间相关源码输入后整项作废。
- 实际包通过公开 Manager 验证文件提案的缺少 `workspace.write`、目录穿越及停用后的即时撤销（`target/java-file-grants.log`，1 项）。语言模块普通回归 44 通过、24 忽略（`target/java-s2-language-regression.log`）；通用事务先构建真实 `lsp_fixture`，再显式运行分析守卫回归，2 项通过（`target/java-s2-analysis-guards.log`）。普通命令中另外 12 项事务夹具用例仍为忽略，未算通过。
- S2 验收使用 `dist/java-0.1.4.zip`，SHA-256 `AFCCF351FF4E3C4B92253B81972E7628698AD98D138A5FCFEACD21F2FCA782B0`。运行中的主入口阻止 Cargo 将已链接的 `target/release/deps/editor_app.exe` 复制到 `target/release/editor-app.exe`，两次 Release 命令因此报告失败，日志 `target/java-type-rename-build.log` 与 `target/java-s2-host-final-build.log`。没有停止其他任务的进程；已对链接产物制作同摘要副本，副本的 `--plugin-cargo .../Cargo.toml check --target wasm32-wasip2` 通过（`target/java-host-copy-sdk.log`）。不能把主入口复制失败记为完整 Release 构建成功。
- `cargo check --workspace --jobs 2` 通过（`target/java-s2-workspace-check.log`），独立 Java guest／native 工程格式检查及标准 `git diff --check` 通过。全仓格式检查当前只检出其他任务的 `editor-history` 文件（`target/java-s2-fmt-final.log`）。完整 workspace 测试先遇到其他任务的历史字段缺失，该字段随后由工作区更新补齐；本轮新增的凭据字段又暴露一处遗漏的既有测试构造，已补齐。最新完整命令仍在其他任务的 `legacy_expiry_removes_only_the_older_snapshot` 失败（`target/java-s2-workspace-tests-final.log`），正在单独检查其余模块；不将过滤后的结果称为完整 workspace 通过。

## 阶段增量记录

S3 增量记录：

- 0.1.5 实际包经公开 Manager 验证原生 Java 应用模板、主类和字面参数保存；两轮先失败后通过，见 `target/java-run-form-green.log`、`target/java-run-validation-green.log`。构建动作恰好一项的回归亦先失败后通过（`target/java-run-build-sequence-{red,green}.log`），随后对齐现有准备协议，构建与最终启动使用同一个便携绑定。
- 新增通用 `run.targets 1.1` 参数前缀，保留 1.0 的程序名结果；宿主和独立插件使用同一规范。编译由现有、可取消的准备资源会话执行，完成后才交付最终 Java 启动命令，不另建后台任务系统。
- 冷项目 JDK 25 真实编译通过（`target/java-cold-build-cursor-green.log`）；无界面 ConPTY 消费者需要自行应答 `ESC[6n`，测试通过公开输入接口处理。该记录是编译 helper 的阶段验证，不代替后续准备、运行和控制台验收。
- 原生窗口实际新建 Java 配置、输入主类、保存、点击构建和点击运行的用例通过（`target/java-native-configuration-run.log`，1 项）。构建产生 class 而不执行主类，运行后才写入中文内容文件。追加的中文控制台呈现断言尚待重跑；公开管理器的原始输出断言失败正在诊断（`target/java-preparation-compiles-green.log`，文件名不表示结果通过）。
- 冷项目 JDK 8 先明确失败于缺少准备（`target/java-cold-sdk8-red.log`）。通用 `dependencies 1.4` 只读依赖选择钩子已通过运行时编译检查（`target/java-service-selection-check.log`），限定已声明、已授权计划的子集，复用原始调用权限、取消和私有依赖租约。0.1.6 插件消费与实际下载／编译尚待验证，未计为完成。
- 最新 `cargo check --workspace -j 2` 通过（`target/java-s3-host-check.log`）。原生测试编译曾被历史设置模块的 Future 生命周期错误阻断，工作区其他任务随后修复，重编译后完成前述原生用例；未修改该模块。宿主 Release 主入口仍有其他进程占用，链接与复制结果分别记录，不把复制失败算作构建成功。

- 新版宿主 Release 已完整通过（`target/java-service-selection-host-build.log`）。实际导出的 SDK `f1fa10fe5325763d9aa676b07f3f4043a95c5ba5dd9e480ae55a5ed0c40e79ca` 独立检查通过（`target/java-service-selection-guest-check.log`）。用于构建的同摘要主程序副本为 `target/java-host/20261022-143706/editor-app.exe`，SHA-256 `BA0671A737F63DDD60CBDA5EAB00A2399F79F05D891374882CE02441CD3E5F28`；文件夹名仅为本轮副本名称。
- 原始输出证实中文被错误代码页二次转换（`target/java-console-byte-diagnosis.log`）。Java 自有启动器在本次独立 ConPTY 中先设置输入／输出 UTF-8 代码页，再启动 JVM；未修改全局设置。该处理与 [OpenJDK 的 Windows 控制台编码选择](https://github.com/openjdk/jdk25u/blob/master/src/java.base/windows/native/libjava/java_props_md.c) 一致。0.1.6 实际包的 JDK 25 编译、显式启动及中文输出通过（`target/java-console-sdk25-green.log`）。
- JDK 8 自动准备与编译随后通过，但旧 JVM 无法加载带 Windows 扩展前缀的 classpath（`target/java-console-sdk8-green.log` 实际为失败）。0.1.7 只在 JVM classpath 参数中转为普通驱动器／UNC 表示，宿主和编译器的路径 pin 保持规范化；同一真实包冷启动用例通过，包含 Java 8 class 版本、中文路径、中文输出且未启动 JDT LS（`target/java-jdk8-classpath-green.log`）。

- 冷配置目录调用在新版组件首次激活时触发 30 秒超时（`target/java-configuration-017.log`）；未出生的提供者现在使用有界的 120 秒冷准备预算，已激活的普通元数据调用仍为 30 秒。相同实际配置用例通过（`target/java-cold-catalog-budget-green.log`）。WASM 指令及每次回调执行预算未放宽。
- 完整原生用例明确复现停用后目标仍运行（`target/java-native-console-lifecycle-4.log`）。新增通用执行来源继承，目标准备结果保留原始提供者，配置来源亦进入同一原生会话；启动前校验原始工作区、权限和实例，运行期间跟随实例退休清理。修复后实际新建配置、中文 stdout／stdin、含空格与引号的字面 argv、Stop、启动、Rerun 恰好一次及停用撤销全部通过（`target/java-native-console-lifecycle-green.log`，1 项，0.1.7 包）。主类栏固定在命令区 scroller 之上，测试按真实所属面板检查，没有把子 scroller 的边界误当作它的裁剪边界。
- 既有运行配置回归 27 项、序列回归 9 项通过（`target/java-s3-run-regression.log`、`target/java-s3-sequence-regression.log`）。协议／运行时库回归 184 项通过、3 项忽略（`target/java-s3-protocol-runtime-lib.log`）。来源继承后的 workspace 编译检查通过（`target/java-s3-origins-workspace-check.log`）。
- 多源码目录、包结构、外部本地 JAR、中文输出目录已实际运行，失败编译不返回旧启动产物。错误消息首先出现中文乱码（`target/java-plain-project-build-red.log`），0.1.8 为管道中的编译器设置旧版与当前 JVM 的 UTF-8 标准流属性。一次重跑触发有界 WASM 预算中断（`target/java-plain-project-build-green.log` 实际失败），同包再次运行通过，包含可读中文诊断（`target/java-plain-project-build-repeat.log`）。已加安全的调用耗时采集；未扩大 WASM 预算，这次中断仍列入最终稳定性复查。
- 本轮完整 workspace 测试因其他任务在编译期间移除历史旧测试文件而未执行完成（`target/java-s3-workspace-tests.log`）；全仓格式检查还有历史模块和其调用方的未格式化改动（`target/java-s3-fmt.log`）。本轮文件的格式和差异另行核对，不把上述门禁称为通过。

- 0.1.8 完整原生构建／运行生命周期重跑通过（`target/java-native-console-lifecycle-018.log`，1 项）。随后完整 `cargo test --workspace --exclude editor-app -j 2` 通过，501 项通过、368 项忽略、0 项失败（`target/java-s3-workspace-tests-fresh.log`）；这次结果替代前述执行中删除文件的失败记录，不把忽略项算通过。
- S4 真实断点用例首先失败于尚未声明调试提供者（`target/java-debug-breakpoint-red.log`）。Microsoft Java Debug Server 0.53.2 取自官方 VS Code Java Debug 0.60.0 固定分发，JAR SHA-256 `A1844DC833EF5237F03D68F794CC2EB879331C41DD7B5C27D1E0B0C616AFC52E`；来源、分发摘要及许可证保存在独立 Java 工程 `licenses/java-debug.json`。

S4 增量记录：

- 通用 `process 1.9` 返回已声明原生服务的所属包资源与私有数据路径，不授予文件句柄；实际独立 SDK 夹具验证不同实例的私有根、缺少执行授权、缺少服务授权、未声明服务和停用后的拒绝，1 项通过（`target/java-native-context-grants.log`）。
- 0.1.9 实际后端揭示 Java Debug Server 底层参数必须为字符串，按官方 Windows 解码规则编码字面 argv 后，0.1.10 的实际断点、调用栈、局部值和停止清理通过（`target/java-debug-breakpoint-0110.log`）。0.1.9 的超时记录保留（`target/java-debug-breakpoint-019-current.log`），不记为通过。
- 新增通用 `debug.session 1.2`，保留 1.1 原有签名；线程、显式线程栈和变量分页分别校验会话、暂停、线程与帧。原协议回归 8 项通过（`target/java-debug-inspection-contract.log`）。真实线程树用例首先失败于缺少能力（`target/java-debug-tree-red.log`）。
- 实际包进一步验证对象字段、循环引用、数组分页与集合的上游延迟值解析。Java 默认只暂停事件线程，插件先等待 `pauseAll` 的真实停止事件，再发布可检查暂停，继续使用后端的 `continueAll`；没有伪造其他线程的栈。0.1.14 经公开 Manager 的完整检查通过，包含 worker 栈与 `workerValue = 99`、跨线程／帧引用拒绝和停止清理（`target/java-debug-tree-0114.log`，1 项）；同一候选的独立进程重跑亦通过（`target/java-debug-tree-0114-direct.log`）。包 SHA-256 `1712C68D31FBF1DE01381D09D8EF1C1446F122911D26DD479FDB121452D55F8A`。
- 原生面板与 F9 源码断点已接入；实际用例成功到达线程检查，随后两次未能滚动回主线程，日志 `target/java-debug-native-0114.log` 与 `target/java-debug-native-0114-2.log`。第二次记录显示帧区仅 22.5 像素高，正在扩大检查区并保持状态行单行显示；尚未将原生调试验收标记通过。
- 宿主测试编译和生产编译分别通过（`target/java-debug-inspection-ui-check.log`、`target/java-debug-inspection-production-check.log`）。期间历史测试的类型／可见性错误由其他工作区任务更新，不归入本轮成果。调试启动正补齐与普通运行一致的配置和目标准备来源继承，最终需重跑真实包验证。

S5 增量记录：

- 0.1.15 的完整原生调试用例通过（`target/java-debug-native-0115-3.log`，1 项，76.91 秒）：原生配置表单、F9 断点、线程选择、对象和数组展开、加载更多、三种单步、暂停／继续、中文控制台输入及含空格、引号、尾随反斜杠的字面参数。目标只启动一次，停止后观察到实际退出。0.1.16 同入口复验通过（`target/java-native-debug-final-0116.log`，1 项，88.52 秒）。这些是 GPUI 原生测试事件与可见布局验收，没有声称物理桌面操作通过。
- 原生面板高度问题已修复：检查区保留 260 像素并允许使用 75% 终端区，状态单行截断；变量滚动测试按实际可见项点击，不使用被裁剪项的假定位置。运行模块回归 28 项通过，包含旧线程、帧和暂停代次结果拒绝（`target/java-s4-run-regression.log`）。
- 调试启动保留原始配置与目标准备来源，逐项校验后再去重并继承实际授权、工作区和实例寿命。真实包用例追加停用后原准备来源无法再创建调试目标的断言；该路径参与最终矩阵。
- 首轮 0.1.15 五代矩阵中，Java 8／21／25 的分析、编译运行和完整调试通过，Java 11／17 的分析通过，编译因 `javac` 拒绝 Windows `\\?\` 前缀而失败。0.1.16 仅在规范化和路径 pin 完成后转换编译命令参数的路径表示，保留 UTF-16／UNC 身份；真实 Java 11 helper 重跑通过（`target/java-jdk11-native-compiler-green.log`），最终实际包矩阵继续执行。首轮日志 `target/java-s5-jdk*-*.log`，复验日志 `target/java-s5-final-jdk*-*.log`。
- 实际 grammar 在原生 WASM loader 中通过 Java 8 lambda、11 var、17 record、21 guarded switch、25 compact source 的代表语法（`target/java-grammar-versions.log`）。Java 25 `import module` 明确失败（`target/java-grammar-module-import-red.log`）；已准备上游语法补丁，私有安装缺失构建工具的授权仍在等待，未把该语法支持记为完成。
- 本轮宿主测试编译曾发生一次分配失败，完全相同入口重跑通过（`target/java-debug-native-test-build-4-retry.log`），没有清理缓存或更改编译配置。Release 链接已生成，但其他进程占用主入口使 Cargo 的最终复制失败（`target/java-debug-host-final-build.log`）；不能记为完整 Release 构建成功。用于当前 SDK／包构建的链接产物副本为 `target/java-host/20261022-171325/editor-app.exe`，SHA-256 `D01A5D51C899D8E2ECB9C67030888D132F8FF70D366F9DC1A02833164C7B9B38`，其 SDK 缓存身份为 `450925b1a3e6d7e8d773f401188de61cf431e451517c1c5f45ee9d653fd0c211`。
- 全仓格式检查当前只检出其他任务的 `editor-ui/src/aligned_editor/geometry_measurement.rs` 与 `editor-ui/src/scroll/tests/native_windows.rs`（`target/java-s5-fmt-final.log`）；本轮文件已单独格式化，`git diff --check` 通过。全仓测试及新增的 SDK 失败、只读准备边界和旧调试消费者回归仍在读回最终结果。

以上为各阶段当时的结果。S5 的最终矩阵、失败路径、生命周期和构建门禁现已读回通过，见本页“最新验收结果”；当前仍需处理 Java 25 模块导入语法补丁及最终交付核对。尚未提交、推送、创建议题或发布。

S5 最终增量：

- Java 11／17 的编译路径修复后，0.1.16 五代矩阵全部通过；0.1.17 还修复了调试目标工作目录被误作项目索引根的问题。现在原始工作区来自公开准备环境，目标 `cwd` 只用于目标进程；五代独立目录调试均通过，未新增宿主 Java 分支。
- 真实热更新先失败于 Eclipse 配置锁文件不能读取，日志 `target/java-update-lock-file-diagnosis.log`。宿主仅补充通用私有文件复制错误的目标路径上下文。0.1.18 在 Java 原生适配器以规范化索引路径命名的操作系统互斥保护后端所有权，并关闭 Eclipse 字节锁，保留更新预备期间的数据可读性；依据见 [Eclipse 运行参数](https://help.eclipse.org/latest/topic/org.eclipse.platform.doc.isv/reference/misc/runtime-options.html)。互斥在子 JVM 退出前持有，强制退出可由操作系统释放；没有跳过锁文件或修改宿主快照语义。实际暂停调试更新随后通过；再持有真实编辑后端进程，旧进程和调试目标共同退休的用例也通过。后一个用例保有真实语言进程，但没有将其算作同时完成一轮语义分析。
- 当前工作区 GPUI 升级后，原生测试曾因 `gpui` 测试依赖名与现有导入冲突报 335 个编译错误。用户明确授权后，仅将 `crates/editor-app/Cargo.toml` 的测试依赖别名改为 `gpui-test-foundation`，包名、固定版本与功能不变；重新编译通过，见 `target/java-s5-app-alias-fixed-build.log`。此前用户授权的 `ActionGesture` 修复最终仅添加了 `Debug` 派生，未改变手势行为。
- 旧 Rust 原生调试首次冷准备两次超过测试中的 30 秒等待；记录见 `target/java-legacy-native-debug-regression.log` 与 `target/java-legacy-native-debug-repeat.log`。仅把该用例首轮等待改为现行 120 秒冷准备预算，后续控制等待不变，真实用例通过。生产调试启动仍为有界 180 秒；没有扩大 WASM 执行时间或燃料。
- 历史阶段中的全仓格式、历史测试编译及 Release 主入口占用均已随当前工作区复查恢复。未把其他任务的 GPUI、控件或历史修改算成本轮成果。最终门禁和原生套件使用当前 GPUI 0.8 源码与明确的独立二进制副本；没有覆盖正在运行的主程序。
- 最新 `cargo fmt --check` 和 `git diff --check` 均通过，见 `target/java-final-fmt-delivery.log` 与 `target/java-final-diff-delivery.log`；后者仅有 Git 的 LF／CRLF 提示，没有差异错误。最终候选通过宿主直接 `--plugin-package` 构建，见 `target/java-final-package-0120.log`。源码复核检查了通用执行来源、只读准备、原生上下文、重命名 revision 与有界调试树，并补齐线程行模型注释；没有宣称已执行以提交为起点的正式双代理代码审查。
