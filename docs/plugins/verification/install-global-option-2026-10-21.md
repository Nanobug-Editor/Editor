# 首次安装插件的全局启用选项

日期：2026-10-21。依据：用户要求安装提示框左下角显示“是否默认全局启用”，默认不勾选。呈现约定见[管理器布局规格](../specs/plugin-manager-shared-layout.md#2026-10-21-首次安装启用范围)。

状态：本次安装选项与针对性行为验收通过；完整 workspace 测试被其他改动的测试编译错误阻断，另有历史 ignored 面板用例失败，详见下文。未提交、推送或发布。

## 实现范围

- 本地包和市场首次安装的确认框共用一个本地 Checkbox 组合，位于固定操作行的左侧；状态仅属于当前确认框。
- 未勾选时，工作区插件保存为当前项目启用；空项目上下文与应用作用域插件只安装、保持停用。勾选后保存全局启用。
- 选择经 Worker 和 InstallControl 进入资源包、后台 WASM 准备与无项目安装三条现有事务路径。首次启用范围与安装记录一起持久化，不先全局启用再关闭。
- 更新、降级及重装沿用已保存的全局默认和项目覆盖；原有权限、信任与按需激活检查继续执行。
- 工作区已有 UI 控件迁移、滚动和插件契约等修改不计作本次成果；本次没有修改公开访客 SDK、插件包或版本。

## 回归入口

- `crates/plugin-runtime/tests/installation_scope.rs`：资源包在项目内和项目外的默认值、应用作用域、注册表重开、其他项目不可用、更新保留设置；显式忽略用例检查账本中固定 example WASM ZIP 的事务安装。
- `extensions::installation_tests::install_confirmation_global_option_is_unchecked_and_accepts_opt_in`：本地确认框真实控件的点击、Space 切换、底部布局、中英文、深浅主题与 150% 显示缩放。
- `extensions::tests::second_local_import_shows_visible_consent`：长权限内容下操作行可见，勾选后取消、重开恢复默认未勾选。
- `extensions::worker::marketplace::tests::marketplace_native_consent_and_install_in_both_locales`：真实回环 HTTP ZIP 下载、原生管理窗口确认及未勾选/勾选两种注册表结果。

## 验证结果

构建前已用 `cargo metadata --no-deps --format-version 1` 核对主工作区为 `C:/Projects/RustProjects/Editor`，输出为其 `target/`。共享目录存在其他 Cargo 构建，测试按现有目录锁排队。

| 实际命令或入口 | 结果 |
| --- | --- |
| `cargo test -p plugin-runtime --test installation_scope -- --test-threads=1` | 4 passed、1 ignored；忽略项在下一行显式执行 |
| `cargo test -p plugin-runtime --test installation_scope wasm_install_respects_global_opt_in -- --ignored --exact --test-threads=1` | 1 passed；使用账本中已构建、摘要校验的固定 example ZIP |
| `cargo test -p editor-app install_confirmation_global_option -- --test-threads=1` | 1 passed；默认值、鼠标、Space、中英文、深浅主题和 150% 缩放 |
| 当前测试 EXE 的 `extensions::tests::second_local_import_shows_visible_consent --exact --test-threads=1` | 1 passed；长内容、底部布局、勾选后取消再打开的状态重置 |
| 当前测试 EXE 的 `extensions::worker::marketplace::tests::marketplace_native_consent_and_install_in_both_locales --exact --test-threads=1` | 1 passed；真实回环下载与两种全局默认的注册表结果 |
| `cargo test -p editor-app unbundled_official_market_install_and_existing_user_upgrade -- --test-threads=1` | 1 passed；固定官方 TOML ZIP 的当前项目高亮、首次安装和已有用户升级 |
| `cargo fmt --check` | 最终通过；日志 `target/install-global-option-format-final.log` |
| `rustfmt --edition 2024 --config skip_children=true --check <本次 Rust 文件>` | 通过 |
| `cargo check --workspace` | 通过；日志 `target/install-global-option-workspace-check.log` |
| `git diff --check`、本次新增 i18n 键与文档相对链接检查 | 通过 |

两个直接执行的 GPUI 测试使用本轮 Cargo 生成的 `target/debug/deps/editor_app-646c88d348da6726.exe`，设置 `RUST_MIN_STACK=16777216`。对应日志为 `target/install-global-option-local-ui.log`、`target/install-global-option-market-ui.log`；官方升级和真实 WASM 分别见 `target/install-global-option-official-upgrade.log`、`target/install-global-option-wasm.log`。以上针对性测试共 9 项通过，不把默认跳过项计为通过。

首次完整格式检查遇到其他进行中改动的格式差异及尚未创建的 scroll 测试模块；后续模块补齐、格式差异消除后，重新执行完整命令已通过。本次没有修改那些 UI/滚动文件。

## 未通过与未覆盖

- 首次 `cargo test --workspace --exclude editor-app` 在并行链接时出现 Windows 资源不足（OS error 1450）及 `msvcrt.lib` 链接错误；日志为 `target/install-global-option-workspace-tests.log`。
- 改用 `cargo test --workspace --exclude editor-app --jobs 1 -- --test-threads=1` 后，在其他任务新增的 `crates/editor-ui/src/scroll/tests/native_windows.rs` 编译失败：第 12 行 `crate::assets::AppAssets` 不存在，第 49 行 `Root::new` 的 Context 和 Entity 类型不匹配。该文件不在本次修改范围内；完整 workspace 测试未通过，也未宣称控件全套通过。日志为 `target/install-global-option-workspace-tests-serial.log`。
- 额外显式执行历史 `extensions::capability_tests::capability_package_consent_displays_native_text_and_uninstall_reclaims_panel --exact --ignored --test-threads=1`，安装确认已发出 `Work::Install(package, true)`，实际安装成功，但停在“已安装访客欢迎文本立即出现”的断言。该旧用例直接安装后复制 `manager.live`；当前既有冷安装/按需激活流程不会仅因安装生成该欢迎面板（`manager/data_updates.rs` 的 `activate_business = old.is_some()`）。本次仅适配显式勾选与 Work 参数，没有扩大为修复历史面板验收。日志为 `target/install-global-option-application-consent.log`。
- 未执行七个官方 WASM 包的完整 ignored 矩阵，未执行 Release 打包、跨平台或独立桌面手工全流程验收；本次 UI 结果来自实际本地控件的 GPUI 输入和可见布局断言。
