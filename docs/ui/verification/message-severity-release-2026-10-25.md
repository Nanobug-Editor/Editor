# 主分支 Release：仅警告时的橙点实测

日期：2026-10-25。依据：用户要求在主分支重新构建 Release，并用实际程序验证只有警告时显示橙点。

后续修正（2026-10-26）：用户新截图确认当时的 Warning 色 `#ea580c` 在小圆点上仍显得偏红。本记录保留当时的等级映射与新旧产物对照结果，但不再把它作为警告与危险颜色充分可辨的验收依据；当前配色与顶部边框修正见[后续验收](message-severity-appearance-2026-10-26.md)。

## 结论

当前 `main` 工作区重新生成的 `target/release/editor-app.exe` 已通过真实桌面交互验收：先确认已有提醒，再通过未保存文件的关闭保护产生 Warning，消息按钮显示橙点；消息列表没有 Error。打开消息窗口后，已有提示点消失，警告记录保留。

本轮也运行了构建前备份的旧 EXE，在同一配置、同一文本和相同操作下同样显示橙点。因此不能把之前报告的红点归因于“主分支漏合并”或“旧 EXE 必然将警告绘制为红色”；那个具体红点现场仍未复现。

## 构建与产物

- 工作区：`C:/Projects/RustProjects/Editor`，分支 `main`，提交 `ffd9efbe5176a23d6caf4424c83903f6d409f64e`。
- 这是当前 main checkout 的构建，包含工作区原有的未提交改动；没有把它称为干净的 HEAD 快照，也没有还原、提交或混入其他任务的改动。
- Cargo metadata 核对的输出目录为该 checkout 的 `target/`；未共用其他源码副本的输出目录。
- 消息三色生产源码本轮未修改。`editor-ui/src/lib.rs` 和 `editor-app/src/main.rs` 仅更新时间戳以强制 Cargo 重编译，操作前后 SHA-256 相同，记录在本机 `source-invalidation.json`。

实际构建命令：

```powershell
cargo build -p editor-app --release --jobs 2
```

结果：exit 0，3 分 02 秒。日志明确包含本工作区 `editor-ui`、`editor-app` 的 `Compiling`，以及 `Finished release profile [optimized]`；既有 unused 与 Wasmtime／Tree-sitter 链接器警告保留。

| 项目 | 构建前备份 | 本轮重新构建 |
| --- | --- | --- |
| 文件时间 | 2026-10-25 14:17:00 | 2026-10-25 15:32:35 |
| 大小 | 89,072,128 字节 | 89,072,128 字节 |
| SHA-256 | `70938E56C81F03547095183D2D860A24B8D54785C1FB3E6CFA6E5A711FAD7B9B` | `E64C46ADC8BAC10A5E1DB5D8016C4045EC8C436ED7CCF3F16C5344C2F9AEAC63` |

最初尝试 `cargo clean --release --target-dir C:/Projects/RustProjects/Editor/target -p editor-app -p editor-ui`，Cargo 因 `CACHEDIR.TAG` 缺失或无效而拒绝清理。没有绕过该删除保护；之后通过入口文件时间戳失效缓存，再执行上述正常构建命令。没有删除源文件或清空整个 target。

## 真实 Release 交互

本轮运行的是上述实际 EXE，不是 Debug、测试程序或独立控件样例。程序通过既有 `--profile` 入口使用前轮已由用户选择模式的隔离配置；不访问用户源文件进行输入、保存或关闭。

1. 打开本轮持有的临时文件 `target/message-severity-release-2026-10-25/workspace/severity-probe.txt`。
2. 检查启动消息：仅有一条“此文件未记录历史（命中历史排除规则）”Info，没有 Error。打开消息窗口确认该信息，再收起窗口；提示点消失。
3. 在临时文本输入一个 `x`，点击文件标签的关闭按钮。真实关闭保护阻止关闭，并发布“关闭此标签页前请先保存更改”，实际消息等级显示为“警告”。
4. 消息窗口尚未打开时，实际按钮出现橙点。随后打开消息列表核对：一条新的 Warning、一条已确认的旧 Info，没有 Error；唯一新增未读等级是 Warning。这里没有把“只有警告”表述为历史列表中不存在已读 Info。
5. 待界面更新稳定，打开操作确认提醒，提示点消失；消息记录仍在。
6. 撤销临时字符并保存恢复后的原文。文件前后 SHA-256 均为 `BCDDF1286F5F95F3C2B208173CE365EB23D4A6169441DE087197C71E532FCCF0`。

输入来自 Computer Use 的实际桌面鼠标／键盘操作，不是直接调用 `MessagePanel::push` 或 GPUI 测试事件。系统标题栏关闭按钮没有缓存的可点击 bounds，未将该操作记作成功；诊断结束时仅清理已核对 EXE 和隔离 profile 归属的诊断进程。

旧产物对照使用同一个临时文件和 profile，在独立进程运行 `previous-editor-app.exe`，先确认启动 Info，再输入 `x` 并点击标签关闭按钮；也观察到橙点和 Warning，无 Error。对照 EXE 没有覆盖本轮生成的正式 Release。

## 证据与范围

本机证据目录：`target/message-severity-release-rebuild-2026-10-25/`。

- `artifact.json`、`launch.json`：新 EXE 的提交、路径、SHA-256 和进程归属。
- `workspace-before.txt`、`source-invalidation.json`：构建前已有改动清单及内容未变的时间戳操作。
- `clean.log`、`build.log`：被 Cargo 拒绝的清理和成功的真实 Release 构建。
- `warning-only-dot.png`、`warning-message-list.png`：新 Release 的橙点及实际警告列表。
- `previous-warning-only-dot.png`、`previous-warning-message-list.png`：旧 EXE 的同场景对照。
- `verification.json`：观察结果、消息等级、文件恢复及两份产物的标识。颜色为桌面截图目视核对结果，没有伪称为程序内部状态采集或像素 RGB 自动断言。
- `verified-editor-app.exe`：已实测的新 EXE 固定副本；正式产物仍为 `target/release/editor-app.exe`。

本轮没有产品源码行为修复，没有复跑并宣称全 workspace 测试通过。新增证据是 Release 重新编译和当前浅色主题下的真实 GUI 场景；[2026-10-24 三色验收](message-severity-2026-10-24.md)仍保留其当时的测试范围。

返回[原生 UI 目录](../README.md)。
