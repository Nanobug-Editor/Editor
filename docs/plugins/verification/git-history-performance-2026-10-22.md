# Git 大提交列表的滚动与加载开销

日期：2026-10-22。状态：本批性能、实际包及 Windows 原生行为验收通过，0.1.26 候选已构建；全仓门禁的范围外失败见下文。范围：独立 Git 插件的提交记录绘制、分页准入和后台观察；不改宿主生产 API，不安装到用户配置、不重启用户程序、不提交或发布。与此前[分支快速筛选](git-branch-fast-switch-2026-10-21.md)、[工作区呈现](workspace-presentation-2026-10-22.md)共同使用。

## 可复现的问题

原实现已经只输出可见行，但 `HistoryDrawing::synchronize` 每次滚动都比较全部提交的字符串；每追加一页又复制全部记录并重算完整分支线。分页去重也重新分配整份 OID 集合。后台观察对未变化的大列表逐页重读，已加载 32,768 行时最多重复 256 页。

先加入真实 `history_page` → `history_scroll_view` 接缝上的分配计数测试，再修改生产实现。测试分配器仅在当前测试线程的计量区间统计请求字节，生产 WASM 不包含该分配器；初始数据和模型容量准备不计入追加页的重复开销。固定 1,000×280 逻辑像素视口，32,768 条完整记录，追加 128 条，再执行 128 个 7px 滚动事件与场景序列化。

```powershell
$env:CARGO_TARGET_DIR = 'C:/Projects/RustProjects/Editor/target/git-history-performance-2026-10-22/guest-build'
& target/workspace-presentation-2026-10-22/Nanobug.exe --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib large_history_page_and_scroll_work_stays_bounded -- --nocapture
```

| 同机 debug 诊断样本 | 修复前 | 修复后首次回归 |
| --- | ---: | ---: |
| 追加 128 行请求的临时分配字节 | 31,315,741 | 138,053 |
| 其中绘制阶段的分配字节 | 28,326,011 | 62,627 |
| 128 次滚动及序列化 | 625.27 ms | 65.78 ms |
| 256 行小列表的同一滚动循环 | 49.30 ms | 49.10 ms |

这是测试进程中的分配流量和处理时间，不是应用常驻内存、显示器 FPS 或完整端到端延迟。分配上限断言确定性失败后转绿；耗时作为对比样本记录，不用易抖动的毫秒阈值代替正确性测试。原始日志为 `target/git-history-performance-2026-10-22/performance-{red,probe,green}.log`。后台刷新测试 `unchanged_background_history_reads_only_one_page` 也先失败于继续请求剩余 255 页，见 `refresh-red.log`。

## 修复

- 提交模型保留一份完整元数据；派生绘制缓存只保存分块几何及尚未遇到的父提交。128 行一块，分页到来时延续分支线状态，不移动或重算全部旧几何。
- 记录替换由准入处显式使绘制缓存失效；普通滚动只检查长度和失效标记。相同长度的内容替换仍更新 scroll epoch，旧输入继续被拒绝。
- OID 索引随新页增量维护；完整替换重新建立索引。重叠页不产生重复可选行。完整旧 SVG 块保留，未填满的尾块在追加后更新；视口之外的 SVG 缓存仍按原预算撤销。
- 后台观察先处理实际 ref/HEAD 探测；若缓存归属仍有效且第一完整页一致，保留已加载范围、滚动、选择及 epoch，结束本轮历史读取。引用变化、记录变化或缩短时按原逻辑刷新。手动刷新继续复查全部已加载范围，保留对浅仓库边界等外部变化的明确重新读取入口。

实现位于 `C:/Projects/RustProjects/Git-for-Nanobug/src/{history.rs,history/drawing.rs,graph.rs}`。新增回归覆盖分块边界上的 merge/共享父节点、追加尾块、重叠页、同长度替换、过期输入，以及后台刷新不能复用的条件。宿主仓库只新增真实包测试，未添加 Git 专用宿主 API。

## 验证与产物

- 独立插件全量 `--plugin-cargo … test --lib`：**143 passed，0 failed，0 ignored**，`guest-final.log`。
- Git 源码格式检查及 `git diff --check`：通过。
- Windows 原生 `git_native_history_scroll_probe --exact --ignored --nocapture --test-threads=1`：**1 父测试、2 个真实窗口场景通过**，26.24 秒。深色中文 100% 与浅色英文 150% 的连续滚轮、边界反向、滚动条拖动、选中项保留及 SVG 像素缓存复用通过。32 个原生输入累积 160px，发布样本分别 39ms、24ms，原图块均有复用，正常关闭且无意外错误日志。
- 原生使用已验证的配套宿主 `target/workspace-presentation-2026-10-22/native-test-verified.exe` 加本次实际 ZIP。本批宿主生产逻辑未改，不将它记为整个当前共享工作区 UI 的重新构建验收。证据在 `target/git-history-performance-2026-10-22/native/{dark-zh-CN-100,light-en-150}/`，包含 `evidence.json` 和 D3D 截图；未覆盖系统输入法候选窗或其他平台。
- 真实 8,193 条提交、65 页的 `git_package_large_history_keeps_scroll_scenes_bounded`：**通过**，21.64 秒。128 个排队滚动事件保留全部 896px；每帧少于 80 个绘制操作、整个历史场景小于 64 KiB；末尾记录可见，末尾首个可见行点击取得与真实 Git 相同的 OID；停用后资源归零。见 `large-package.log`。
- 实际包的分页/焦点/保存/新增提交预览回归及滚动 burst/旧点击拒绝回归：**2 项通过**，分别 11.18 秒、6.19 秒，见 `preview-package.log`、`scroll-package.log`。三项均设置最终 ZIP 的 `NANOBUG_GIT_CANDIDATE`，以新编译的 `git_plugin-9e4196f2c0808ef0.exe` 显式执行 `--ignored`。
- `cargo check --workspace --jobs 2`：**通过**，15.02 秒，保留既有告警，见 `workspace-check.log`。
- `cargo fmt --check`：**未通过**。差异来自并行的本地历史实现及关联设置、测试文件，本批插件与新增真实包测试没有格式差异；见 `workspace-format.log`。
- `cargo test --workspace --exclude editor-app --jobs 2`：**未通过**。停止前 20 个测试组共 **399 passed、1 failed、18 ignored**；失败为 `capabilities::terminal_tests::process_terminal_presentation_is_negotiated_without_an_identity_exception`，`crates/plugin-runtime/src/capabilities.rs` 当前公布 `process 1.9.0`，断言仍期待 `1.8.0`。该文件在开始时已修改，本批未改动；未替其他任务变更能力版本或断言，后续未执行的组不计为通过。详见 `workspace-tests-final.log`。

首次真实包测试编译发现新子模块需要显式相对 `#[path]`，已经修正。该问题使首次非 app workspace 测试停在编译阶段，不将这轮失败计作通过；最终重跑使用 `runtime-build-final.log` 与 `workspace-tests-final.log`。

Git 清单、Cargo 版本与构建锁文件为 **0.1.26**，协议/能力/权限沿用 0.1.25。包由现有宿主 `--plugin-package` 独立构建：

`target/git-history-performance-2026-10-22/packages/git-0.1.26.zip`

SHA-256：`046cde37754042f17ed3538ba8d90e5cea3376ae6d903aa353565356c372b3e0`。

本批开始时独立源码备份为 `target/git-history-performance-2026-10-22/guest-before.zip`，修改涉及的独立源码摘要为 `guest-source-inputs.json`；宿主共享工作区已有 607 项状态记录，保存在 `workspace-before.txt`。其中并行的本地历史实现不属于本次成果，也未替该任务格式化。原生截图已经检查，最终 ZIP 摘要再次核对一致；不声明整个共享工作区所有门禁通过。
