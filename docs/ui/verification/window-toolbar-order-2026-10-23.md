# 底栏窗口按钮顺序与长按拖动

后续更新：消息提醒在 2026-10-24 按用户要求改为普通信息蓝点、警告橙点、危险／错误红点，包含溢出入口；见[三色提醒验收](message-severity-2026-10-24.md)。本文下方“未读红点”保留当时实现记录。

## 本轮行为

- 默认前三项为资源管理器、大纲、消息；其后保留原生终端，插件窗口按首次安装顺序追加。同一插件内按 `status_order`（缺省 1000）和清单顺序排列，升级保留安装位置。
- 鼠标左键按住 350 毫秒后移动开始排序；原按钮和窗口布局在松手前保持不变。有效位置显示主题色插入线，组外及滚动裁剪区外显示禁止图标。浮动标签位于指针右上方，顶部空间不足时改为右下方，并限制在窗口内，避免遮挡插入线或裁切文字。
- 有效落点松手才提交；组外松手、Escape、窗口失活、来源移除均取消。拖动与取消后的松手不会触发窗口显隐，短按和键盘激活继续使用原按钮行为。
- 窄窗口的“更多窗口”菜单列出全部窗口，可纵向拖动到任意位置，支持同一菜单内连续调整。插件集合变化后拒绝旧菜单的重排并关闭，重开获取当前列表。
- `SessionState.window_button_order` 保存本机工作区的稳定按钮 ID；重开恢复，暂时停用插件保留原位置，新窗口追加。消息被排到溢出部分时，“更多窗口”保留未读红点，菜单保留未读说明。
- 旧安装记录没有历史先后信息，`Installed.install_sequence = 0` 的记录以插件 ID 确定性排列；不根据目录修改时间伪造首次安装顺序。旧会话缺少新字段时采用上述默认顺序。

## 实现边界

通用控件位于 `crates/editor-ui/src/reorder_group/`，通过 `ReorderGroupState`、`ReorderGroup` 和 `ReorderEvent` 暴露横向／纵向手势，复用 GPUI 原生拖动和 Base 按钮。菜单的可选重排也走同一控件；宿主只负责窗口身份、安装顺序、会话保存及失效校验。

安装序号在资源包、无工作区安装和 WASM 事务的成功提交处统一分配；升级与启停保留。没有改变插件能力协商、权限、面板显隐和功能按钮排序，也没有新增依赖。

## 验证状态

| 检查 | 实际结果 |
| --- | --- |
| `cargo test -p plugin-runtime --test installation_order --test installation_scope` | 7 项通过，1 项 WASM 用例按要求另行显式执行 |
| `cargo test -p plugin-runtime --test installation_scope wasm_install_respects_global_opt_in -- --ignored` | 1 项通过 |
| `cargo test -p editor-ui reorder_group::tests -- --nocapture` | 9 项通过，含早移取消、短按／键盘、失焦、Escape、裁剪、最终松手坐标与预览不遮插入线 |
| `cargo test -p editor-app window_toolbar -- --test-threads=1` | 5 项普通测试通过；真实包与原生用例按忽略说明另行执行，不计入普通测试通过数 |
| 编译后测试程序：`tests::host_messages::button::host_messages_button_selection_tracks_panel_visibility --exact --nocapture --test-threads=1` | 1 项通过，确认消息按钮原有选中绘制和显隐行为 |
| 编译后测试程序：`extensions::tools_tests::order::window_toolbar_real_packages_keep_installation_and_user_order --exact --ignored --nocapture --test-threads=1` | 1 项通过，42.25 秒；真实包进入公开管理器及宿主，覆盖安装／升级、用户拖动、启停、后续新增插件和过期溢出菜单 |
| `cargo test -p editor-app extensions::tools_tests --jobs 2 -- --ignored --nocapture --test-threads=1` | 最终 3 项全部通过，108.76 秒，含上述排序用例、窗口控件可见策略和原有功能上下文／独立溢出菜单组合验收；日志 `target/window-toolbar-package-tests-final.log` |
| Windows 原生验收 | 通过，见下文 |
| `cargo check --workspace --jobs 2` | 通过；存在工作区既有警告 |
| `cargo test --workspace --exclude editor-app --jobs 2` | 执行至 `language_declarative` 时失败：测试仍期望 `language.lsp 1.7.0`，同工作区另一项改动已将实际版本升为 `1.8.0`。此前 `editor-ui` 127 项普通测试通过（12 项忽略），包括本次 9 项控件用例；后续测试未执行 |
| `cargo fmt --check` | 最后失败项位于其他任务的语言激活／准备和 Maven 测试文件，本任务文件无格式差异；未改动那些文件 |
| `git diff --check` | 通过 |

测试使用仓库已有冻结 `tools-example` 包，没有通过历史脚本重新打包。新增排序夹具按当前公开 `ui.panels >=1.1` 契约声明隐藏窗口控件。同模块旧组合测试补充公开视图需求以激活冷安装的辅助插件，验证其功能按钮之前先建立实际贡献。

### Windows 原生证据

最终测试程序直接执行 `tests::window_toolbar::native::window_toolbar_native_windows_acceptance --exact --ignored --nocapture --test-threads=1`，并设置 `NANOBUG_WINDOW_TOOLBAR_OUTPUT` 为本地证据目录。父进程限时管理测试子进程，使用隔离的临时工作区与用户配置；没有修改用户显示缩放设置。

- 平台为真实 WindowsPlatform／D3D11，输入通过原生窗口的 `Window::dispatch_event` 发送，不是人工鼠标操作；截图读取实际 D3D 渲染目标并核对 Win32 客户区尺寸。
- 浅色 100% 和深色 150% 均通过：默认顺序、短按显隐、长按拖动、有效放置、非法区域、Escape 取消、保存后重开、拖动不误激活窗口。实际缩放分别为 1.0／1.5，重开后再次确认。
- 已目视检查有效插入和禁止区域截图；标签不裁切、不遮挡插入线，禁止标志可辨识。
- 本机证据位于 `target/window-toolbar-native-final/`：`evidence.json`、10 张 PNG 及各自尺寸／缩放 JSON。生成目录不纳入源码。

### 范围与限制

排序以工作区保存；旧插件记录缺少真实安装历史时只能采用上述确定性回退。此次未提交或推送 Git 变更，也未将同工作区其他进行中任务的结果计为本次交付。

全量检查未全部通过：`crates/plugin-runtime/tests/language_declarative.rs:239` 与本轮未修改的 `crates/plugin-runtime/src/capabilities.rs` 中语言能力版本不同步；格式检查涉及的其他任务文件也保留原状。对应实际日志为 `target/window-toolbar-workspace-tests-final.log`、`target/window-toolbar-format-final.log`，不能将这些检查报告为通过。

返回[原生 UI 目录](../README.md)。
