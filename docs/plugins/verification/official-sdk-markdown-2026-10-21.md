# Markdown SDK A2/A3 与原生大纲验收

日期：2026-10-21 开始，2026-10-22 完成。状态：0.18.5 实现与交付验收完成；0.18.4 全部过程记录保留。只记录本会话实际证据；未提交、推送、发布或替换日常安装。

依据：[改造方案 A2/A3](../specs/official-plugin-sdk-adoption-2026-10-21.md)、[三会话范围](official-plugin-sdk-adoption-execution-2026-10-21.md)。用户追加“Markdown 大纲解析也要一起完善”，本记录包含这项追加范围。

## 输入与隔离

| 输入 | 实际身份 |
| --- | --- |
| 固定宿主 | `C:/Projects/RustProjects/Editor/target/plugin-developer-tooling/debug/editor-app.exe` |
| EXE SHA-256 | `c82565746b49011761884604da17e1bcbd30a45cb972b42a2a6ba20fda940856` |
| SDK SHA-256 | `8e2f359fedf819d8e8b6b3da1012722629e12d9731c02c38e5b01ccab8bd02e4` |
| SDK 使用 | 宿主 `--plugin-cargo` 自动映射受管缓存中的 `plugin-protocol 0.2.0`；插件与独立测试不指向 Editor/crates，不复制 SDK |
| 插件仓库 | `C:/Projects/RustProjects/Markdown-for-Nanobug`，开工 HEAD `985ea81ed762ea7a62f371ec4dfdf273cc9f0fe4`，已有大量 0.18.2 改动 |
| 保留的旧 ZIP | `Markdown-for-Nanobug/dist/markdown-0.18.2.zip`；SHA-256 `61a648070f6d659ffbadd9cf8a7f8da04110ba47d007a55f6ddcd28c93a31ca2` |
| 本批新版本 | `0.18.5`，wire 仍为 7；新增公开 `language.structure ^1` 声明，无新增权限 |
| 新 ZIP | `C:/Projects/RustProjects/Markdown-for-Nanobug/dist/markdown-0.18.5.zip`；SHA-256 `383a0d5be242eac1c47be54bf219f4c0ba9ce314beb94d5ede0924a39f0aafe0` |
| 插件完整源码摘要 | 156 个 tracked/非 ignored untracked 文件（含测试与文档，排除 dist/target/.git）；`2edf75779d2c68aced4a4c659aca9272d1ae310c9d8e496ece8d0a4492e5e81e`；0.18.4 原账本另保留 |
| 原生测试源码快照摘要 | 1946 个文件；`736355d9bdbcaf881edd78e71b00d864ce0925ddb74019552058269ed089fb19` |
| 原生测试 EXE 摘要 | `66e43ae88590f9a327190d70c2fc9ec007a6fbff307ba1c9eabe6dc0cd69bdd9`；与固定开发宿主 EXE 分别记账 |

证据根为 `C:/Projects/RustProjects/Editor/target/sdk-adoption-markdown-2026-10-21/`，下文日志均相对此目录。开工状态、二进制 diff 和原始相关源码保存在 `baseline/`；不能把既有 HTML、README 图片语义与 UI 改动算成本批工作。

主仓库开工源码冻结在 `source/`，Cargo 输出独立为 `cargo/`；先保存 `metadata.json` 核对 workspace_root/target_directory。仅把本批 Markdown 测试同步到该快照，不从并发 TypeScript/Git 会话增量拷贝 SDK。插件实际构建源码在 `plugin-source-path.txt` 指定的独立 Temp 目录，避免 Cargo 从 Editor 上溯到错误 workspace；受管构建输出及 metadata 单独保存。

逐文件 SHA-256 见 `plugin-source-ledger.json`、`native-source-ledger.json`，完整身份与结果索引见 `delivery-evidence.json`。所有归档文件已对照声明的源码/受管编译产物逐字节核对；`manifest.json` 由打包器规范化序列化，解析后的内容与源码完全相同。插件构建使用 `plugin-managed-metadata.json` 中的 Temp workspace 与 `MeEditor/plugin-build/141e635681bcf24f/windows-x86_64/release/wasm` 独立目录。

## A2 与大纲实现

- Formatting、Navigation 各自持有一个 `EditorIntent`；Pending 仅保留领域元数据。保持 30 秒、TryTerminate、选区读取→计划→单次替换、document/revision/UTF-8/expected_selection 和最终选择校验。
- 清理失败保留原 owner 和失败原因，阻止新分配；显式输入可重试。Prepare 独立清理两方，不短路或丢弃失败 owner；图片导入的副作用回执账本跨逻辑重置保留。取消不承诺回滚已完成编辑、打开或浏览器行为。
- 原生大纲通过公开 `language.structure` 纯 worker 返回不可变源码结构；Prepare 返回空 Output，解析不发布预览、不申请编辑资源。使用现有 CommonMark/GFM 解析器识别 ATX/Setext、真实层级、中文/重复/空标题、行内内容、章节范围和折叠范围。围栏/缩进代码与原始 HTML 不生成 Markdown 标题。
- 定义范围使用原始 UTF-8 字节，行内代码剥离反引号及规范化边缘空白。名称按 512 字节 UTF-8 边界截取；4096 标题和公开回复配额整体校验，超限明确失败。
- 大纲的 1 MiB 源码预算独立于预览 256 KiB。密集正文的纯解析预算测试曾复现 fuel 耗尽；优化只压缩无语法意义的同缩进正文续行，并保留原始坐标映射。全段预扫描保护 Setext 和引用语法，Unicode 空白不作为 CommonMark 空行。语法判定始终交给 CommonMark。
- 同类约 1 MiB 输入，密集短行原为 165,088,903 fuel，thin LTO 仍为 163,231,580，已撤回该构建试验。最终含末尾代码标题的短行/长段落分别为 87,138,814 / 69,606,296，低于未修改的 100,000,000 上限，原始 UTF-8 末尾标题定位保持正确。特殊高复杂度内容仍受宿主 fuel/deadline 保护，不承诺任意 1 MiB 输入都可无限计算。

## A3 独立试验与保留结论

保留产品 `src/scrolling.rs` 的原始实现。Temp `-a3` 副本实际接入 EditorIntent，维护 pending+latest、StopWaiting、2 秒、driver、导航抢占及清理失败阻塞；纯替换增加两个 owner/清理字段和跨 Prepare 的清理判断，源码 291→298 行（格式化前），未减少领域状态。

同一公开 WIT 输入分别运行旧 0.18.2、A3 实验包和新包：21 个源码视口位置只分配 2 次 LocateViewport，最终为 `b-550-paragraph`、origin 2、2000 ms；源码→预览→标题导航→源码的抢占序列共 4 次请求，晚到旧回执增加 0 次，最终保持正确段落。日志 `scroll-trace-old-full.log`、`scroll-trace-prototype-full.log`、`wire-final-outline.log`。这是行为等价证据，不是 CPU、绘制或吞吐性能提升结论。

## 已执行验证

实际命令均直接使用 Cargo 或上述固定宿主，不使用历史脚本。`tests/intent-host` 为独立 workspace，实际加载 ZIP 内 WASM，经公开 JSON/WIT 分发，不伪造 SDK 私有 owner。终端回执测试不冒充原生编辑成功。

| 检查 | 已观察结果 | 证据 |
| --- | --- | --- |
| 插件 `--plugin-cargo Cargo.toml test --lib` | 83 passed，0 ignored | `unit-final-0.18.5.log` |
| WASM WIT 行为回归 | 最终 ZIP 23 passed，0 ignored；所有普通调用使用生产 100M fuel，预算探针额外计量实际消耗 | `wire-final-0.18.5.log` |
| 原生热更新 | 最终 0.18.5 通过；PID/Editor Entity、未保存中文、更新前 Undo/Redo、一次格式 Undo、受管数据保留，失败准备保留旧实例，停用/移除后旧新资源全退役 | `native-final-0.18.5.log` |
| 原生大纲 | 最终 0.18.5 通过；中文结构、点击精确定位、未保存刷新、停用撤回，以及接近 1 MiB 的生产 StructureProvider（fuel/deadline） | `native-final-0.18.5.log` |
| 静态准入 | passed=true、executed=false；只证明静态检查，业务由原生/WIT 测试证明 | `plugin-check-final-0.18.5.json` |
| 最终隔离开发生命周期 | 0.18.5 的 admission/startup/activation/cleanup/status 均 passed，normal termination，checkpoint.partial=false；正常 stop 退出 0 | `dev-final-0.18.5-report.json` |
| workspace 非 editor-app 测试 | 451 passed，342 ignored；ignored 未当作通过 | `workspace-tests.log` |
| workspace 编译检查 | 独立快照 `cargo check --workspace -j 6` 通过，已有警告保留 | `workspace-check.log` |
| 格式 | 快照及共享主仓库 `cargo fmt --check` 通过；插件与独立 WIT workspace 分别检查 | `workspace-fmt-final.log`、`shared-workspace-fmt.log` |
| 通用 trap 兜底 | 固定 SDK 账本的 lifecycle-http/editor 0.1.2 真包，2 个 ignored 测试显式运行通过，含真实 WASM trap 和 owner 退役 | `runtime-lifecycle-trap-final.log` |

原生两项最终为 2 passed、0 ignored，180.19 秒。实际从冻结 `source/` 执行 `cargo test -p editor-app --no-run -j 6`，再运行产出的同一 EXE，避免对已运行 EXE 重新链接；环境分别显式指定实际新包、SHA-256 和未改变的旧包：

```powershell
$evidence = 'C:/Projects/RustProjects/Editor/target/sdk-adoption-markdown-2026-10-21'
$env:NANOBUG_MARKDOWN_SDK_PACKAGE = "$evidence/package-final-0.18.5/markdown-0.18.5.zip"
$env:NANOBUG_MARKDOWN_SDK_SHA256 = '383a0d5be242eac1c47be54bf219f4c0ba9ce314beb94d5ede0924a39f0aafe0'
$env:NANOBUG_MARKDOWN_SDK_OLD_PACKAGE = 'C:/Projects/RustProjects/Markdown-for-Nanobug/dist/markdown-0.18.2.zip'
& "$evidence/cargo/debug/deps/editor_app-646c88d348da6726.exe" 'extensions::markdown_tests::sdk_adoption::' --ignored --test-threads=1
```

首次完整 Markdown 原生筛选已执行 11 项通过，后主动终止，未宣称全部 87 项通过。它包含真实组合输入、任务、链接、IME、缩放、HTML 和首次滚动等场景，属于中间候选证据。最终包只按实际重跑项目记账。

## 隔离开发窗口与限制

固定 EXE 的 `--plugin-dev --release --profile ... --workspace ... --report ...` 使用独立目录。真实 UI 完成未保存中文输入、H2/一次 Undo、中文相对链接及原生大纲显示；错误 protocol 999 的 reload 明确失败且旧 PID 45916 和未保存文本继续保留，恢复后成功激活 0.18.4，旧 owner 资源归零。最终候选窗口还执行任务框写入与一次 Undo。

`dev-report.json` 包含故意拒绝输入，最终 admission/status 为 failed、cleanup passed；不能当全通过。第二个会话 `dev-clean-report.json` admission/startup/activation/cleanup 均 passed、normal termination，但 checkpoint.partial=true，因此 status=inconclusive。二者保留为实际过程证据，未改报通过。

0.18.5 的 `dev-final-0.18.5-report.json` 在新的独立 Temp profile 完整通过，checkpoint.partial=false、无 HTTP producer，正常 stop 退出 0。0.18.4 的 `dev-final-report.json` 也保留。固定的 business=not_exercised 按报告设计保留；业务通过依据上述真实原生测试与先前 UI 操作，不以生命周期报告冒充业务验收。诊断数据只统计请求、阶段和资源，不保存用户文本；未测 CPU/绘制开销。

收尾额外回归在 0.18.4 复现独立 CR 换行的章节/折叠错误，已同步按 CR/LF 计算边界；0.18.5 的公开真包测试验证 LF、CRLF、独立 CR。原 `dist/markdown-0.18.4.zip`（`76ee83cadeeac7ade066ee6c6f085e97db61788c968788166b78c933d75dbe07`）保留为拒绝/回归输入，未覆写。

独立 Spec/Standards 审查已覆盖开工 dirty baseline 后的本批变化；修复了纯 worker Prepare 发布 UI、行内代码反引号定位、跨行 Setext 前缀及 Unicode 空白分段问题。按用户限制未生成 commit，因此这不是提交范围的 commit review。
