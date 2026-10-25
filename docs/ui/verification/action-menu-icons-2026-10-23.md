# 分层菜单方向图标

日期：2026-10-23。用户要求将 Git 顶部分支菜单中的文字箭头改为方向图标。

## 实现范围

通用 `editor-ui::menu::ActionMenu` 使用已有 `Icon` 控件及 Kit 静态资源：折叠组使用 `ChevronRight`，展开组使用 `ChevronDown`，右侧子菜单指示使用 `ChevronRight`。图标为 12 逻辑像素，继承所在行的主题和选中颜色，与本地目录树一致。保留原有行高、缩进槽、检查标记及展开、搜索、键盘和命令路由。

源码：`crates/editor-ui/src/menu/action_menu/view.rs`。本次未修改插件协议或 Git 插件包；所有使用通用分层菜单的插件共用这项外观调整。

## 验证

验证日志保存在 `target/action-menu-icons-2026-10-23/`。共享工作区中其他任务的已有改动不归属本次。

- `rustfmt --edition 2024 --check crates/editor-ui/src/menu/action_menu/view.rs` 及本次文件的 `git diff --check` 通过。
- `cargo test -p editor-ui menu::action_menu::tests` 首次执行被已有 `tabs/dragging/tests.rs` 的 E0716、E0499 编译错误阻塞。本轮未修改该文件；其他任务随后修正了它，下述 workspace 测试中的两项菜单用例均通过，覆盖中文输入法组合提交和嵌套动作搜索。
- `cargo test --workspace --exclude editor-app --jobs 2`：`editor-ui` **127 passed，12 ignored**；整体在 `plugin-runtime --test language_declarative` 的 `declarative_language_plan_needs_no_empty_wasm_and_retires_original_authority` 失败，实际版本 `1.8.0` 与断言的 `1.7.0` 不一致。该测试与版本实现不在本次修改范围，未将后续未运行或 ignored 用例记为通过。
- `cargo fmt --check` 最终仍未通过：差异来自其他任务的历史列验收、画布文字等文件；本次菜单文件无格式差异，详见 `format-final.log`。
- `cargo check --workspace --jobs 2` 最终通过，保留已有警告，日志为 `workspace-check-final.log`。首轮等待共享构建锁后被中断，未计为通过；补跑取得完整成功结果。
- 全仓 `git diff --check` 通过，保留已有 CRLF 提示。

## 原生显示

使用当前 checkout 在本次菜单改动后构建的 Debug 测试程序（`target/debug/deps/editor_app-420de45671c333eb.exe`，2026-10-23 16:53:41），复制到证据目录运行既有 `git_native_workspace_presentation`。实际 Git 包为上一任务的 0.1.33，SHA-256 `a2b4d7fe638b073aa49cbe50c36fe67aa038f87edf69973586b331662053268c`；非 Git Review 夹具通过宿主 `--plugin-package` 重新构建。运行使用隔离用户配置和临时 Git 工作区。

已检查 Windows D3D11 真实截图：

- `native/dark-zh-CN-100/presentation-head-menu.png`：深色、中文、100%。
- `native/light-en-150/presentation-head-menu.png`：浅色、英文、150%；通过同一程序的 `git_native_matrix_child` 单独运行该组合，使用与父测试一致的私有 Git 配置。

两张截图均包含展开组、折叠组、选中分支及实际打开的级联子菜单。向下／向右方向、垂直对齐、图标缩放及普通／选中行颜色正常；原有检查标记保留。

两条流程均在菜单截图之后的 `incoming filtered history with main title` 步骤超时，因此整条原生流程未通过，`shutdown_complete` 不标记成功；日志中的项目关闭和原生窗口撤销均完成。本记录只确认实际观察到的菜单显示，不宣称完整 Git 历史交互通过。未提交、推送、发布或替换用户正在运行的宿主。
