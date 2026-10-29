# Git 历史树重载、文档流与统一消息

日期：2026-10-29。状态：修复与行为验收通过，未发布或替换用户已安装插件。

## 范围与证据

用户报告历史文件树显示 `LimitExceeded: Document stream overflow; enumerate and resubscribe`，连续点击有时导致 Git 窗口重载，并要求错误、警告及日志统一进入宿主消息管理，警告和错误还在右下角浮窗显示。

在用户仍运行的 Git 0.1.50 插件管理日志中读取到 19:43:37 的 `wasm/event` 错误：`WASM execution budget exceeded`。栈从 `serde_json::ser::format_escaped_str` 经 `EditorSource`、`Kind`、`Node` 到 `Document` 的序列化；随后记录实例暂停及重新创建。这证明至少一次实际重载是访客执行预算耗尽，不是原生文件树控件直接崩溃。日志截断了长栈，不能据此再区分 fuel 与 epoch 中断。

对应代码中，Git 的全量 `views()` 即使详情与冲突窗口不可见，也构造带正文 Snapshot 的比较场景。历史比较正文同时由宿主原生比较视图持有，隐藏场景仍再次序列化两侧正文。无业务变化的 Active/Selection/Viewport 文档回执也会走全量场景生成，增加队列积压和执行成本。

旧版本的实际包回归在注入终止订阅回执后失败，精确发现 `history-status` 仍包含截图错误；此前仅检查 `history-error` 的测试遗漏了这个位置。该注入验证消息路由和恢复行为，不能单独证明自然文档流溢出的频率。

## 修复边界

- Git 的隐藏次级窗口发布轻量场景，窗口实际显示时再构造正文；原生历史比较继续使用公开只读文档来源。
- 无关文档回执不再生成场景，关闭、内容变化、订阅失败仍按资源归属处理。
- Git 的业务诊断使用通用 `ui.interaction 1.1` 的 `PublishMessage`，消息接纳后立即返回 `Published`。旧 `Notify` 仍等待关闭，避免把用户交互请求的寿命强加给日志发布。能力与权限声明随插件版本更新，不引入专属宿主 API。
- 宿主按记录身份、来源和工作区消费运行日志，统一列表与右下角浮窗的细节见[统一消息验收](../../ui/verification/unified-messages-2026-10-29.md)。

## 实际验证入口

独立插件使用宿主 `--plugin-cargo` 和 `--plugin-package`，产物输出到主项目 `dist/git/`。实际包原生测试设置 `NANOBUG_GIT_CANDIDATE` 后显式执行 ignored 用例：

```powershell
$env:NANOBUG_GIT_CANDIDATE = 'C:/Projects/RustProjects/Editor/dist/git/git-0.1.51.zip'
cargo test -p editor-app --bin editor-app extensions::windows_tests::history_preview::ownership:: -- --ignored --nocapture --test-threads=1
```

其中所有权测试覆盖终止订阅后再次选择文件；压力测试覆盖 32 个提交、12 个文件和连续 120 次树输入；大文件测试覆盖两份每侧 256 KiB 的合法文本与三次切换，同时满足 Git 自身的单 blob 上限和宿主只读文档上限。逐帧检查同一实例存活、无运行故障、最终正文正确，记录最大调用 fuel 与资源数量；隐藏面板不得复制非空正文 Snapshot。

最终候选为 `git-0.1.51.zip`，SHA-256：`5E683710E15B0FF4AE88E71F1969AECED7862FBF23EE59374C49C23CFDCFA3E2`。插件单测 252/252、WASM 目标检查、格式检查和打包通过。最终包的上述 3 项 ignored 原生测试均显式运行通过（376.50 秒）；本机直接调用对应 Cargo 生成的测试二进制，沿用同一过滤词、环境变量及参数。

宿主门禁通过：`cargo fmt --check`、`cargo test --workspace --exclude editor-app`（873 通过、446 ignored、0 失败）、`cargo check --workspace`。ignored 项不计入通过数；真实包检查独立执行。最后一次仅应用层的无归属错误补丁另以最新 20 项消息测试、10 项管理界面测试、应用构建及 workspace check 验证，插件原生测试所覆盖的 API 与场景代码未变。

最终构建在独立测试配置和仓库中实际打开 Git 窗口，交替点击历史文件显示原生差异；使用本任务创建的 `index.lock` 触发真实 Git 失败，确认统一历史仅新增一条错误、右下角仅出现一个可滚动浮窗，关闭浮窗保留历史、清空后不会回放。未使用用户项目制造失败，验收后移除测试锁并停止独立进程。

本机原始日志集中于 `C:/Users/Tmiracle/.codex/task-artifacts/git-events-messages-20261029/`，包括 `native-final-package.log`、`large-old-package.log`、`plugin-final-stash-tests.log`、`host-workspace-tests.log`、`host-messages-complete.log`；界面证据为 `native-final-single-error.png`。交付提交与本地合并身份记录在两个仓库各自的 `.git/task-worktree/tasks/git-events-messages-20261029-01a18782.json`，不作为项目运行依赖。

已执行的旧包对照：`git-0.1.50.zip` 的所有权用例因 `history-status` 的截图错误失败（82.33 秒）；120 次普通文件树输入通过。后者说明小文件点击本身不足以稳定复现预算故障，必须保留大正文用例与真实运行日志证据，不将普通压力测试误报为旧版崩溃复现。

旧包的大正文用例也按预期失败：隐藏详情面板仍发布 262144 字节 Snapshot（83.36 秒）。最初超过 Git 单 blob 上限的夹具不能作为缺陷复现，已改用上述 256 KiB 合法边界；最终旧包对照和新版绿灯均使用该边界。

返回[插件系统目录](../README.md)。
