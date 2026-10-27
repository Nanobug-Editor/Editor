# 运行配置添加模板菜单

日期：2026-10-27。状态：实现及菜单交互验收通过，代码已本地合并 main；保留下述既有原生退出限制。

用户要求将运行配置窗口 `+` 的模板抽屉改为菜单弹窗。当前要求已同步到[运行配置规格](../specs/run-config-plugin-tree.md)；早期抽屉验收不作为本轮通过证据。

## 实现

- 添加按钮按实际绘制边界定位菜单，在按钮下沿显示分组、图标和模板名称；配置树持续挂载，右侧表单不移动。
- 复用 `editor-ui::menu::PopupMenu` 的焦点、键盘、滚动及关闭逻辑，只增加通用图标行呈现；不引入其他组件运行时。
- 不可用模板保留置灰和原因提示，空目录保留已有本地化提示。打开期间到达的模板目录更新同一菜单实体，保留选择与焦点。
- 选择沿用现有草稿创建、当前文件夹插入和插件表单请求；不隐式保存、构建、运行或调试。菜单回调核对原窗口身份。

## 验证

工作区：`C:/Users/Tmiracle/.codex/worktrees/run-config-add-menu/Editor`，分支 `codex/run-config-add-menu`，起点 `2b043e2d74c91cbb10f44ff8e247cf677425fd48`。独立 Cargo 输出目录为该工作区 `target/`，构建前已通过 `cargo metadata --no-deps --format-version 1 --locked` 核对。

| 命令／场景 | 实际结果 |
| --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test -p editor-app template_menu --locked --offline -j 6 -- --test-threads=1` | 最终重跑 2 passed、0 failed；覆盖深浅主题、菜单定位、配置树保留、禁用项跳过、Esc／外部点击、键盘重开、文件夹插入、长菜单滚动和目录更新 |
| `cargo test --workspace --exclude editor-app --locked --offline -j 6` | 126 组，831 passed、0 failed、441 ignored；包含 editor-ui 控件回归 |
| `cargo check --workspace --locked --offline -j 6` | 通过，保留已有警告 |
| `cargo tree --workspace --target all --edges normal,build,dev --all-features --locked --offline` | 通过；依赖树、锁文件无 gpui-component／宏库，源码只有既有防回归断言提及这些名字 |
| `cargo build -p editor-app --locked --offline -j 6` | 独立工作区普通 dev 原生程序构建通过 |

日志位于该工作区 `target/verification/`。定向最终结果为 `target-template-menu-test-verified.log`；早期失败日志也保留：首轮断言未考虑通用菜单的 8px 视口边距，随后补齐 Base 按钮激活所需的按键释放事件，最终原生输入重开检查通过。旧实际包用例的模板定位保留，只将抽屉几何断言同步为菜单锚点。未执行的 ignored 实际包测试不计为通过。

### Windows 原生交互

使用本轮构建的 `target/debug/editor-app.exe`，临时工作区为 `C:/Users/Tmiracle/AppData/Local/Temp/nanobug-run-menu-20261027/workspace`，独立配置目录为同级 `profile-native`。提前按 SessionState 格式记录该临时工作区为受限模式，不处理或授予原生信任提示；未安装插件、运行配置命令或写入用户正式配置。

通过真实鼠标和按键检查：添加按钮下方显示 Nanobug、Shell 脚本分组及图标；Esc 只关闭菜单；Enter 从添加按钮重新打开菜单，End／Enter 选择 cmd 后创建未保存草稿并展示对应表单；再次打开后，外部点击只关闭菜单，原配置和表单保留。最后点击取消，草稿丢弃，配置窗口正常关闭。

**退出限制：** 关闭验收主窗口时，`accesskit_windows 0.34.0/src/util.rs:380:52` 的 `ClientToScreen` 复现 `HRESULT(0x80070578)` 无效窗口句柄及 COM cannot-unwind abort，日志为 `native-accepted-stderr.log`。签名与[既有原生窗口退出记录](git-commit-panel-2026-10-24.md)相同；不宣称普通进程退出通过，不升级或修改本轮范围外的依赖。菜单操作及配置窗口取消已在该错误之前完成。

## 交付

用户授权验证通过后普通提交并本地合并 main；未授权推送或发布。主目录开始时存在其他任务未提交改动，不能混入本次提交。

实现提交 `da002c3b0922f669643e71bdbc39812edb388b3d` 仅包含菜单、直接相关测试和文档。首次交付因 main 的其他未提交改动受阻，其中 `docs/plugins/README.md` 与本轮同文件；用户随后明确授权“允许临时保存并恢复，再合并 main”。

已在排他集成锁内保存 19 个既有改动文件的原始字节、摘要及未暂存补丁，再通过带明确身份的临时 stash 保留跟踪和未跟踪文件。隔离候选从 main `2b043e2d74c91cbb10f44ff8e247cf677425fd48` 快进到实现提交，源码树与已测试工作区完全相同，候选格式检查通过，沿用同一源码的上述测试和原生验收；主目录随后快进到该提交。

恢复核对以原始字节为准：非重叠文件逐一匹配，唯一重叠的插件文档索引保留原有本地修改并增加本轮验收链接。备份位于 Git common directory 的 `task-worktree/backups/run-config-add-menu-20261027/`，集成及恢复回执为 `task-worktree/tasks/run-config-add-menu-merge-receipt.json`；核对成功后只移除本次临时 stash 和干净集成工作区，不删除任务分支或任务工作区。未推送或发布。
