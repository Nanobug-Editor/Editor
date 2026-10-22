# Git 提交窗口改造（2026-10-22）

## 范围和行为

用户确认按 IDEA 截图重排提交窗口，并明确选择“勾选立即暂存、取消勾选立即取消暂存；提交暂存区内容”。独立源码位于 `C:/Projects/RustProjects/Git-for-Nanobug`，候选版本为 0.1.27；未执行安装、提交、推送或发布。

- 标题改为“提交 / Commit”。紧凑图标工具栏位于顶部，中间为可滚动文件树，提交说明和提交按钮固定在底部。
- 文件归入“更改”“未进行版本管理的文件”，冲突单独分组；目录显示文件数且不着色，文件保留状态颜色。
- 部分暂存文件只显示一次，用混合勾选态表示；再次勾选暂存剩余修改。目录操作包含尚未展开的后代。取消暂存不改工作区字节，未出生的 HEAD 同样支持。
- 勾选前检查宿主未保存文档，执行前复查索引观察；路径经 NUL 分隔 stdin 和 literal pathspec 处理。现有整文件、回滚预览和差异块操作保留。
- 未加入截图中尚无对应实现的修订上次提交、提交消息历史及提交并推送按钮。

## 通用宿主边界

公开 `ui.tree 1.7` 增加三态复选框、节点后缀及批量目录展开。事件携带原显示状态和目标布尔值；宿主校验实例、资源、epoch 和节点状态，不执行 Git 策略。旧 minor 提供者无法通过可选能力声明使用新字段。原生复选框、主题色、焦点和键盘行为来自本地 UI 控件及 GPUI Base。

原生多行输入框支持显式视口高度，保留原文本状态、IME 与 Undo。插件弹窗使用上游公开遮挡接口，避免父层消耗子按钮的原生按下事件。未引入 `gpui-component`。

原生验收还发现首次 refs/HEAD 查询之间的分支菜单结构变化：比较命令曾在 HEAD 返回后才加入，违反缓存节点的命令意图约束。现改为预先保留命令并在 HEAD 未就绪时禁用，补充了对应回归测试。

## 产物和验证

证据目录：`C:/Projects/RustProjects/Editor/target/commit-panel-2026-10-22`。

- 插件包：`packages-final/git-0.1.27.zip`，SHA-256 `a189ac6016f4608d58b7df0336c207151423468a7f22fd1337a4015c6135015e`。
- 宿主：`Nanobug.exe`，SHA-256 `21140363e974f2036bffaf8532a3c34063ee33a2d34b41d8031427b481236745`。`cargo build -p editor-app --release --jobs 2 --config 'profile.release.package.editor-app.opt-level=0'` 完成编译和链接后，因运行中的 `target/release/editor-app.exe` 被 Windows 占用而在最终替换时报错（os error 5）；使用本轮新链接的 `target/release/deps/editor_app.exe` 复制到验证目录，没有停止用户程序。
- `--plugin-cargo ... test --lib`：147 通过，0 失败，0 忽略。
- `cargo test -p plugin-runtime --lib tree_ --jobs 2`：9 通过，2 忽略；包含过期／伪造勾选事件和目录请求撤销回归。
- 实际 ZIP 经公开 Manager 的 `index_edges::` 测试：5 通过，覆盖整目录、字面路径、未保存缓冲区、部分暂存、外部索引变更和 unborn HEAD。
- 实际 ZIP 的单路径、多行提交说明、仅消费暂存区、差异块、字面路径和重命名取消暂存综合测试：1 通过。同步更新了历史画布测试，使其检查虚拟视口中的最新记录。
- 宿主 `ui::plugin::` 针对性测试：73 通过，17 忽略。
- `cargo test --workspace --exclude editor-app --jobs 2`：未全部通过。`editor-ui` 为 104 通过、11 忽略，包含显式高度与中文多行输入测试；运行至插件运行时后，已有终端能力测试仍断言 `process 1.8`，当前实现为 1.9，因此停止（该运行时测试批次 125 通过、1 失败、3 忽略）。未将忽略项计为通过。
- `cargo check --workspace --jobs 2`：通过。
- `cargo fmt --check`：受其他进行中改动影响失败，包括 Java 测试、aligned editor 几何、滚动测试和调试服务；本次文件单独格式化。`git diff --check` 通过。

## Windows 原生行为

实际候选 ZIP 经插件管理器安装到私有验证配置，在 Windows D3D11 窗口中使用原生按键、指针与滚轮操作。深色中文 100% 和浅色英文 150% 两个子场景均通过，真实截图分别位于 `native-final/dark-zh-CN-100` 和 `native-light`：

- 差异块按钮实际改变索引，部分暂存行随后通过勾选进入完整暂存，再通过取消勾选恢复索引。
- 文件树滚动不移动提交说明区；中文多行草稿在暂存、折叠、展开和滚动之后保持原值。
- 有暂存内容和说明时可提交，空索引时按钮和公开场景均禁用提交。
- 两组 `evidence.json` 均记录 `shutdown_complete=true`、业务场景通过、原生窗口归零、无意外错误日志。图片来自 D3D11 读回，未合成界面；未人工验证操作系统输入法候选窗口。

两个子场景的专用三张截图、工作流身份、业务完成和关闭回执已另行逐项检查通过。旧矩阵父进程仍检查通用工作台截图，因此其首次运行在子场景通过后报告缺少 `workbench.png`；源码已改为本场景的专用清单，没有伪造该图片或把父进程失败记为通过。

## 并行工作区变动限制

已交付候选和上述原生验收使用本轮起始的 GPUI 0.7.0 基线。验证后期，其他进行中改动将共享工作区 GPUI Kit/Base 更新为 0.8.0，并改写了 Cargo.lock；本次没有升级或回退这些依赖。随后重建测试程序时出现 335 个 `E0659: gpui is ambiguous`，首批位于 `app/plugins/status_tests/host_messages.rs` 和 `tests/project_home/` 等既有测试，阻断新依赖下的矩阵父进程复跑。记录为 `native-build-receipt-corrected.log`。这不改变已验证候选的行为结论，但不能把原生验收结果扩展为 GPUI 0.8.0 全仓测试通过。
