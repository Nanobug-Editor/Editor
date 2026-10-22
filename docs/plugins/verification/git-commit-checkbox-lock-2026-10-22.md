# Git 提交目录勾选与索引锁反馈（2026-10-22）

## 原因与修复

用户要求目录复选框联动全部子级，并修复原始 Git 错误挤占提交区的问题。沿用已确认行为：勾选立即暂存，取消勾选立即取消暂存，提交只消费索引。

- 目录路径匹配及批量写入已经包含未展开的后代；本次增加 471 个深层后代及相似前缀兄弟目录的回归。实际仓库存在 2026-10-21 18:27:48（本地时间）留下的空 `.git/index.lock`，会直接阻止批量暂存。无法仅凭时间确定该遗留锁最初由哪个进程产生。
- 原提交区直接绘制完整 `self.message`，多行 stderr 撑高固定底部区域。现在使用固定高度的简短反馈按钮，锁冲突显示本地化“索引被占用 · 查看详情”；完整的既有脱敏诊断只在独立滚动弹窗中显示。弹窗冻结打开时的内容，后续刷新不会替换正在阅读的详情。
- 可被抢占的后台 `git status` 原先允许刷新索引 stat 缓存。所有插件 Git 命令设置 `GIT_OPTIONAL_LOCKS=0`，查询不再产生可选索引写入，必要的 add/restore/commit 锁继续由 Git 执行。未添加自动删除锁或重放旧写入的逻辑。
- 源码位于 `C:/Projects/RustProjects/Git-for-Nanobug`，包版本提升为 0.1.28。没有新增宿主 API、权限或 Git 专用宿主分支；未提交、推送、发布或安装到用户配置。

## 当前仓库恢复

两次核对无 `git.exe` 进程，确认锁为超过一天未修改的空文件，并以拒绝其他读写共享的文件句柄核对未被占用后，将锁改名为 `.git/index.lock.stale-20261022-190516`，保留备份。前后索引 SHA-256 均为 `29b737c2a0e301835b90eeeabb6828b12514e8751cb05a8a744e4a932b42363b`；没有执行 add、reset、restore 或删除索引。此为本次确认后的环境恢复，不是插件自动恢复策略。

## 验证记录

证据目录：`C:/Projects/RustProjects/Editor/target/commit-checkbox-fix-2026-10-22`。

- 单元回归首先在旧实现失败：`commit_lock_error_keeps_the_composer_compact` 报告原始错误进入表单；修复后插件全部 149 项单元测试通过。
- 实际旧 ZIP 0.1.27 经公开 Manager 运行新增集成测试：锁冲突布局检查和被动刷新索引不变检查均失败；原有脏缓冲区／字面路径／部分暂存保护两项通过。记录见 `integration-red.log`。
- 同一真实包集成测试加载修复版后 4 项全部通过，见 `integration-green.log`：实际锁仍保留，失败时索引和勾选态不变；详情可打开／关闭，释放测试拥有的锁后，同一目录所有深层文件一起暂存／取消暂存，兄弟目录和工作区字节保留；被动刷新不改索引字节。
- 当前 workspace 的 `cargo fmt --check`、`cargo test --workspace --exclude editor-app --jobs 2`、`cargo check --workspace --jobs 2` 均通过，见 `check-results.json`。忽略的 WASM/UI 项目不计为通过。
- 既有 GPUI 0.7 基线原生验收程序加载新 ZIP，中文深色 100% 提交流程通过，草稿、文件复选框、祖先状态和固定提交区截图位于 `native/`。此项不代表当前 GPUI 0.8 原生程序的验收结果。

插件包：`C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.28.zip`，SHA-256 `21968b4e8e185b2f3bef1c6ade09cdd21858ed76b19f96ec371ddfc3f6da39e8`，与验证目录 `packages/git-0.1.28.zip` 相同。

## 当前宿主原生验收

当前 GPUI 0.8 源码已构建测试程序，保留为证据目录 `native-current.exe`。分别直接运行 `extensions::worker::native_acceptance::git_native_matrix_child` 的深色中文 100% 和浅色英文 150% 子场景，最终均通过；输出分别为 `native-dark/` 和 `native-light/`。两份回执的 `directory_lock_recovery_and_compact_feedback`、`checkbox_partial_stage_unstage_and_draft`、`shutdown_complete` 均为 true。

- 在真实原生复选框上点击未展开分组，测试持有的索引锁使操作失败，锁文件与索引保持原样。
- 错误按钮实测高度 24px；树视口高度变化不超过一行反馈占用。点击详情打开独立滚动区域，中英提示、原始错误、关闭操作及两组截图均已检查。
- 由测试释放自己持有的锁后，一次点击暂存全部 28 个未跟踪后代，再点击混合态更改组暂存全部 29 个文件，取消组勾选后索引为空。后续差异块、文件复选框、中文多行草稿、折叠和滚动流程在最终两次运行中通过。
- 原生截图包括 `commit-lock.png`、`commit-lock-details.png`、`commit-default.png`、`commit-checked.png`、`commit-final.png`，均为 D3D11 读回，未合成。

验收波动如实保留：首轮把按钮辅助名称当作可见标题比较，导致测试等待超时，已修正断言。随后一次深色完整流程在草稿同步断言失败（`native-verified/dark-zh-CN-100/`），该轮目录／锁反馈子场景已通过，截图仍显示完整草稿；补充失败诊断后，浅色与深色重跑均通过。本次没有改写草稿同步业务逻辑，尚未稳定复现该波动，不能把首次失败或矩阵父进程命令记为通过。最终通过的是上述两个独立子场景，回执与截图另行核对。
