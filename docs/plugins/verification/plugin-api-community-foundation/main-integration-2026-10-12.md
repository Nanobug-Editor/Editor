# 社区 API 分支合并主分支：2026-10-12

本记录只描述本次合并候选的验证，不把此前七张工单的历史测试当作当前合并的通过结果。普通合并和推送的最终提交以 Git 记录为准；主工作区未提交内容不进入合并提交。

## 合并输入与保留行为

- 原主工作区 `main`：`a07eafc848044092bbbfd55c108b0f81b69c13ef`。
- 合并时远程 `main`：`4d172b97cbbf7aff2bbcd986116321e3566023b9`，保留官方市场、显式安装、业务源码外置与无随附发行。
- 社区分支：`901c0907f9a0734d30506b3557f4603bdf4cab24`，保留七张工单已交付的公开文档、交互、事务、诊断、语言、树、装饰、云服务和生命周期能力。
- `Manifest` 同时保留替代声明和按需激活策略；冷服务发布检查平台、原作用域的配置与权限。替代事务持久提交后撤销旧可用性；失败恢复使用新实例身份。
- 原生文档导航先取消原目标的异步意图，再排空链接输入；生产插件发现继续只读注册表，不恢复随附插件或启动时自动安装。

社区独立消费者迁入 `crates/plugin-runtime/tests/fixtures/community/`。它们仅用于公开 SDK/Package/Manager 回归，不属于应用、安装器、SDK 或市场负载。各 Cargo 项目独立于宿主 workspace。原历史 ZIP 按版本和 SHA-256 固定，不能用新编译的 WASM 冒充旧 SDK 消费者。

## SDK 与实际构建输入

本候选宿主经 `--export-plugin-sdk` 导出 50 个构建文件，逐一与 `SDK_FILES` 内嵌字节核对，没有 Markdown。按宿主缓存规则计算的 SDK SHA-256 为 `b02e41e9b1a7f92e8e745885df2388baab68cb28f487a009d3505b363b6d730f`。

使用上述宿主的 `--plugin-package <独立项目> --output <绝对目录> --release` 实际构建以下消费者；每个构建成功后完整 ZIP 通过公共 Package 校验。三个项目声明自己的空 workspace，保证 Windows 扩展路径下仍独立构建。原同版本历史档案不替换，新档案明确使用 `-main-sdk` 名称。

| 合并后 SDK 消费者 | ZIP SHA-256 | 实际构建耗时 |
| --- | --- | ---: |
| `capability-example-0.19.1-main-sdk.zip` | `4c9c700bfff64d1b61f8e959f5495c3efa6c786e608450f8b229f7d0179293f2` | 54.04 秒 |
| `cloud-translation-0.1.2-main-sdk.zip` | `ea90c00bd23e166d1414080522ef43dde089040c279f24830e3a2a7e274916c8` | 47.61 秒 |
| `code-host-query-0.1.2-main-sdk.zip` | `8ec375e51f53811dcfa67b4edf565a935d2cd4fa0affc1dacea89e19444a7dc4` | 47.41 秒 |

所有 49 个固定输入通过 [固定账本](../../../../crates/plugin-runtime/tests/fixtures/packages/inputs.json) 选择实际版本和摘要，新输入单独记录原始源码树摘要、宿主 EXE 与 SDK 身份。历史构建元数据未记录的字段明确为 null，不从合并源码推断。对应消费者行为回归结果在下表按实际执行补充。

## 已执行验证

| 对象 | 实际结果 | 原始记录 |
| --- | --- | --- |
| `cargo fmt --check` | 最终通过（2.87 秒）；仅格式化本次冲突解决涉及的文件 | `main-integration-final-fmt.log` |
| `cargo check --workspace` | 最终通过（5.36 秒） | `main-integration-final-check.log` |
| `cargo test --workspace --exclude editor-app` | 最终 266 通过，0 失败，254 ignored（198.19 秒）；ignored 未计作通过 | `main-integration-final-test.log` |
| `cargo test -p editor-app --no-run` | 测试编译通过；不代表 UI 行为验收 | `main-integration-app-no-run.log` |
| 应用 `sdk_export::tests::` | 6 通过；覆盖完整性、修复、并发和保留调用者其他文件 | `main-integration-sdk-tests.log` |
| 应用 `extensions::worker::bundled_tests:: --include-ignored --test-threads=1` | 9 通过；显式安装、旧记录拒绝/迁移、提供者选择、撤销及原生恢复状态 | `main-integration-startup-tests.log` |
| Runtime 9 个实际包定向回归，逐例 `--exact --ignored --test-threads=1` | 9 通过，0 失败，0 ignored；平台/原项目配置门禁、替代成功/失败、取消依赖下载、新 SDK 热配置/Unicode HTTP，以及真实旧 SDK 准入/托管迁移 | `main-integration-runtime-results.json`、`main-integration-runtime-case1.log` 至 `case9.log` |
| 应用 5 个定向回归，逐例 `--exact --include-ignored --test-threads=1` | 5 通过，0 失败，0 ignored；注册表恢复、冷命令发现、精确文档菜单、虚拟文档原生输入拒绝、图片只读容器 | `main-integration-app-results.json`、`main-integration-app-case1.log` 至 `case5.log` |

原始日志保存在本地 Codex 工作树相邻的 `target01/`。应用测试 EXE SHA-256：`38197ffefdeeb9d3f530472f952766e777df7c4a0f1068dbdf0bd0eafcab2668`；SDK 导出宿主 SHA-256：`bd2a5d9ba930bd9a18ff0f2fc417e96d0f48e9b069b04e1e989416c7e6bb030f`。

真实 `service_activation --ignored` 首轮为 4 通过、1 失败，冷原生执行报告 `TimedOut / NotExecuted`；原始日志保留在 `main-integration-service-activation-tests.log`。保持同一源码、同一历史 ZIP、原始 30 秒产品期限和全部断言，以仅命令行的 `--config 'profile.dev.package.cranelift-codegen.opt-level=2'` 优化 WASM 编译器后，定向复核该失败项通过（1 通过、0 失败，实际行为 11.38 秒）；记录为 `main-integration-native-execution-optimized.log`。初次失败来自未优化编译器的冷编译耗时，不能把原失败日志伪记为通过；优化设置没有写入仓库或扩大产品期限。

## 独立审查

首轮本地临时审查对象：`273f633cb35ac83bc06cc32c1c2fb010cde6fb5d`，双父为上述远程主分支和社区提交。完整比较为 `git diff 4d172b97cbbf7aff2bbcd986116321e3566023b9...273f633cb35ac83bc06cc32c1c2fb010cde6fb5d`，另用社区父到快照的直接差异检查双方保留行为。临时对象只用于冻结审查输入，不属于推送历史；交付的普通合并提交保留双方原始父历史。

### Spec

首轮独立审查未发现新增规格缺失、范围蔓延或实现方式错误，三类发现均为 0。旧版运行文档遗漏已授权的 `dependencies 1.2` 首次方法准备说明，本次同步补齐。新增 SDK 包的固定增量快照 `2627b5e1105f28ca93a5a03550e9b43ffe37b699` 独立核对了 49 个 ZIP 摘要、46 个历史 ZIP 字节不变和三个新包清单身份；发现一项 P3 来源摘要排序说明不完整，已明确 Windows 路径的大小写无关排序规则，摘要和实际 ZIP 不变。修复快照 `8bf3c0fdbb9383c0b0e0cb4eb15510607c3e3d9f` 经独立复核，该项已解决，最终未解决 Spec 发现为 0。

### Standards

首轮及 `273f633 → 2627b5e` 增量独立审查均未发现硬性规范违规或需要修复的坏味道，发现数为 0。复核确认历史输入、默认选择器、SDK 构建字节、三个独立消费者来源和真实超时日志均保留。

## 原主工作区恢复

变更前逐字节备份 185 个路径及原 index/staged/unstaged 补丁，记录于本地 `main-integration-20261012/manifest-before.json`。恢复采用原 HEAD、原本地内容和新合并结果的三方比较，避免旧文件覆盖新 API。已经由后续提交吸收的旧内容保留原字节备份，不回退新实现；未被吸收的改动继续保留为未提交内容。原暂存范围仅包含截图删除。

三方提案覆盖全部 185 项且未解析冲突为 0：65 项已被新提交吸收，33 项仍独立的 tracked 修改，19 个需保留的 untracked 文件，55 项明确列出的文档删除及唯一暂存 PNG 删除，另 12 项删除已被新提交吸收。原数据、原 staged/unstaged 补丁与独立恢复审计继续保存在上述备份目录。恢复前重新核对原工作区和候选哈希；恢复后核对新 HEAD、原暂存范围及每个残余文件，不复制旧 index，不整目录回放旧源码。

推送使用普通 fast-forward 校验并读回 `origin/main` 完整 SHA；工作区恢复与交付实际读回保存在本地备份的最终审计记录。本次不修改父设计议题、已完成工单状态或发布版本。
