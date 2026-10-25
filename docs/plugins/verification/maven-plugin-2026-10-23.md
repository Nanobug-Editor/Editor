# Maven 插件实施记录

日期：2026-10-23

状态：本轮确认范围已完成；Maven 0.1.18／Java 0.1.34 的最终实际包测试、原生多模块验收及宿主门禁均通过，产物待用户审阅，尚未提交、推送或发布。依据：[Maven 方案](../specs/maven-plugin.md)。用户已选择 Maven，并授权按推荐进行私有安装及缺失时自动准备。清理会话于 2026-10-23 14:34 放行后才恢复构建与验证。

## 0.1.18 刷新收尾

- Maven 包 `C:/Projects/RustProjects/Maven-for-Nanobug/dist/maven-0.1.18.zip`，SHA-256 `72F1D4E53EFA300E560BEBEC2A1FF83BF310E59F3C45D462D74CF1B61477E532`；配套 Java 仍为下列 0.1.34。使用同一公开 SDK 独立打包，`target/maven-package-018.log`。新增在成功发布模型时失效发现代次，撤销导入等待期间产生的暂时错误。
- 两个通用运行时缺陷分别有快速失败及修复证据：`target/maven-context-seal-red.log`／`target/maven-context-seal-green.log` 证明借用方退休必须撤销原生持有的标记；`target/maven-source-reservation-red.log`／`target/maven-source-reservation-green.log` 证明已排队的原文档请求在缓存被撤销后须重新预约，不能反复准备而无法完成。修复后分别 1 项通过，第二项从 2.05 秒失败变为 0.05 秒通过。临时跟踪代码已撤回。
- 原生宿主现在发布通用语言发现代次，界面据此替换旧的等待与失败状态；没有 Maven／Java 标识专属分支。导入面板的测试还等待设置的新可用性代次发布，避免拿旧回执发送命令；有限任务泵保持真实截止时间可观察。
- **实际 GPUI 多模块场景通过**：`target/maven-native-discovery.log`，1 项、177.76 秒，无跳过。未保存的库方法被应用识别，两个文档共享传输；真实二进制依赖只读导航及输入拒绝、POM 变动后撤销、重新导入和重新绑定均通过；刷新后新增 `afterRefreshMissing` 的独立源码修订收到新 JVM 的语义诊断。没有用旧诊断充当刷新成功。
- 宿主最新门禁：`target/maven-workspace-final-018.log` 中 **545 项通过、396 项按仓库配置跳过**；`target/maven-check-final-018.log` 的 workspace check 通过；`target/maven-format-delivery.log` 的交付格式检查通过。两个独立访客和原生工程格式检查均通过。额外枚举的通用原生控制夹具仍有 2 项 ignored，未准备该独立夹具、未计为通过，见 `target/maven-native-control-regression.log`。
- 安全输出的最新宿主为 `target/maven-host/20261023-final/editor-app.exe`，SHA-256 `22DFA1C6C38E67034FB28F6EE5DE661794D80E6BDD5C16CFCEDF5CE8AEEF78E8`，直接 Cargo Release 构建 5 分 59 秒通过，`target/maven-host-final-build.log`。SDK 已导出到 `target/maven-sdk-final`。没有覆盖用户正在使用的主输出 EXE，也未安装到用户实际插件资料目录。
- **最终 0.1.18／0.1.34 实际包测试 13 项全通过、无跳过，952.09 秒**，见 `target/maven-actual-018-034.log`。使用专用 `target/maven-acceptance/runners/final-018-034.exe`，设置 `NANOBUG_MAVEN_PACKAGE`、`NANOBUG_JAVA_PACKAGE` 为上述 ZIP，并执行 `--ignored --nocapture --test-threads=2`；除下列既有范围外，刷新用例还重新发现、完成新原生绑定并核对新旧服务身份与有效性。下列 0.1.17 记录仅保留为前序证据。
- 交付包的 protocol 7、WASM 魔数、Windows 原生 PE、许可证及资源检查通过；Maven 内嵌固定发行摘要与 helper 版本一致，Java 包内含 `ProjectGraph.class` 且 bundle 版本一致。方案与记录中的本地相对链接检查通过，见 `target/maven-delivery-artifact-check.log`。使用时需同时采用支持公开 `language.lsp 1.9` 的更新宿主及 Java 0.1.34；未安装到用户实际资料目录。
- 最终三份文档链接、两个 ZIP 和宿主 EXE 摘要再次核对通过，见 `target/maven-delivery-final-check.log`；`git diff --check` 通过，见 `target/maven-delivery-diff-check.log`，仅有既有工作区的 LF／CRLF 提示。

## 0.1.17 阶段验收与失败历史

- Maven `0.1.17`：`C:/Projects/RustProjects/Maven-for-Nanobug/dist/maven-0.1.17.zip`，SHA-256 `815BB7B1A52A63F04597CE221F6E846EC1E72B3EF82C754EB7B2237FEE9B37B3`。协作 Java `0.1.34`：`C:/Projects/RustProjects/Java-for-Nanobug/dist/java-0.1.34.zip`，SHA-256 `CC81B212E88D1F31A7A486C0840E53F483BBFC8AA8C164BD4D8337D5F8650913`。
- 两个包均使用 `target/maven-host/20261023-metadata/editor-app.exe --plugin-package <源码目录>` 独立构建，日志 `target/maven-package-017.log`、`target/java-package-034.log`。该宿主副本 SHA-256 为 `817F8EE5FB0E3E71EADE7CA8A57B5A8F2579250AAE4E389C63091360F11E2523`；未覆盖或停止用户原有 Release 进程。
- 该阶段实际包测试 **13 项全通过，无跳过**，775.65 秒，`target/maven-actual-017-034.log`。入口为专用 `target/maven-acceptance` 测试二进制，设置两个 ZIP 环境变量，执行 `--ignored --nocapture --test-threads=2`。覆盖有效单模块／reactor／profile／BOM／传递依赖、失败保留、面板／取消／工作区事件、Java 诊断与定义、Maven 排除项、库发起的跨模块未保存重命名、各模块独立 source／target、旧绑定撤销、实际构建运行及调试、目标停止与重跑。
- 应用用例包含 64 个真实隔离仓库 JAR，实际类路径超过单参数 4 KiB，使用受控环境传递；实际断点行的栈帧、局部变量 `result=7` 及停止后零 Java 进程通过。停用 Maven 后再次启动旧上下文返回／取消为 InvalidHandle，未启动进程。
- 库发起重命名的旧候选失败见 `target/maven-graph-red.log`，只返回 Library 编辑；把整个工作区链接到每个私有工程的候选仍失败，见 `target/maven-graph-green.log`。依照固定 [JDT URI 映射](https://github.com/eclipse-jdtls/eclipse.jdt.ls/blob/v1.62.0/org.eclipse.jdt.ls.core/src/org/eclipse/jdt/ls/core/internal/JDTUtils.java) 核对后，各工程只链接自己的源码目录；`target/maven-graph-source-links.log` 通过，183.57 秒。最终 13 项再次通过。
- 编译级别旧候选的失败证据为 `target/maven-compiler-red.log`：库模块缺少 source 8／target 25。最终候选由 Maven 的表达式求值器导出标准参数，Java 各工程保持独立；真实诊断只在 source 8 模块拒绝 record，source 25 模块接受。
- 原生 OS watcher 的创建／修改／重命名／删除用例通过，0.79 秒，`target/maven-native-os-watch.log`。GPUI 的 FileWatch 在 cfg(test) 中明确停用原生线程，因此界面场景通过生产 `apply_reconciliation` 接口投递真实落盘 POM 事件，不能把它说成原生监听线程验收。
- 仓库门禁：`cargo fmt --check` 通过；`cargo check --workspace -j2` 通过，`target/maven-final-workspace-check.log`；`cargo test --workspace --exclude editor-app -j2 -- --test-threads=1` 通过，**543 项通过、396 项按仓库配置跳过**，`target/maven-final-workspace-tests-verified.log`。最初并发运行有既有滚动条动画用例的取值时序失败，单独复跑通过；串行检查又暴露本次能力升级遗漏的硬编码 1.7 断言，改为公开能力版本后完整检查通过。没有修改滚动条实现。
- 当时原生 GPUI 多模块场景仍在诊断，未计为通过：真实未保存 Library／Main 共享传输、预期诊断、二进制只读导航、POM 后撤销均已到达，随后刷新阶段无法完成调度。`target/maven-native-stages.log` 记录到“stale model observed; refreshing”；仅终止自己的失去进展的测试进程，再以测试内有界日志收集生命周期证据。未把用户进程或其他会话作为清理目标。该问题现已修复并通过最上方 0.1.18 原生场景复验。
- 手工审查覆盖了本轮宿主权限／代次／退休接缝与两个插件的模型、进程、类路径和失败资源路径。当前无用户固定的已提交审查基点，外部 Maven 源目录也尚未初始化 Git；没有为了满足基于提交区间的 code-review 流程而自行提交或声称已执行正式双轴审查。
- 当前限制：Windows x86-64；手动明确导入；未声明编译参数沿用 Java 默认配置，不推断任意编译插件默认值、Toolchain、自定义生命周期或 `.mvn/jvm.config`；Maven Wrapper／Maven 4 未验收。Java 原有 `import module` Tree-sitter 限制保持，不归为本轮已修复。

## 早期实施与失败记录

- 独立源码：`C:/Projects/RustProjects/Maven-for-Nanobug`。通过宿主 `--plugin-package` 构建，不引用宿主源码或附带 SDK 副本；尚未发布、提交或推送。
- `cargo build -p editor-app --release -j 2` 在清理后重建通过，日志 `target/maven-host-release-build.log`。构建用了 10 分 27 秒，存在既有警告。用于后续包构建的同摘要副本为 `target/maven-host/20261023-145100/editor-app.exe`，SHA-256 `D1AFD065DF7D6FC69A3AC31DC2927110B29F1A69FDAB239144880E1DBF20775B`；导出 SDK 身份 `4ae7d4f66625a6d4f04c8ec95160d35445ad85c08731dc97360568cd4dd7ad7b`。
- 实际源项目通过 Cargo metadata 核对 workspace 与 target。共享目录中的测试等待其他会话构建，本轮停止了自己的排队测试，改用独立 `target/maven-acceptance`；没有停止其他任务。该目录的运行时集成测试初编译通过，日志 `target/maven-acceptance-test-build.log`。
- 0.1.0 包的真实 Manager 安装先被现行原生可执行规则拒绝：`mvn.cmd` 不能成为受控服务入口，日志 `target/maven-import-red.log`。改为受控 Rust 原生入口直接启动 JVM，不执行 shell／批处理；0.1.1 包安装激活通过，原始功能测试明确失败于“导入尚未实现”，日志 `target/maven-import-feature-red.log`。
- 0.1.2 构建先暴露 Windows verbatim 路径混用分隔符，修正工具参数后打包通过。执行又发现错误地假设不同依赖缓存目录相邻。0.1.3 改为宿主解析声明式 `${dependency:maven}` 参数，通过公开 StartService、有界 stdin 与 CloseStdin 传递本次元数据，保留原生服务的依赖租约。
- 0.1.3 的单模块真实包用例通过，1 项、73.12 秒：中文路径、有效源码／输出、release 11、导入不生成用户 class；`target/maven-import-green-013.log`。包 SHA-256 `C8A205BE8CCD54BC8EAB748EFCCE8F705F3D2E17732AA3E1EF37C9D6E4589BBE`。
- 多模块用例先失败于缺少测试源码元数据，日志 `target/maven-reactor-red.log`。0.1.4 通过同一用例，1 项、55.65 秒，日志 `target/maven-reactor-green.log`：实际父 POM、profile 激活的两个子模块、继承源码、测试源码／输出、BOM 版本管理、reactor 身份和外部传递依赖均正确；用户模块未编译。依赖 JAR 在隔离夹具仓库内真实编译并解析，未读写用户 Maven 缓存。
- 已采用真实 Maven 自有 aggregator goal 导出模型；Bootstrap 只把本版本自有 helper 发布到 Maven 实际选定的本地仓库，校验已有同版本字节，保留 Maven 的工程求值及解析器。前期一次性探查记录为 `target/maven-model-probe.log` 和 `target/maven-goal-probe.log`，不将它们计为 Nanobug 插件验收。
- 0.1.4 的失败刷新用例先失败于缺少公开模型状态命令，日志 `target/maven-refresh-red.log`；0.1.5 通过同一用例，1 项、56.91 秒，日志 `target/maven-refresh-green.log`。旧模型及 generation 保留并标记 stale；快照不保存进程句柄，恢复模型须重新导入；迟到的旧导入不能覆盖较新意图。
- 正在验证 Java 组合。实际 0.1.5 首先失败于未声明 Maven 语言提供者，日志 `target/maven-java-red.log`。Java 仅扩展通用项目接入服务，不解释 POM。原生配置／任务、完整取消与退休、五代 JDK 和最终门禁尚未完成，没有把早期包称为完整交付。
- 0.1.6／Java 0.1.21 的模块配置和绑定借用检查已到达最后的退休断言，但用例仍失败，日志 `target/maven-java-green-016.log`；按原生宿主补上消费者登记后复查仍失败，日志 `target/maven-java-consumer-green.log`。暴露普通注册仅依赖提供者的通用接口缺口；正在扩展 `language.lsp 1.8` 的调用方作用域注册并重建内嵌 SDK，不将这两个用例计为通过。
- 首次 `cargo build -p editor-app --release -j2` 因主目录现有运行进程占用 EXE 而无法替换，日志 `target/maven-host-lsp18-build.log`；没有停止该进程。改为直接 Cargo 输出：`cargo rustc -p editor-app --release --bin editor-app -j2 -- -o C:/Projects/RustProjects/Editor/target/maven-host/20261023-lsp18/editor-app.exe`，4 分 29 秒通过，日志 `target/maven-host-lsp18-direct-build.log`。该副本 SHA-256 `437302BB3C2D2ADC8469030ECAE6812F9C7413A4187CD35484165B1AA6709951`，内嵌 SDK `3744d39bb611129ba2920164b01b5b9dd047520bcbe7cccf04fe572ec9084b30`。这是后续打包入口，未改动或关闭现有窗口。
- Java 0.1.22 的新能力准备先表现为超时：`target/maven-java-borrowed-green.log`、`target/maven-java-phase-green.log`、`target/maven-java-direct-green.log` 均失败。沿公开分离 worker 接口取得实际发布错误后，`target/maven-java-chain.log` 明确显示 `Dependency configuration requires language.lsp >=1.7,<2`。原因是旧门禁要求范围包含精确 1.7.0，误拒绝最低要求 1.8 的消费者；正在修正并复验，另记录选中上下文失败不结束等待的问题。定向诊断标记已从生产代码移除。
- 原生 Maven 目标先失败于没有配置贡献者，日志 `target/maven-goal-red.log`。0.1.7 已实际执行成功，但 ConPTY 未收到 Java 输出，日志 `target/maven-goal-green-017.log`，不能计为通过。0.1.8 只让管道服务使用 CREATE_NO_WINDOW，目标保留自己的 ConPTY；相同测试 1 项、78.24 秒通过，日志 `target/maven-goal-green-018.log`，验证原生表单准入、真实目标准备、带中文／空格／元字符的字面 Maven 属性、控制台 BUILD SUCCESS 及退出码。
- Maven 工程面板先失败于未声明原生入口，日志 `target/maven-panel-red.log`。0.1.9 的相同公开 Manager／原生 UI 事件用例 1 项、113.44 秒通过，日志 `target/maven-panel-green-019.log`，验证导入按钮更新公开模型及受检关闭；发现 POM／profile／离线输入的实际交互仍待补验。
- 修正版本门禁后，Maven 0.1.9／Java 0.1.22 的模块协作用例 1 项、188.38 秒通过，日志 `target/maven-java-floor-green.log`：应用与依赖源码、独立输出／JDK、禁用 JDT 内部 Maven 导入、封印绑定及停用 Maven 撤销借用上下文均通过。此用例只验证公开 Manager 的配置／绑定路径；实际 Java 语义、运行／调试与原生显示仍须继续验证。
- 无效 JDK 的真实组合用例在 `target/maven-context-error-red.log` 复现等待超时；保留捕获的注册上下文身份后，`target/maven-context-error-green.log` 1 项、139.29 秒通过。实际 `Unsupported project JDK` 错误结束 Java 原始等待并转发给 Maven；旧选择已撤销的结果不发布。
- 进一步的工程面板用例使用真实 POM 发现、中文／空格子目录选择、dev profile 与离线复选框；1 项、71.88 秒通过，`target/maven-panel-details.log`。尚未将公开 native-document 验证等同于完整 GPUI 可见交互验收。
- Java 语义用例 `target/maven-semantics.log` 发现外部 Maven JAR 无法解析。路径规范化（Java 0.1.23，`target/maven-semantics-paths.log`）和等待库更新任务（0.1.24，`target/maven-semantics-classpath.log`）均未解决，不能计为通过。保留真实后端的临时只读 REPL 将查询缩短到约 1 秒：实际传入 api-1.jar，但 `java.project.getSettings`／`getClasspaths` 显示源文件属于默认临时项目，只有该项目的 src/bin，没有已传入的库。临时 REPL 已退出，仍须清理源码中的诊断分支。
- 对照固定 [JDT LS 1.62.0 导入器](https://raw.githubusercontent.com/eclipse-jdtls/eclipse.jdt.ls/v1.62.0/org.eclipse.jdt.ls.core/src/org/eclipse/jdt/ls/core/internal/managers/InvisibleProjectImporter.java)，自动发现会排除已有构建文件的源码；正在由 Java 的通用显式项目入口建立私有分析项目，沿用已传入的源码和库，不解析 POM，也不恢复 JDT Maven 导入器。

## 组合与刷新复验

- Java 0.1.25 的通用显式分析项目通过真实语义用例：`target/maven-semantics-explicit.log`，1 项、132.20 秒；未保存中文源码、跨模块定义、实际二进制依赖内容、停用借用方后撤销及进程清理均通过。临时 REPL 和诊断分支已从源码移除；路径规范化和等待 classpath job 的失败假设随后撤回。
- 成功刷新后旧 Java 绑定仍有效，在 `target/maven-refresh-binding-red.log` 复现。新增通用发现代次和借用方代次守卫后，Maven 0.1.10／Java 0.1.25 通过相同用例：`target/maven-refresh-binding-green.log`，1 项、132.22 秒。输出目录变化后旧服务立即无效，原文档意图需要重新发现。
- 原生 GPUI 用例先在 `target/maven-native-app-red.log` 暴露最低版本 1.9 未进入按文档路由：旧判断只接受精确 1.5／1.6。改为依据公开当前能力版本，继续排除旧版范围后，`target/maven-native-app-context.log` 到达实际诊断，暴露“工作副本先属于默认项目”的依赖解析问题。直接协议用例通过不等于原生路径通过。
- Java 0.1.27 候选在初始化时通过 JDT 的通用 [importers 扩展点](https://raw.githubusercontent.com/eclipse-jdtls/eclipse.jdt.ls/v1.62.0/org.eclipse.jdt.ls.core/schema/org.eclipse.jdt.ls.core.importers.exsd) 建立显式项目，并等待 [Started 状态](https://raw.githubusercontent.com/eclipse-jdtls/eclipse.jdt.ls/v1.62.0/org.eclipse.jdt.ls.core/src/org/eclipse/jdt/ls/core/internal/handlers/InitHandler.java)。`target/maven-native-app-initialization.log` 仍因超时失败，正在诊断，未计为通过。
- Maven 应用模板缺失的真实用例在 `target/maven-application-red.log` 复现，1 项失败、100.88 秒。候选模板与 Java 通用启动上下文随后已实现；`target/maven-application-green.log` 暴露准备请求结束提前撤销上下文，`target/maven-application-lifetime.log` 仍超时。正在记录完整终止状态，未把这两个名称含 green／lifetime 的日志计为通过。
- Maven 0.1.12 的扩展面板用例通过：`target/maven-panel-watch-cancel.log`，1 项、76.72 秒。公开工作区 POM 事件撤销模型；立即取消下一次导入后保留原 generation、stale=true、importing=false，受检关闭后资源为零。实际原生 OS watcher 仍随 GPUI 场景补验。

## 开始时的事实（续）

- 本轮新增记忆约定后先按指定只读入口检索。首轮 request `6ba9ba89-bc3c-4026-a29d-c9b35e9f5fa6` 超时，状态 unavailable、无命中；随后自然中文检索 request `954f8dc6-0628-4ccd-8bea-63b790d11b30` 为 ok。采用的历史依据仅为用户已确认的独立插件边界：memory `616a2cd4-f889-4dc9-83a7-14fa1ed28ddf`、document `conversation:01a15f43-ff77-7b12-8b7f-b4ab24de42b6:turn:01a15f58-bb7a-7091-9afc-595becde9939`，以及 memory `0591a656-e15d-4f56-88bd-357addf30860`、document `conversation:01a15f43-ff77-7b12-8b7f-b4ab24de42b6:turn:01a15f5d-c8fa-74a1-b2f6-8a69f6ed6b80`。已与当前对话和方案核对；未把其他插件的早期进度当成本轮验收依据。

- 已重新读取项目需求基线、开发计划、插件平台与 Java 后续插件职责，检查当前工作区。开始时 9 项已有改动属于 Git 原生验收及文档，保留且不归入 Maven 成果。
- 未发现本机 `mvn`／`mvn.cmd`，已确认现有 Java、Javac、Jar 和 Cargo。
- 官方 Maven 3.10.0 ZIP 已下载并核对官方 SHA-512，再解包到 `C:/Projects/RustProjects/Editor/target/maven-toolchain/3.10.0/apache-maven-3.10.0`。SHA-256 为 `1F6D9909266510F039F59AA0E13DCD2C66DA85F043E56276E41F2918F8BDDAFF`；固定来源见[来源记录](../specs/maven-sources-2026-10-23.json)。没有修改全局 PATH。
- 静态核对了公开插件服务、语言委托、上下文注册、原生路径、受控进程与运行配置契约，以及 Java、Vue／TypeScript 的现有消费者。Maven 导入需真实工具执行，不能放进当前只读语言准备钩子。
- 未对主项目或插件执行编译、测试、打包或产物复制。初次只读 Java `MavenCli --version` 探查因 classpath 未包括 Maven boot/classworlds 返回类缺失；随后仅用 `javap` 查看公开类型，没有将其记为 Maven 工具运行验证通过。

## 构建暂停历史

清理会话传达了用户“清理完成后才编译”的明确要求。构建闸门 `C:/Users/Tmiracle/AppData/Local/Temp/nanobug-full-cleanup-build-hold-20261023.json` 曾为 `active=true`、`waiting_for_remaining_packaging_resolution`。用户随后明确保留剩余发行包并允许其他会话继续，14:34 起闸门为 `active=false`、`released`。本轮读取闸门核对后才恢复构建。

本轮没有运行中的构建需要终止；保留只读调查结果。闸门允许静态资料编辑，本轮只保存方案、来源和本记录；没有保存会触发 IDE 构建的 Rust／Java 源码。没有替清理会话修改闸门，也没有删除或重建已清理的构建产物。

## 恢复后的首轮

1. 先核对闸门已放行，以及宿主源码、SDK 缓存与输出目录的实际状态；不能继续引用已被清理的旧主程序副本。
2. 在独立 Maven 插件工程中，沿真实包与公开 Manager 接缝写首个导入用例，再实现最小可运行路径。模型取自实际 Maven 会话，不手工猜测 POM。
3. 验证单模块／多模块、继承、profile、传递依赖、取消及失败保留旧模型，再扩展 Java 组合与原生构建任务。
4. 完成实际包、原生交互、生命周期和仓库门禁后才标记相应阶段通过。

Java 0.1.20 的模块导入语法及审查覆盖限制仍见 [Java 验收记录](java-plugin-2026-10-22.md)，不因开始 Maven 而标为完成。本轮未创建正式议题、提交、推送或发布插件。
