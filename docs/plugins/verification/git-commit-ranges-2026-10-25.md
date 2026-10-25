# Git 提交范围与即时勾选响应

日期：2026-10-25。状态：实现、199 项插件测试、同一最终包的 14 项实际业务回归、格式与编译检查完成；全仓测试有本次未改动的终端模块失败，详见下文。未安装到用户配置。

## 已确认行为

用户明确要求采用 IDEA 变更列表方式：「两类文件勾选都只改变本次提交范围，点击『提交』或『提交并推送』时再执行 Git 操作，勾选后按钮要及时响应」。这是对旧「已跟踪勾选即暂存」契约的替换，不能继续将实际暂存区或暂存命令完成作为本地勾选的准入条件。

- 两类复选框、目录及组节点统一维护本地文件集合；目录包括未展开和未加载的子文件。取消勾选保留真实暂存区和磁盘内容。初始范围为空，明确勾选后参与本次提交。
- 两个按钮共用本地选择、非空草稿、完整仓库状态、无冲突及无执行中提交的校验。勾选回执直接发布可用性，不发起 Git 或 ListDocuments。
- 点击按钮后捕获路径与说明，只检查所选文件的原生未保存状态；检查失败要求用户保存并复核，不自动保存无关文档。一次带分支头的 porcelain-v2 状态查询核对冻结的 HEAD／分支及各文件的索引观察，失败时不执行 add、commit 或 push。
- Git 使用字面、NUL 分隔路径 stdin。所选未跟踪文件先加入 Git，再以 `commit --only` 提交所选路径；已暂存重命名包括原名与新名。整文件选择提交当前磁盘内容，包括该文件尚未暂存的修改；未勾选文件已有的暂存内容由 Git 自身的限定路径事务保留。显式暂存与差异块工具继续操作真实 index，与提交范围分开。
- 提交中继续编辑草稿／选择只影响后续提交；重复提交不排入第二份命令。只有成功提交后才执行普通 push，推送失败明确保留本地提交。错误弹窗可关闭，草稿和可重试范围保留。

提交说明通过独立 argv 值传递，路径占用 stdin；说明限制为 16 KiB，并检查 Windows 引号展开后的长度，路径总长限制为 64 KiB，越界在任何 Git 写入前拒绝，不截断。没有 Shell 拼接或新增原生服务。

## 实现与复现

独立源码：`C:/Projects/RustProjects/Git-for-Nanobug`。包版本 0.1.43 → 0.1.44；原生辅助程序仍为 0.1.1。协议、能力和权限不变；宿主生产代码没有 Git 专属修改。

`src/commit_panel/selection.rs` 维护本地范围；`submission.rs` 负责点击时捕获、批量命令和前置状态校验。移除不再使用的复选框写入队列、相关旧状态与旧批处理分支；显式暂存路径不受影响。`completion.rs` 和原生文档回执负责成功／失败链路与清理。

第二轮性能优化为仅发布已缓存节点的 `checked` 元数据，保留原目录结构、图标、计数、分页游标和原生交互状态。一次按路径层级聚合完整目录的选择数量，避免对每个已加载节点重新扫描所有文件并重建整棵树。叶节点与目录分别计数，覆盖已删除文件 `foo` 与新目录 `foo/bar` 同时存在的情况；未加载后代仍纳入父级的全选／半选状态。

红灯命令：

```powershell
& C:/Projects/RustProjects/Editor/target/release/editor-app.exe --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib commit_range_ -- --nocapture
```

旧实现稳定失败于「tracked uncheck queued a Git write」和「commit must respond to the local selection receipt」。修复后同一路径通过，另补 HEAD／分支变化时禁止写入的回归。测试不是只断言命令能构造，还通过公开 Manager 和实际 WASM 包执行真实临时 Git 仓库中的提交／推送。

证据目录为 `C:/Projects/RustProjects/Editor/target/commit-range-2026-10-24/`，沿用本次工作开始时的目录名。`before/` 保存本轮修改前的独立源码、清单和相关集成测试；共享宿主已有其他任务改动记录于 `start-status.txt`。

## 验证结果

- 独立插件红／绿回归及完整测试通过：`--plugin-cargo ... test --lib` 为 **199 passed、0 failed、0 ignored**，见 `guest-optimized-final.log`。普通测试没有跳过此插件的回归。
- **同一最终包的 14 项实包回归全部通过**，包含本地勾选、目录和懒页、590 个未跟踪文件／unborn HEAD、真实索引锁及重试、脏缓冲区、外部索引变化、选中范围隔离、重复提交、说明与范围冻结、提交成功后推送、远端拒绝后保留本地提交，以及既有显式整文件／差异块暂存路径。12 项见 `integration-optimized.log`，提交并推送的一项含成功与拒绝两个子场景，见 `integration-optimized-push.log`；单独的延迟测量一项见 `integration-optimized-latency.log`。全部显式启用 ignored 用例，逐项状态与同包摘要汇总于 `final-verification.json`。
- 首次实包运行中，旧辅助函数将打开错误弹窗后的根节点交互门禁误判为提交树消失。`assert_commit_tree_present` 现分别核对无弹窗时的实际交互准入、有弹窗时未被禁用且仍存在的底层树节点；测试仍检查错误弹窗和 Git 字节结果。初次中断记录保留于 `integration-modal-probe.log`，修正辅助断言后对同一候选重新执行整组，不将该次中断记为通过。
- 重复点击通过真实 UI 准入验证：执行中按钮不可用，第二次事件返回 InvalidHandle；测试应断言拒绝而非对该预期错误 unwrap。提交中修改草稿及选择，只影响下一次提交；实际 HEAD 仅增加一次，已执行提交仍采用原说明和范围。该断言修订没有改变宿主或插件生产代码。
- 最终 workspace、独立插件和本次宿主测试文件格式检查，以及 `git diff --check` 全部通过，见 `format-final-results.json`。首次 workspace 格式检查被共享工作区其他终端／画布修改阻断；其他工作完成后最终检查通过，本任务没有代为格式化这些文件。
- `cargo check --workspace` 通过，见 `workspace-check.log`。`cargo test --workspace --exclude editor-app` 首次遇到编译器异常退出，保留于 `workspace-tests.log`；降低并发以 `--jobs 2` 复跑后，停止于 **editor-terminal 的 166 passed、33 failed**，见 `workspace-tests-retry.log`。失败集中于 `crates/editor-terminal/src/grid/storage.rs:229` 的 `requested.0 < self.visible_lines as i32` 断言，属于本轮没有修改的共享终端工作，未将全仓测试报告为通过，后续尚未运行的测试不计通过。没有修改 editor-app 生产代码，未执行与本次无关的原生应用全套验收。

实际包消费者先通过 Cargo 编译。为避免测试进程长期占用共享 Cargo 构建锁，将产物复制到证据目录 `bin/` 后直接运行 Rust 测试程序；测试仍通过公开 Manager 激活真实 WASM 包。相互隔离的临时仓库业务用例使用三个测试线程，延迟测量单独串行运行。提交并推送使用修正预期拒绝断言后的同类消费者程序，来源及摘要见 `consumer-artifact.json`；最终所有用例均指向下列同一包，没有拼接不同候选的通过数。

## 耗时与交付

从发出真实勾选事件到返回可点击提交节点计时，不包含激活/JIT 和辅助函数的空闲防抖，也不以易受机器负载影响的毫秒阈值决定测试通过。确定性回归断言两按钮在同一次事件回执内可用、没有启动新 Git 进程或原生文档请求、实际 index 字节不变。

| 场景 | 耗时（毫秒） | 记录 |
| --- | --- | --- |
| 0.1.43、小仓库、三次勾选到可点击 | 393、281、293；中位数 293 | `baseline-0.1.43.json` |
| 0.1.44 最终包、相同小仓库 | 3、4、4；中位数 4 | `optimized-0.1.44.json` |
| 590 个未跟踪文件、目录勾选（已有 HEAD／unborn） | 23、20 | `large-untracked-optimized.json` |
| 同一 590 文件场景、单文件勾选 | 15、16 | `large-untracked-optimized.json` |

延迟降低主要来自把 Git 写入移动到明确提交之后；不是宣称 Git commit 本身加快了相同比例。第二轮仅元数据补丁前的 0.1.44 记录另保留于 `final-0.1.44.json` 与 `large-untracked.json`：小仓库 3、4、4，目录 25、23，叶子 19、18。大目录两轮业务测试并行且共享机器负载不同，不将两组小差异当成严格受控的百分比结论。

最终包为 `target/commit-range-2026-10-24/package-optimized/git-0.1.44.zip`，SHA-256：`2b930fe197283d03f4bc0b99b3baf570d0f9e510e3f6c05707be13d4541ede1b`。与 0.1.43 逐项核对：协议、能力、权限和原生服务声明均不变。先前未作第二轮补丁优化的同版本候选保存在 `package/`，不是本次交付包。

未在用户仓库执行暂存、提交或推送，未改动已安装包与用户设置，未提交或发布版本。原生屏幕呈现延迟尚未重新验收，不将 Manager 事件测量称为屏幕帧率。
