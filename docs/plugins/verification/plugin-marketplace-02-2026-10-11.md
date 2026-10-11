# 插件市场 02：宿主验证

2026-10-11，依据 [市场工单 02](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/issues/3)。本单在隔离工作树 `C:/Users/Tmiracle/.codex/worktrees/marketplace-versions/Editor`、分支 `codex/marketplace-versions` 实施，基线为 `a07eafc848044092bbbfd55c108b0f81b69c13ef`。主工作区其他任务的改动未提交或改写。

## 实现和验证接缝

原生市场提供历史版本、所选版本说明/日志、统计时间和兼容原因；已装插件只允许更高兼容版升级。后台启动/六小时/手动检查共用新鲜度和 generation 门禁，标题栏红点保留最后有效检查结果。新增权限确认、原有权限直接更新，撤回只影响可安装性，不远程修改已有安装。

测试经过实际 HTTP/ZIP、Package 和公开 Manager/InstallControl；原生用例从实际已安装记录进入窗口，以真实输入事件执行历史导航、确认/取消和升级。真实 SDK guest 包用公开打包入口生成，再由市场夹具导出；不增加插件专用宿主 API。

## 实际结果

- `cargo fmt --check` 通过；`cargo test --workspace --exclude editor-app` 233 passed / 181 ignored；`cargo check --workspace` 通过。全量链接首次缺 `msvcrt.lib`，导入现有 Visual Studio 18 Community 的开发环境后重跑通过。
- `cargo test -p editor-app market -- --ignored --nocapture --test-threads=1`：3 passed / 0 ignored，419.55 秒。真实首装、新增权限确认/取消、迁移/激活失败恢复和成功更新均经实际 WASM；工作区内容保持不变，旧/新私有数据通过 guest 公开命令观察。
- SDK guest 通过 `--plugin-package` 和市场 `tests.export_wasm_fixture` 生成，ZIP SHA-256 为 `6228ee43d31af039f9dff2483f331f046ff37b70b11de6f3ff49c1258505421a`。工具链 Rust/Cargo 1.98.1、Windows x86_64 MSVC，未安装新工具。
- 最终 `cargo test -p editor-app market -- --test-threads=1` 7 passed / 3 ignored（这 3 项已如上显式执行）；`cargo test -p editor-app local_import -- --test-threads=1` 2 passed。涵盖多插件历史导航、兼容默认选择、红点、取消/确认、本地 ZIP 首次检查及第二次可见授权窗口。
- Standards 与 Spec 双轴复查均无未修复阻断。其他 ignored 不计通过；手工操作系统 IME 和其他平台不作已验证声明。

市场端的注册表、统计、双轴审查与 Pages 交付证据统一记录于[市场验收记录](https://github.com/T-miracle/Nanobug-Plugin-Marketplace/blob/main/docs/verification/02-versions-updates-2026-10-11.md)。生产空目录按用户此前决定保留，测试插件不上架；实际官方包由后续工单 03 提供。
