# 终端首次启动空白行与滚动条修复

日期：2026-10-25（Asia/Shanghai）。状态：实现与下述验收完成；未提交或推送。完整 workspace 首轮遇到占用及依赖产物问题，遗漏目标均已单独补测，不将初轮错误或 ignored 计为通过。

## 问题与原因

新建 PowerShell 终端时，提示符前出现一行空白，只有一行提示符也显示滚动条。这两个现象来自同一行虚假的历史记录。

真实受控 PowerShell/ConPTY 启动输出已复现：首个文本位于逻辑第 2 行，历史行数为 1；原始启动字节没有换行。将记录重放到 `Engine` 仍失败，启动前历史为 0，首次清屏后变为 1。最小化为空网格执行一次 `clear_viewport`，同样得到 1 行历史。

[原网格计算](../../../crates/editor-terminal/src/grid/mod.rs)用迭代器停留的行号计算需要归档的行数；空网格遍历结束时仍停在首格，导致 `0 + 1`。后续正常绘图展示这行历史，并据此生成滚动条范围。

## 修改范围

- 网格清屏以实际找到的非空可见单元格确定归档范围；找不到则归档零行。
- 移除占位行后，扩展回归发现恢复后的宽度重排会把首行移入历史，而底部尚有空位。存盘和恢复网格内容完整，缺失发生在显示范围；复用现有空位显示逻辑，在正常重排后保持首行可见，保留手动滚动与备用屏幕策略。
- 保留真实输出、样式空格及既有历史，保留进程主动输出的换行；没有通过裁剪文字、重置历史或强制隐藏滚动条掩盖问题。
- 核心来源说明同步在 [NOTICE](../../../crates/editor-terminal/NOTICE)；不引入上游整包、路径覆盖或新依赖。

## 回归与实际结果

| 入口 | 结果 | 证据 |
| --- | --- | --- |
| 空网格清屏最小回归 | 修复前 0 passed、1 failed，历史实际为 1，预期为 0 | `target/terminal-startup-minimal-red.log` |
| ConPTY 首次重绘记录 | 修复前 0 passed、1 failed；启动前 0 行历史，重绘后 1 行，首个文本行号为 1 | `target/terminal-startup-core-red.log` |
| 恢复后可见文本回归 | 修复初始化后曾暴露首行显示缺失；最小重排回归明确得到 `history=1, offset=0`，完整文本仍在网格中 | `target/terminal-startup-visible-restore-red.log`、`target/terminal-startup-restore-diagnostic.log` |
| `cargo test -p editor-terminal --locked --offline --jobs 2` | 最终 205 passed、0 failed、1 ignored，3.62 秒；包含 45 组录制回放和零历史软换行恢复/重排/绘图验证；doc-test 入口也通过 | `target/terminal-startup-core-final.log` |
| `cargo test -p editor-app terminal:: --locked --offline --jobs 2 -- --test-threads=1` | 最终 38 passed、0 failed、5 ignored，54.58 秒；含真实 Shell 首行及历史恢复、尺寸变化、现有任务与升级回归 | `target/terminal-startup-app-verified.log` |
| 显式 `native_first_prompt_frame --exact --ignored` | 1 passed、0 failed，2.20 秒；真实 Windows 首帧、真实 PowerShell、正常打开终端动作和受控子进程清理 | `target/terminal-startup-native-delivery.log` |
| 显式真实 Neovim Unicode/窗口尺寸变化 | 1 passed、0 failed，4.29 秒；使用已安装的 Neovim，未安装工具 | `target/terminal-startup-neovim-final.log` |
| `cargo check --workspace --locked --offline --jobs 2` | 最终通过，1 分 24 秒 | `target/terminal-startup-workspace-check-final.log` |
| `cargo fmt --check` | 最终通过 | `target/terminal-startup-fmt-delivery.log` |
| `cargo test --workspace --exclude editor-app --locked --offline --jobs 2 --no-fail-fast` 与补测 | 首轮返回 101；其余组通过，11 个可执行目标因 Windows OS error 32 未执行，终端 doc-test 一次依赖解析失败。11 个目标复制到独立测试目录后全部补跑通过，终端最终单包 doc-test 也通过；合并覆盖 113 组、782 passed、0 failed、409 ignored | `target/terminal-startup-workspace-tests.log`、`target/terminal-startup-missed-targets-results.json`、`target/terminal-startup-workspace-summary.json` |

[核心回归](../../../crates/editor-terminal/src/engine/startup_tests.rs)还覆盖零历史时不同可视高度均无滚动条、清屏保留真实文本/背景色空格、显式首行换行不被删除。[宿主回归](../../../crates/editor-app/src/terminal/tests/startup.rs)从正常编辑器入口启动真实 Shell，检查提交到原生网格的首行位置、历史及滚动条。Windows 原生截图入口默认 ignored，本轮已按表中记录显式执行。

首帧截图 `target/terminal-startup-first-prompt.png` 已读回查看：提示符从第一行开始，没有顶部空白行和滚动条。逻辑几何记录为 `first_ink_y=8`、`cell_height=21`、`history=0`、`scrollbar=false`；当前 Windows 比例为 150%。产物 SHA-256 为 `6BD1FAAB7F620D189BE7E925C6FCA62057F2F44574A2A7E8D19C3FE423FAC6AB`，来源、独立临时工作区与资料目录见 `target/terminal-startup-native-delivery-artifact.json`。首帧验收使用显式打开动作；恢复布局时不自动创建或重启 Shell 的既有行为保持不变。

本轮最初宿主测试曾在共享工作区的 `editor_sources`、`EditorActionElements` 并行迁移接口处编译失败，也遇到共享测试程序占用导致的链接失败；这些尝试没有执行断言，不计为通过。等待相应改动及占用解除后，最终正常 Cargo 宿主测试入口已通过。未运行其他 ignored WASM/原生场景，也没有修改无关源码或终止其他会话的进程。
