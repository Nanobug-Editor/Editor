# 启动恢复上次项目

日期：2026-10-21。状态：本次实现、回归及 Windows 原生验收完成，要求的格式、workspace 测试与编译检查通过；尚未提交、推送或发布。

## 已确认行为

用户要求编辑器默认打开上次最后退出的项目，并在设置页面加入开关。本次替代[项目下拉与空启动](../specs/project-menu-startup.md)原有的“无路径不恢复项目”条款。

- 设置 → 外观与行为 → 启动 → 启动时恢复上次项目，默认开启，空窗口和项目窗口均可编辑。
- 用户 profile 下的 `startup.json` 保存开关和退出项目；项目设置不能覆盖，不改变既有 `MeEditor` 数据目录。
- 普通主实例无路径启动才读取恢复目标，显式项目或文件优先；同 profile 的第二次无路径启动仍激活现有窗口。
- 单独关闭项目在清理成功、原生窗口实际关闭时更新记录；统一退出应用使用开始退出前最后使用的项目，所有资源清理成功后写入。取消、清理失败不改退出记录；空窗口退出保留已有记录。
- 恢复沿现有打开、信任和会话入口，不提高工作区信任，不自动重启运行程序。记录目录消失、无记录或配置损坏时进入空编辑器；损坏文件不覆盖。
- 退出记录保存失败时保留旧记录并输出诊断，不阻塞已经完成清理的退出；设置页面保存失败则显示错误，保留原开关值，允许重试。

## 验证结果

- 修复前：`cargo test -p editor-app startup_restore_setting_is_available_without_a_project -- --nocapture`，1 项失败，实际设置对话框缺少启动恢复控件；测试运行 0.03 秒。
- `cargo test -p editor-app startup_restore -- --include-ignored --nocapture --test-threads=1`：12 项通过，0 项跳过，运行 13.21 秒，包含 Windows 原生验收。覆盖默认值、持久化、显式目标优先、profile 隔离、无效目录与损坏文件保护、关闭顺序、统一退出快照、取消、清理失败重试、设置错误呈现与键盘按下/松开。
- 已构建测试程序执行 `app::projects::tests --nocapture --test-threads=1`：46 项通过，5 项按原有夹具或原生环境要求跳过；未将跳过项计为通过。
- Windows/D3D 验收使用临时项目和独立 profile，验证实际恢复的中文文本及受限信任状态，通过绘制后的 Base 控件发送平台鼠标事件，检查两种主题下的开关语义、布局和持久化。两张原生截图位于本机 `target/verification/startup-project-restore-2026-10-21/`，均已目视检查。单独显式运行该原生测试也通过，运行 3.35 秒；未验收系统中文 IME 候选窗，本次控件不接收文本输入。
- `cargo check --workspace` 通过，保留已有警告，运行 2 分 06 秒。
- `cargo fmt --check`、本次修改文件的独立格式检查与 `git diff --check` 通过，规格及本记录中的本地链接均有效。
- `cargo test --workspace --exclude editor-app --jobs 2`：421 项通过，0 项失败，336 项按既有条件跳过，包含 `editor-ui` 控件测试；未将跳过项计为通过。第一次运行曾因范围外 `crates/editor-ui/src/scroll/tests/native_windows.rs` 的 3 项编译错误失败，该文件由其他工作修正后，本次复跑通过。
- 首次实现后曾被范围外 `crates/editor-ui/src/shortcut_panel/mod.rs:325` 缺少 `SmoothScrollable` 导入阻塞；该处由其他工作更新后继续验证，本任务未修改滚动组件。

## 实现入口

- [启动策略与持久化](../../../crates/editor-app/src/app/startup/preferences.rs)、[退出目标快照](../../../crates/editor-app/src/app/projects/controller/startup.rs)。
- [设置页面](../../../crates/editor-app/src/app/settings.rs)、[设置所有者](../../../crates/editor-app/src/app/settings/owner.rs)。
- [用户配置回归](../../../crates/editor-app/src/app/startup/preferences/tests.rs)、[窗口与设置交互回归](../../../crates/editor-app/src/tests/project_home/restore.rs)。
- [Windows 原生启动与设置回归](../../../crates/editor-app/src/tests/project_windows/startup_restore.rs)。
