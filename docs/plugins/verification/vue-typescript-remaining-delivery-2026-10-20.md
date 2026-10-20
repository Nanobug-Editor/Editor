# Vue / TypeScript 剩余场景交付

日期：2026-10-20，Asia/Shanghai。新 #122–#126 的实现、实际包、Windows 交互与技术验收已完成；普通提交、推送及 issue 状态读回尚待本页末尾的实际回执。#121 既有交付保留，旧 #108–#118 和父 #107 不执行、不改写、不关闭；不创建 Release。

完整过程和旧失败分别保存在[实施记录](vue-typescript-remaining-2026-10-19.md)、[暂停检查点](vue-typescript-paused-2026-10-20-0508.md)及[恢复记录](vue-typescript-resumed-2026-10-20.md)。逐条对应关系见[覆盖映射](vue-typescript-remaining-coverage-2026-10-19.md)，独立结论见[双轴审查](vue-typescript-remaining-review-2026-10-20.md)，实际输入和可见结果见[普通桌面记录](vue-typescript-desktop-2026-10-20.md)。历史时点的 pending、失败和暂停描述不覆盖本页最终证据。

## 交付范围

一个 Vue 插件保留多个不可变兼容后端，TypeScript 插件统一管理项目与共享源码上下文，通过公开契约与 Vue 和专门生态插件协作。宿主提供通用上下文选择、脏文档同步、映射编辑、分组撤销、依赖准备和恢复机制；没有新增按 Vue、TypeScript 或具体插件 ID 分支的业务路由。

首批正式范围限于 **Windows x86-64、TypeScript 5.9.3、Node 24.19.0、Vue 2.6.14 / 2.7.16 / 3.5.22 及兼容账本中 26 个固定写法组合**。包括混合仓库、适用的 Options / Composition / script setup、JS/TS、类组件／装饰器、JSX/TSX、合法静态外部 JS/TS src、CSS/SCSS/Sass/Less 高亮与格式化。Vue 2.6 Composition API、类组件及 TSX 扩展按已锁依赖分别验收，不从样例外推全部动态语义或整个 semver 区间。

Vue 2.5 及更早、1.x/0.x、其他未验收补丁、未来版本和 HTML/CDN 继续按长期计划补齐；现有历史后端长期保留。Pug、Stylus、Nuxt 专属语义由专门插件承担。动态工厂、继承／计算装饰器元数据、外部样式表、非 Windows 平台和实际 UNC 执行没有标为正式通过。未知／冲突证据明确提示，不静默套用最新后端。

## 场景验收

| 场景 | 实际完成的关键行为 | 主要证据 |
| --- | --- | --- |
| S2 / #122 | 共享脏文本、来源 F12 和本机歧义偏好、补全 resolve 与原子自动导入、跨文件预览／取消／提交／整组 Undo；过期结果全拒绝 | 覆盖映射；普通桌面真实键盘和选择；最终双包 mixed 回归 |
| S3 / #123 | 26 类固定写法、八行历史事务、两种外部脚本、12 行 whole/range 样式格式化、插件 grammar 和块注入 | source/sealed 各 102 PASS；历史原生 144.24s、external 104.93s；格式化与桌面分层记录 |
| S4 / #124 | 独立四角色生态贡献实际影响映射与事务；替换／卸载撤销；四类冲突拒绝与移除恢复 | 四角色 152.95s；冲突 280.28s；[专项记录](vue-typescript-s3-s4-native-acceptance-2026-10-20.md) |
| S5 / #125 | 候选准备失败保持旧组合、激活回滚、缓存／离线／租约、真实内层与外层进程退出后有限恢复；其他上下文保持 | 内层 122.54s；外层更新／EOF 全链 164.27s；Worker 撤权 67.94s、更新 67.84s |
| S6 / #126 | 当前 SDK 公开构建、最终包精确联合运行、独立双轴审查、原生输入和最终仓库门禁 | 下列产物与命令；三仓 Git 回执单独记账 |

普通窗口实际验收包括编辑与诊断消长、补全选择和自动导入、悬浮、F12、引用、单文件及跨文件重命名预览／取消／提交／整组撤销、SFC 格式化、混合上下文选择与重开保持。中文 IME 使用实际系统候选窗完成“你”的提交、第二次组合取消及 Undo；并观察滚动、字号缩放和深浅主题。GPUI TestAppContext、WindowsPlatform 自动化、工具语义及普通桌面的证据分别记录，不互相冒充。

暖缓存悬浮重入、暗色卡片及行内代码颜色、Escape 后显式键盘重开／迟到结果三个通用缺陷均有实际 RED→GREEN 与独立增量审查。最终普通窗口在同一光标、同一鼠标位置完成 Ctrl+I → Escape → Ctrl+I；并非直接调用 handler 或写入测试状态。

## 最终产物与构建身份

以下路径中的 H 为当前 Editor 集成工作树，T 为 `C:/Projects/RustProjects/TypeScript-for-Nanobug`，V 为 `C:/Projects/RustProjects/Vue-for-Nanobug`。普通宿主和测试程序是不同构建入口，不混用身份。

| 产物 | 位置 | SHA-256 |
| --- | --- | --- |
| TypeScript 0.1.15 Package | `T/target/s6-supported-015-package/typescript-0.1.15.zip` | `beab4419c7004ce96d4bf2ce3f4a826f0c8f2a777a9a4b0c3896496f314c038b` |
| Vue 0.1.12 Package | `V/target/s6-supported-012-package/vue-0.1.12.zip` | `5f6a14c2e6b4aa139e33bcdbb3711d56ac03620b784f2dd86cda39810b7e1b73` |
| 最终普通宿主 | `H/target/s6-final-supported-native-host/editor-app.exe` | `1cae48c982197430fbbe0daf8c0664780d0ce5a1c30999c836d2f1c71ac89f61` |
| 最终 app 测试程序 | `H/target/s6-hover-reopen-complete/deps/editor_app.exe` | `beed53fdea083781c2a724e7185308430f2cbc8d44e3ae3fbb101d438ede40f1` |
| 最终 policy 测试程序 | `H/target/s6-final-policy/deps/vue_compatibility.exe` | `137fbed8191edd2b825bb3ff508647c3ea0459bf4a5cdff00d91bafb5a0c42a3` |
| 内嵌 SDK 内容身份 | `H/target/s6-final-supported-sdk-export` | `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb` |

实际普通宿主公开 `--plugin-package` 构建两包均 exit0，release WASM 分别 1m07s / 1m09s。冻结源码 29 / 44 文件分别使用独立 managed target `a3b7cc99a8bede30` / `4a6e736cc250abe7`；构建前 metadata 核对 root/target，构建后源码及 Cargo.lock 未变。没有复制 SDK、添加指向宿主 crates 的依赖、调用历史 scripts 或升级上游依赖。

公开 SDK 导出为 **54 文件、0 Markdown、与既有 4af 缓存 0 字节差异**。输入和实际命令分别保存在 `target/s6-final-supported-source-inputs.json`、`s6-final-{typescript,vue}-metadata.json`、`s6-final-{typescript,vue}-package.log` 和 `s6-final-supported-sdk-proof.json`。

## 最终运行与门禁

| 实际命令／入口 | 实际结果 | 原始记录（H/target/ 下） |
| --- | --- | --- |
| `cargo fmt --check` | PASS，exit0 | `s6-final-supported-fmt.log` |
| `cargo test --workspace --exclude editor-app -j1` | 319 PASS / 0 FAIL / 327 ignored，85 summaries | `s6-final-supported-non-ui.log` |
| `cargo check --workspace -j1` | PASS，2.82s，exit0 | `s6-final-supported-check.log` |
| 最终 app 程序精确运行 `extensions::worker::vue_typescript_tests::independent_vue_typescript_packages_analyze_native_mixed_project_tabs --exact --ignored --nocapture --test-threads=1` | **1 PASS / 0 FAIL / 0 ignored，97.62s**；三个真实 Vue 项目，暖缓存 Ctrl+I、真实语义／引用／脏文本和 checked shutdown | `s6-final-pair-native.log`、`s6-final-pair-inputs.json` |
| 最终 policy 程序精确运行 `real_vue_package_reports_its_selected_versions_without_attaching_native_modules --exact --ignored --nocapture --test-threads=1` | **1 PASS / 0 FAIL / 0 ignored，179.74s**；实际新 Vue 包与受控 TS 元数据提供者，不代替真实编译器测试 | `s6-final-policy.log`、`s6-final-policy-inputs.json` |
| V 中 `node --test --test-concurrency=1 tests/versions.test.cjs`，Node 24.19.0 | **5 PASS / 0 FAIL / 0 skipped，1345.2615ms** | `s6-final-vue-versions.log`、`s6-final-versions-node-version.log` |
| 最终 app hover／keyboard、卡片、文字样式、Escape 过滤 | 7 / 6 / 2 / 9 PASS；Escape 另 1 ignored；组间重叠不相加 | `s6-hover-reopen-complete{,-cards,-text,-escape}.log` |

全部 ignored 没有宣称一并通过；涉及本轮真实包的专项按记录显式运行。`editor::language_features` 曾匹配 0 tests，不计覆盖。最终 `cargo tree --workspace --prefix none` 无 gpui-component 依赖；源码仅保留禁止该依赖的测试断言，无实际调用。已通过的大型历史／生态／恢复矩阵继承的是逐字节不变负载和对应阶段程序，未伪称全部在最终 ZIP 上重跑；最终新访客组件和支持标签另由上述精确双包与 policy 运行验收。

## 归档闭包与审查

最终归档独立流式实读 T 22 / V 39 条目，静态闭包 **0 项意外差异**。29 / 44 冻结源码在打包核验时与 checkout 完全相等；原 47 项资源、11 项 Vue 封印模块、TypeScript native .14 整包和许可不变。公开 packager 重序列化 manifest 原始字节，但保留数组顺序的深层 JSON 比较完全相同；与历史 manifest 仅顶层插件版本不同。重新编译的两个 WASM component 与各自 managed target 输出逐字节相等，不冒称旧二进制身份。

完整静态账本为 `target/s6-final-supported-packages.json`，SHA-256 `9f956d536d0ce054a600f929a379afc78677305f664216e6087e178af4e4697f`；同名 `.md` 提供全部条目摘要。旧候选两包及其快照、历史 .14/.11 两包均独立保留且摘要不变。TypeScript 插件 .15 与 native .14、Vue 插件 .12 与 adapters .11 分别版本化；`records[].status=candidate` 是协议准入决策，正式支持由独立 `support`／`acceptance` 精确范围标记。

打包后最后的注释门禁仅在 H `editor/language_edits/completion.rs` 和 T `src/selection.rs` 的 `#[cfg(test)] mod tests` 内补一行模块说明和空行，没有改行为。T 最终 checkout 的 29 个归档输入中，selection.rs 相对原冻结源只有这项测试注释差异，不能再声称当前源码每字节全等或该注释已经编进旧产物。原冻结源、包及静态账本保持原字节，独立补充核对见 `target/s6-final-comment-only-postlude.{json,md}`。H 再次 `cargo fmt --check`、T `rustfmt --check --edition 2024 src/selection.rs` 及 diff 检查均通过；此注释增量没有重新运行完整构建或语义矩阵，沿用前述实际行为证据。

Standards 与 Spec 对实现、发现修复、缓存／颜色／键盘增量以及最终九项支持元数据分别独立审查，最终均无未解决的可行动发现。最终支持元数据只改变 T 四 / V 五文件，历史 candidateVerification／retainedArtifacts 原始块 6824 字节未变。静态审查、归档闭包与运行结果独立记证；完整发现和解决依据保存在[审查记录](vue-typescript-remaining-review-2026-10-20.md)。

## 保留数据与已知限制

用户原工作区 `C:/Projects/RustProjects/Editor` 没有用于实现、暂存或提交。当前原工作区 22 个脏路径保留；Drawing Excalidraw 的已知原始 SHA `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6` 与 S1 已提交记录相同。05:08 暂停文档中的同一字符串缺一个 `a`，是记录字面量错误，不是文件变化；其余 21 路径缺少初始逐文件字节基线，不能声称已独立证明完全不变。实际原项目 27 份 fixture 磁盘未变，临时六文件逐摘要核对并删除，临时根已不存在。当前私有验收进程已结束；没有用静态进程快照外推历史未知孤儿进程不存在。

普通窗口点击退出仍出现仓库既有 **AccessKit 0.34.0 `ClientToScreen` 无效 HWND / COM abort**。最终记录 `target/s6-final-supported-desktop-stderr.log`；功能与可见输入通过不等于普通退出 exit0。此前基线见[项目窗口记录](../../ui/verification/project-home.md)和[本地控件记录](../../ui/verification/base-only-components-2026-10-19.md)。本轮未升级或 fork GPUI／AccessKit，未将这一平台限制隐去，也不与语言服务 checked shutdown 的专门回归混同。

## Git 与工单回执

固定比较点：Editor `fc9dc531ef991315d73e21c8a092f12a0f1ff78b`、TypeScript `873e8c3d3ebe3a86a8bb1e028a3c4bcd1c557b94`、Vue `f8e5880bf89161f16723ae7be1c76458d50a676c`。普通提交只纳入本任务变更；用户主工作区没有用于暂存或提交。

19:02 两个独立插件普通提交后，已依次实际 `git push origin HEAD:main` 成功，并用 `git ls-remote --heads origin main` 核对下表完整 SHA；两仓工作区均干净。宿主和 issue 回执仍待实际完成，不提前勾选 S6-AC7。

| 仓库 | 普通提交 | 已读回 origin/main |
| --- | --- | --- |
| TypeScript | `2bc5be0301fc485d3b684641104e4dd596d3456e` | 同一 SHA，已核对 |
| Vue | `173c265e4c19298ca2fdb86b849ae90b6e3e4043` | 同一 SHA，已核对 |
| Editor | 待本轮普通提交 | 待推送核对 |

最终提交范围 Standards 及交付文档 Spec / Standards 均 **0 项未解决发现**；原索引状态 P2、两处模块注释项保留发现并记录修复。报告分别为 `target/s6-final-git-scope-standards-review.md`（SHA `4b4143cee8981fe763e0ac1d9edfb1360049f1b293ba6eefdbCDD67f285880f6`）、`s6-final-delivery-docs-spec-review.md`（`2a6a33835dfccb62385671295af3062784fa0ce185c8461db85215c347e37253`）、`s6-final-delivery-docs-standards-review.md`（`2d1a81ef08f985197fbb2dfab98be334327ccacef1ce4f335f6e31eb57c4045c`）。注释独立补录 JSON SHA `6c00b0e26da950a03afd97373858d1cf4353e30d1ebcc7901c6ac1cbb0cae5f5`。此后只追加真实 Git 状态，不用审查预先认证未来推送。

推送前已实际读回 #122–#126 为 open、#121 为此前 closed；旧 #108–#118 和父 #107 均为 open，正文与状态保存为对照。后续先核对宿主远程 SHA，再读回新工单；#121 沿用其[已交付证据](vue-typescript-s1-delivery-2026-10-19.md)，旧工单与父议题保持原状。
