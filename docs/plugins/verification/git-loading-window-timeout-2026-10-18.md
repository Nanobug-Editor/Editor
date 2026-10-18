# Git 加载与工作窗口超时回归（2026-10-18）

状态：加载超时修复、定向回归与工作区检查通过，本地修复版已生成；用户报告的整窗卡死仍未独立复现。本记录不重新打开或继承 #119、#120 的关闭状态。

## 输入与边界

- 用户截图打开 `C:/Projects/RustProjects/Nanobug-Git-Lab`，它是场景容器，没有 `.git`。正常测试入口是其 `workbench` 子目录；其他工作树分别作为项目打开。
- 只读 Git 核验确认 `workbench` 的仓库根目录、最新三条提交有效；未重置、合并、暂存或修改用户 Lab 的任何分支、冲突与脏文件。
- 安装中的独立 Git 插件为 0.1.7，ZIP SHA-256 为 `8034109e64266b12e17d70fcb7c513de81d1b2130b690baa117ae44450135d71`，与最终交付包一致；52 项访客源码账本亦匹配。此次修复位于所有插件共用的宿主机制，不改变插件包或协议。
- 截图的“重试: timeout”来自树分页错误行。它不能由“打开了非仓库目录”解释为正常行为；非仓库也应完成空页并显示可操作的说明。

## RED 与因果验证

实际 Package → Manager 的普通加载检查先通过：非 Git 目录的分支、暂存、详情文件树均完成空页；独立正常仓库加载三条真实提交、脏文件和精确详情 OID。这个接缝没有复现原生投递时序。

随后用真实 WindowsPlatform、生产 Worker、相同 0.1.7 包、临时工作区/配置和恢复可见的详情窗口复现。没有使用用户配置或修改其运行进程。

| 运行 | 实际结果 | 日志（仓库 target/ 下） |
| --- | --- | --- |
| 三面板恢复，首次 | RED，60.68 秒，详情根页 timeout / StaleRevision | `git-nonrepo-native-first-2026-10-18.log` |
| 同输入重复 | RED，60.67 秒，同症状 | `git-nonrepo-native-repeat-2026-10-18.log` |
| 隐藏历史与暂存，仅恢复详情 | RED，60.57 秒，同症状；真实重试行点击及后续计时器返回，关闭成功 | `git-nonrepo-native-minimal-first-2026-10-18.log` |
| 公开 Manager 精确回归 | RED，46.07 秒：已返回 StaleRevision，但对应 loading 仍为 true | `git120-project-loading-stale-page-red-2026-10-18.log` |

定向同步日志扰动使两轮原生探针转绿，记录于 `git-nonrepo-native-probe-2026-10-18.log` 和 `git-nonrepo-native-probe-full-2026-10-18.log`；不将其计为修复证据。临时 `[DEBUG-git-load]` 插桩已移除。

确定性回归通过真实 `PanelVisibility` 发布推进 scene revision，证明同一树句柄、同一 epoch 下，旧页因界面版本过期被拒绝。`TreeResource.request` 已登记 pending；Manager 在进入 WASM 前拒绝，Worker 仅将错误记为良性的旧输入，旧 pending 却一直等待 15 秒，最终显示虚假的提供者 timeout。复现耗时还包含 debug WASM 冷编译，不能作为用户加载性能指标。

## 修复

- `Manager::event` 继续返回原 `StaleRevision`，只对被拒绝的 `Tree::Request` 释放未投递分页。
- Instance 先通过 `owned_tree` 验证完整实例、scope、资源类型及存活状态。
- TreeResource 只移除完整 Request 相等的 pending，清除该分支 loading 并推进资源 revision；保留 epoch、缓存、cursor、兄弟请求和展开状态，不制造错误页。
- 当前原生界面的既有门禁重新申请未加载页；没有把旧事件改绑新界面，也不重放 Selected、Invoke 或其他业务操作。
- 新序号 pending 不会被重复的旧请求清除；正常提供者错误和真实超时仍使用原错误/手动重试路径。

没有新增公开能力、WIT、SDK 或 Git 专属宿主分支，没有复制或 fork gpui-kit/base 源码。

## 验证记录

使用本任务既有源码快照 `target/git-ticket119-source` 和独立冻结 runner，避免工作区其他任务改动或 Windows 可执行锁影响本次证据。以下均为实际执行结果；忽略的用例不计为通过。

| 检查 | 结果 |
| --- | --- |
| Package/Manager 回归，含旧请求隔离及正常仓库加载 | GREEN，94.06 秒，1 passed；`git-stale-page-green-2026-10-18.log` |
| 无插桩 Windows 最小详情场景 | GREEN，47.32 秒，1 passed；`git-stale-page-native-green-details-2026-10-18.log` |
| 无插桩 Windows 原始三面板场景 | GREEN，47.86 秒，1 passed；`git-stale-page-native-green-all-2026-10-18.log` |
| 上述真实标题栏 SC_CLOSE 路径 | 两轮均完成关闭，所有本用例 HWND 退役，父窗口保持响应 |
| 受控原生 close gate | 1 passed，0.19 秒；`git-load-fix-ui-close-2026-10-18.log` |
| 独立 Windows 错误窗口 WM_CLOSE 控制 | 1 passed，1.11 秒；`git-load-fix-native-close-control-2026-10-18.log` |
| 实际数据库树提供者失败/手动重试、刷新与退役 | 1 passed，41.03 秒；`git-load-fix-tree-provider-retry-2026-10-18.log` |
| 正常 Git 仓库完整 Windows smoke | 1 passed，103.74 秒；`git-load-fix-native-smoke-2026-10-18.log`，`evidence.json` 确认完整流程、无未读错误和窗口退役；已核验分支、详情、冲突三张原生截图 |
| 非 UI workspace 测试 | 77 组，278 passed / 0 failed / 300 ignored，exit 0；`git-load-fix-non-ui-workspace-2026-10-18.log`，编译 5 分 55 秒 |
| workspace 编译检查 | exit 0；`git-load-fix-workspace-check-2026-10-18.log`，3 分 58 秒（包含构建锁等待） |
| 任务快照 `cargo fmt --check` | 通过 |
| 原工作区 `cargo fmt --check` | 失败，仅已有 `extensions/preview/toolbar.rs`、`extensions/surface.rs` 的两处换行偏差；本次未改这些文件 |

无插桩的修复后三面板日志仍实际出现一次 `host.ui.stale / StaleRevision`，随后同一根页由 loading 转为 loaded、error 为 None。这轮 GREEN 覆盖到了原拒绝路径，并非只依靠没有触发版本竞争的一轮结果。

核心重现命令（从仓库根运行，候选路径固定为上述 0.1.7 ZIP）：

```powershell
cargo test -p plugin-runtime --test git_plugin --no-run --jobs 1 --manifest-path target/git-ticket119-source/Cargo.toml --target-dir target
# 将生成的测试 EXE 与同版本 PDB 冻结后运行，避免链接锁；设置 NANOBUG_GIT_CANDIDATE。
git_plugin.exe --exact workbench::project_loading::git_package_project_loading_distinguishes_non_git_roots_from_ready_workbenches --ignored --nocapture --test-threads=1
editor_app.exe --exact extensions::worker::native_close_tests::git_non_repository_details_tree_and_caption_remain_responsive --ignored --nocapture --test-threads=1
```

## 尚未验证的边界

用户报告的“整个界面卡死”未独立复现：修复前的真实树重试点击、计时器及 WM_CLOSE 已返回；修复后补测系统标题栏 SC_CLOSE 也成功。不能把本次文件树超时修复写成已证实修复全部卡死原因。没有重跑历史八主题/语言/缩放组合，亦没有验证系统 IME 候选弹窗。

## 本地交付

- 可执行文件：`target/git-load-fix-release-2026-10-18/Nanobug.exe`，117,530,624 bytes。
- SHA-256：`44aa8665075bd6b2d7c098434b1c54c2385caeb4d485329f06bbd6b594aa544a`。
- 同目录保留匹配 PDB 与使用说明；保存文档并退出当前 Nanobug 后运行该文件，打开并按需信任 `C:/Projects/RustProjects/Nanobug-Git-Lab/workbench`。
- PE32+、Nanobug 品牌资源（133,912 bytes）及 8 MiB 原生栈预留已检查；新 EXE 的 `--export-plugin-sdk` 返回 0，向任务私有目录导出 50 个 SDK 文件，没有启动用户 GUI 或改写用户配置。
- Release 构建成功，3 分 39 秒，日志 `git-load-fix-release-build2-2026-10-18.log`。为保留运行中的 `target/release/editor-app.exe`，仅在任务源码快照临时声明同一 `src/main.rs` 的独立 bin `nanobug-git-load-fix-20261018`，为其绑定相同品牌资源和栈配置。首次临时入口因原 bin 资源绑定不完整而 exit 101，原日志 `git-load-fix-release-build-2026-10-18.log` 保留；第二次已修正并通过。
- 临时 Cargo/bin 与 build.rs 条目已撤销，根工作区的构建入口未修改；恢复后再次 `cargo fmt --check` 通过，`cargo check --workspace` exit 0（3.07 秒），分别见 `git-load-fix-canonical-fmt-final-2026-10-18.log`、`git-load-fix-workspace-check-final-2026-10-18.log`。
- 源码摘要、候选身份与最终检查结果见 `target/git-load-fix-release-2026-10-18/verification.json`。运行中的原 `target/release/editor-app.exe` 修改时间仍为 `2026-10-18T05:36:16.9869287Z`；未替换或重启用户程序。

既有安装、用户配置和测试项目状态均保留。此次为本地修复交付，不是插件包更新或正式 Release 发布。

返回[插件文档目录](../README.md)。
