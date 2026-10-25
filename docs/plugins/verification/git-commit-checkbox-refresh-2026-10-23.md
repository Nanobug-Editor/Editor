# Git 提交复选框刷新修复

日期：2026-10-23。状态：源码修复、插件测试、实际包索引回归、Windows 原生验收及全仓门禁完成；未安装到用户配置、提交、推送或发布。

## 问题与边界

目录／文件复选框启动索引操作时，插件把整个提交树标记为 disabled，通用宿主因此以 0.5 透明度绘制整棵树；操作结束又恢复，造成大面积闪动。第一次反馈还会在提交说明上方插入 24px 按钮及 8px 间距，挤压树视口。目录数据本身已有增量补丁，不能把正常的勾选状态更新当作重新加载。

- 独立 Git 0.1.33 保留提交树的正常显示、原生滚动和展开交互；`commit_check` 与文件业务选择仍检查忙碌状态，不并发执行第二次索引写入。
- 反馈区从首次显示就保留固定高度，成功或索引锁错误只更新该区域内容；提交草稿和树的位置不随反馈出现而改变。
- 未改变立即暂存／取消暂存、脏缓冲区检查、索引观察复查、字面路径批处理、部分暂存及 unborn HEAD 语义。
- 生产改动仅在独立插件中。宿主仅扩充既有实际包和原生验收，不增加 Git 专属 API、能力版本或权限。

实现位于 `C:/Projects/RustProjects/Git-for-Nanobug/src/commit_panel.rs`，插件回归位于同目录下的 `commit_panel/tests.rs`；宿主原生逐帧断言位于 `crates/editor-app/src/extensions/worker/native_acceptance/commit_panel/stability.rs`。独立插件目录当前没有 `.git`，本轮修改前文件保存在证据目录 `before/`，没有初始化新仓库或改写历史。

## 验证

证据目录：`C:/Projects/RustProjects/Editor/target/git-commit-checkbox-refresh-2026-10-23/`。

- `red.log`：通过真实 `tree::Event::Checked` 处理函数启动目录取消暂存，稳定失败于整个提交树被禁用。
- `feedback-red.log`：成功反馈使提交区增加一个布局项；`green.log` 的两项回归通过。重复勾选仍只保留一条已获准的索引写入。
- `native-red.log` 及 `native-red/dark-zh-CN-100/evidence.json`：旧 0.1.31 包的实际鼠标勾选在原生帧中失败于 `native commit tree was disabled during a checkbox operation`，确认测试能捕获真实界面中的瞬时问题。
- `guest-tests.log`：最终插件 173 项测试通过。旧忙碌态回归调整为提交树可浏览，其他写入／冲突控件仍禁用。
- `package.log`：宿主 `--plugin-package` 直接构建 `git-0.1.33.zip`，未安装工具链或通过归档脚本打包。
- `integration.log`：最终实际包的 `index_edges::` 8 项测试全部通过，显式执行 `--ignored`。逐次检查索引操作期间提交树仍可用，既有目录缓存、展开状态和草稿保持；覆盖目录批量操作、字面路径、锁恢复、脏缓冲区、部分暂存、外部索引变化及 unborn HEAD，实际索引与工作区字节分别验证。
- `native.log`、`native-results.json`：最终候选通过深色中文 100% 与浅色英文 150% 两组原生验收。每次真实勾选逐帧检查树和输入区的实际边界、原生视图身份与文件选择，覆盖锁失败／恢复、目录与部分暂存文件勾选、取消勾选、树滚动和中文多行草稿保留。两组均正常释放原生窗口，`unexpected_error_logs` 为空；深浅主题截图已检查。未验证系统 IME 候选窗口。
- `check-results.json`、`final-summary.json`：workspace 非 `editor-app` 测试 519 passed、387 ignored，编译检查通过。最终全仓及独立插件格式检查、受影响测试文件格式检查和 `git diff --check` 均通过；ignored 不计作通过。编译保留既有 warning 和 LNK4217 链接警告，无编译错误。

交付包：`C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.33.zip`，与证据目录 `packages/git-0.1.33.zip` 相同；SHA-256 为 `a2b4d7fe638b073aa49cbe50c36fe67aa038f87edf69973586b331662053268c`。两份原生 `evidence.json` 都记录该摘要。协议仍为 7，能力和安装权限与本轮起点 0.1.32 相同。

```powershell
$nanobug = 'C:/Projects/RustProjects/Editor/target/debug/editor-app.exe'
& $nanobug --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib
& $nanobug --plugin-package C:/Projects/RustProjects/Git-for-Nanobug --output target/git-commit-checkbox-refresh-2026-10-23/packages
cargo test -p editor-app git_native_commit_panel --no-run --jobs 2
cargo test -p plugin-runtime --test git_plugin index_edges:: --jobs 2 -- --ignored --nocapture --test-threads=1
cargo fmt --check
cargo test --workspace --exclude editor-app --jobs 2
cargo check --workspace --jobs 2
```

实际包测试设置 `NANOBUG_GIT_CANDIDATE` 为上述候选。原生测试使用同一候选，设置 `NANOBUG_GIT_NATIVE_COMMIT_PANEL=1` 和独立的 `NANOBUG_GIT_NATIVE_OUTPUT`，在编译后的测试程序执行 `extensions::worker::native_acceptance::commit_panel::git_native_commit_panel --exact --ignored --nocapture --test-threads=1`。测试创建自己的 Git 仓库与用户配置。

初次全仓格式检查被本轮开始时已经存在的 Maven 测试格式差异阻断；本任务没有修改这些文件。共享工作区后续整理后，最终 `fmt-final.log` 检查通过。其他任务的窗口、拓扑颜色、原生等待助手和 Maven 改动不属于本次修复；原生证据对应本轮 `native-build.log` 生成的测试程序及同摘要候选包。
