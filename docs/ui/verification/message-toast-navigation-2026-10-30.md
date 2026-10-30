# 消息浮窗两行预览与历史定位

日期：2026-10-30。状态：定向回归、Windows 原生交互验收与仓库检查通过；未发布。

用户要求右下角消息浮窗的描述最多两行，并允许点击打开消息窗口、滚动到对应消息。本次要求替代[统一消息验收](unified-messages-2026-10-29.md)中长描述在浮窗内滚动的行为，现行要求见[宿主消息规格](../specs/host-messages.md)。

自动警告／错误卡片和显式插件 `Notify` 使用两行预览，历史继续保留完整正文。通用通知控件只发出点击回调；宿主管理当前窗口的停靠呈现和消息身份。普通临时通知继续使用原来的展示方式。

浮窗持有消息面板的不可变记录 ID。点击时打开并激活消息窗口，按 20 条批次展开到包含该 ID，待新列表完成布局后重新解析 ID 并滚动到对应行。重复点击不会收起窗口；合并的同文案浮窗指向最近一次对应记录。已清空或超过 500 条上限的目标只打开现有历史，不重新插入内容或按文案匹配其他消息。关闭按钮阻止点击冒泡，不触发跳转。

显式插件通知通过同一个日志消费游标取得历史身份，不重复导入。查看历史保留其原请求；关闭、取消、超时和实例回收仍由原交互拥有者处理。没有修改公开协议、权限、插件包或安装配置。

## 定向与原生验证

| 命令／场景 | 结果 |
| --- | --- |
| `cargo test -p editor-app host_messages --locked --offline -j 8 -- --test-threads=1` | 23 passed，0 failed；包括新增两行描述、旧记录定位及重复／失效目标回归 |
| `cargo test -p editor-app native_plugin_input_confirms_unicode_and_restores_editor_focus --locked --offline -j 8 -- --ignored --nocapture --test-threads=1` | 1 passed；使用账本固定的 capability-example 0.19.0 ZIP，覆盖显式通知两行预览、正文点击与请求保持 |
| `cargo test -p editor-app window_toolbar_native_windows_acceptance --locked --offline -j 8 -- --ignored --nocapture --test-threads=1` | 1 passed；父进程管理的 Windows／D3D 子进程正常完成并退出 |

新增自动回归对比单行与多行的实际布局高度，覆盖显式换行、中英文长描述、浅色 100%、深色 150% 和浅色 200%。定位测试通过原生点击激活生产卡片，检查目标行位于消息列表视口内；覆盖超过初始 20 条的旧记录、窗口已打开、重复合并、清空、容量淘汰及关闭按钮隔离。

原生场景在浅色 100% 和深色 150% 的真实 Windows 窗口中发布一条四行目标消息，再接收 45 条较新的信息消息；点击卡片后打开历史并滚动到旧目标。已查看两组绘制截图，浮窗显示两行，消息窗口显示完整四行；关闭浮窗后消息窗口保持打开。此次使用隔离临时工作区和 profile，不操作用户已有窗口和安装数据。

本机外部证据目录为 `C:/Users/Tmiracle/.codex/task-evidence/message-toast-navigation-20261030/`。日志为 `host-messages-rerun.log`、`plugin-notice.log`、`native.log`；原生截图为 `native/{light-100,dark-150}-message-{preview,revealed}.png`，DPI 元数据和子进程日志在同目录。首轮三项编译适配错误已修正，原始输出保留在 `host-messages.log`；不计为通过。

## 仓库检查与交付

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked --offline -j 8` | 127 组，873 passed、0 failed、446 ignored |
| `cargo check --workspace --locked --offline -j 8` | 通过；保留当前仓库的未使用代码等警告 |
| 本次三个文档的相对链接检查 | 106 个有效，无失效链接 |

构建使用本任务 worktree 自身的 `target/`；未共享主项目或其他源码快照的编译输出。完整日志为同一外部证据目录下的 `workspace-test.log` 与 `workspace-check.log`。

本次不生成安装包、不推送远端，不将其余 ignored 测试计为通过。
