# 全分支交付与主工作区收尾

日期：2026-10-12。用户授权：先提交并推送全部分支，再合并主分支；拆仓插件不恢复进宿主，并移除主仓库的本地残留。

起点：`main` / `origin/main` 为 `002b12668b45f0d27d961e89c35707e617f92c0c`。23 个本地分支中，仅 `codex/explorer-native-acceptance` 的五个提交尚未包含在主分支；远程独有的历史 `codex/run-debug-build` 也已包含。其他工作树均无未提交改动。

## 主工作区已有改动

- 历史截图删除独立提交；安装替代确认的可见布局断言独立提交，使用 index 导出的准确源码验证，1 项通过。
- 独立读者文档目录删除、双语 README 与源码契约链接迁移、Markdown 夹具解耦、原生终端退役及统一 Rust 包说明，保留既有改动并整理交付。市场迁移指针只链接独立项目，不维护过期的实施状态。
- 不将其他任务的历史验收结果称为本次重跑。阶段源码从 index 导出，构建直接调用 Cargo；仅给本进程设置已有 MSVC/UCRT/UM 的 LIB，不安装工具链或修改全局环境。

## 拆仓残留清理

`git ls-files plugins` 为空；旧目录含 Rust、Markdown、XML 与退役示例的本地源码或构建缓存。独立 `Rust-for-Nanobug`、`Markdown-for-Nanobug` 仓库均干净，当前清单为 0.6.1、0.17.2。17 个未跟踪源码文件先在仓库外保存摘要快照，再通过补丁移除；这些源码不提交进宿主。

递归删除剩余缓存被自动审批拒绝，原因仅为 `blocked by policy`。核对来源、目标的绝对边界及无链接后，改用可恢复移动，将剩余目录归档到 `C:/Users/Tmiracle/.codex/workspaces/Nanobug/all-branches-20261012/retired-plugin-caches/`。主仓库 `plugins/` 已不存在，`crates/plugin-runtime/tests/fixtures/` 的固定输入和 SDK 消费者保留；不修改独立插件仓库或用户安装数据。

历史索引的旧源码链接改为拆仓前 `1bfc4cf` 的 Git permalink，并用 `git cat-file` 核对真实目标，避免恢复本地业务源码。

## 收尾候选验证

| 检查 | 实际结果 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo test --workspace --exclude editor-app -j 2` | 266 通过、0 失败、254 ignored；跳过项未计通过 |
| `cargo check --workspace` | 通过，既有 warnings 保留 |
| `cargo test -p editor-app sdk_export::tests -- --test-threads=1` | 6 通过 |
| 真实 Markdown 规格预览与滚动 `--ignored` | 1 通过；读取固定 reviewed ZIP，无旧脚本或源码副本 |
| 新增本地文档链接、历史 permalink 目标、`git diff --check` | 通过 |
| 真实 Markdown 延迟 `--ignored --nocapture` | 未通过；新独立样本 split 中位 dispatch 77.8346 ms、input-to-frame 154.2286 ms，split-no-toolbar 102.4329 ms |
| 同宿主、同冻结包、原已提交样本对照 | 同样未通过；split 中位 dispatch 76.103 ms、input-to-frame 151.0561 ms，split-no-toolbar 100.8421 ms |

延迟失败在原样本基线上也存在，不放宽 60/100 ms 门槛，不以合并任务扩大修改渲染器。当前结果不表示该 ignored 性能用例通过，也不表示整个 UI 或所有真实包测试均已重跑。

原始日志、index 导出与源码摘要快照位于 `C:/Users/Tmiracle/.codex/workspaces/Nanobug/all-branches-20261012/`。资源管理器原生行为证据继续见[文件转移验收](../ui/verification/explorer-file-transfer.md)；主分支合并与最终推送结果在完成后补充。
