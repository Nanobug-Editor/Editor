# 03：Windows 工作区事务原生验收

日期：2026-10-11。议题：[#95](https://github.com/T-miracle/Nanobug/issues/95)。状态：本单新增流程的 C03 原生验收已完成；整合、最终审查汇总、推送与议题关闭由根代理处理。本记录不表示既有 Settings 窗口退役缺陷已经修复。

对应[主验收记录](03-workspace-transactions.md)与[已批准工单](../../tickets/plugin-api-community-foundation/03-workspace-transactions.md)。实体界面、按键与焦点结果来自根代理在本对话中的原生工具操作和回读；没有另存截图或 summary 文件。宿主日志、包、组件和文件指纹可从下述自有路径读回。本次补记只修改验收 Markdown，没有修改冻结源码、SDK、包或原生夹具，没有再次运行 Cargo。

## 同一实例的冻结输入

生产源码固定点为 `0318970594dbb99e7fdf784de803d4d6bc9b1c3b`，原候选文档点为 `bc381adf3125eb3455d646eafb0aeeba519173f2`；两点的 `crates/`、`plugins/` 差异为 0。实际 SDK 指纹为 `bdb62a0a048bc64e171bc84e59170a1887106592c1ad4e49ff2258e5e89fe943`，40 项源码资源、0 Markdown，与协议源码逐项差异 0。

验收根目录为 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-transactions/native-qa-03`，工作区为其 `workspace/`，最终实际 profile 为 `profile-final/`。公开 `--plugin-dev` 只运行一个主项目 `workspace-scaffold-project-final/`，正式 Converter 作为 bundled 包进入同一 Manager；两个消费者在同一个 profile 中执行。开发项目仅从下表正常 release ZIP 复制清单与已编译组件，不重编 WASM。历史双项目 dev 启动被 CLI 拒绝、两独立 profile 的准备方案均不作为最终成功路径。

| 实际输入 | 字节 / SHA-256 |
| --- | --- |
| `bin/editor-app-transactions-final.exe` | 187670016 / `074D7B4415A3B6CE8374A2EF8F0AE9F212145E5EAD58B14C86D4C6AFD7630D00` |
| `workspace-scaffold 0.1.0` 正式 ZIP | 477430 / `206C5334E7DBA07B8337D2E977620B92E608F9EFE69B8EEBD6CA38105695DAE7` |
| `workspace-scaffold-project-final/workspace-scaffold.wasm` | 2594302 / `398384CF18DFC7DD586D76B67FAD3A018E2A748D0E0BBC6266BAB5FE0E7E9DC7` |
| `workspace-converter 0.1.0` 正式 ZIP | 479422 / `DF243CC4F0F51CA93B222F4E63FE5B3A4047389E762406A79F1C97C2F152229A` |
| `workspace-converter-project-final/workspace-converter.wasm` | 2596178 / `70016C867DB4EAA04A2FE47B80C626ABBE06E572EA7ADAA6DACC543B4FC2FD61` |

正式 ZIP 位于本单工作树的 `Editor/target/community-transactions/`。上述原生组件的字节与正式 ZIP 内组件一致。六项已批准权限为 `workspace.read`、`workspace.write`、`editor.read`、`editor.write`、`files.select`、`ui.panels`；正常调用没有另授项目或全局权限。

## 夹具与核心流程

根代理独占的四份初始文件均为严格 UTF-8、无 BOM、CRLF。`community-generated/` 与报告最初不存在。

| 文件 | 初始字节 / SHA-256 |
| --- | --- |
| `community-live.txt`，内容 `旧A 中文🙂\r\n中段\r\n旧B\r\n` | 31 / `9A7CCBF9915781A0C4657DC2F57679322F2F3CE5936B1EB059B2F44645E2670F` |
| `community-disk.txt`，内容 `磁盘旧A🙂\r\n旧B\r\n` | 22 / `67D17BE55074A085A1FDB8825DB6B25D72B93247EBFCAB9847045C8E0BA40A25` |
| `community-template.txt`，内容 `模板 中文🙂\r\n` | 19 / `88764D10D0131C34FAA22B29936C9F8D4DE5499587FC7FB8F0FF1F90581AE0AA` |
| `community-remove.txt`，内容 `可删除原内容\r\n` | 20 / `7F9C8EC3EFB24DCD3CE43E86ACCE82F9CA1193ADF68B8BED9456A4A9EC190744` |

| 实际操作 | 可观察结果 |
| --- | --- |
| 打开 `community-live.txt`，末尾追加 `未保存 中文🙂`，保持 dirty；`community-disk.txt` 保持未打开 | live 的磁盘仍为初始 31 字节；后续计划读取包含中文与非 BMP 字符的当前原生缓冲区 |
| 脚手架菜单生成八步原生预览，按 Escape 取消 | 预览真实出现，取消后文件没有变化，生成目录不存在；桌面没有执行删除步骤 |
| Converter 的 `Convert workspace text / 转换工作区文本` 菜单生成两步预览 | 一个原生文档 Edit 与一个未打开磁盘 Write；template/remove 不包含转换词，没有进入修改计划 |
| 明确点击应用计划 | receipt 为完整完成 2/2；live 可见 `转换A 中文🙂\r\n中段\r\n转换B\r\n未保存 中文🙂`，保持未保存；live 磁盘不变；disk 实际变为 `磁盘转换A🙂\r\n转换B\r\n` |
| 一次整组 Undo | 同一会话的 live 恢复包含未保存追加内容的原始文本；disk 恢复初始内容；四份原始文件磁盘指纹不变 |
| Converter 的 `Export conversion report / 导出转换报告`，在 Native Save 中选择完整目标路径，再确认一项 Write 计划 | 选择完成后、Apply 前报告仍不存在；Apply 后文件真实出现，严格 UTF-8 无 BOM，30 字节；没有把选择对话框本身计为写入完成 |

Save 的精确目标为 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-transactions/native-qa-03/workspace/conversion-report.txt`，内容为 `Conversion report 中文🙂\r\n`，SHA-256 为 `C1131E861681B97E3F21EA27395B9BBE18AE10DD0A18BBE55C2659C412082856`。报告在最后停止后继续保留；当前静态文件回读确认字节、严格 UTF-8 解码、无 BOM 和指纹。外部 selection 的读写限制与失败矩阵由主记录中的真实包测试覆盖，本次桌面成功保存不替代那些权限负测。

## Dark、字号、键盘与正常停止

核心流程完成后，第一次在 Settings 切换 Dark 的画面成功，随后关闭 Settings 并刷新 UIA 时宿主崩溃，具体限制见下一节。同一冻结 EXE、两个组件和 `profile-final/` 再启动后，窗口 `1641168` 的主题回到 Light；主题与字号是当次内存状态，不能写成重启已持久化。

恢复场景里一次 `ctrl-alt-t` 没有切换主题，该次不计通过。根代理随后用已有 Settings 按钮完成 Light → Dark，Main 和 Settings 的实际画面均变色；Editor 页字号通过两次实际点击 `+` 从 14 → 15 → 16。这次 Settings 只用 `include_text:false` 的截图观察，关闭 X 后返回 Main，再对 Main 与新增预览使用完整 UIA。该操作减少查询退役 Settings 的时序竞争，是验收操作缓解，没有关闭宿主可访问性或宣称修复崩溃。

同一恢复实例的新脚手架预览在 Dark、16px 下显示完整八步，操作按钮没有截断。实体完整 UIA 当前帧读回标题 `105 Scaffold workspace / 生成工作区`、`106 取消`、`107 应用计划`。Tab 的稳定焦点先到取消，再到应用；第二次 Tab 曾收到工具的 `user input detected`，根代理按工具提示重新观察后只重试一次，并再次读回焦点确实为 107。Escape 取消后回到脚手架菜单按钮 36 的焦点，receipt 为 `cancelled`、`completed: []`、`stopped_at: 0`、`affected: []`。四份原文件与已有报告的指纹均未变化，生成目录仍不存在。

本次实体 UI 使用中文操作名称和中英组合标题。英文 Cancel/Apply 名称已有当前 SDK 的实际 GPUI/Base 绘制属性证据，见主记录；没有执行实体 UI locale 切换，不能把英文实体界面计为本次操作结果。

根代理最后通过控制器会话 `99440` 的正常 `stop` 结束实例，控制器返回 0；Sky 的所属窗口列表为 `[]`，精确所属 EXE 的进程数为 0。`controller-final-resume.log` 保存正常 admitted/activated 消息，无 panic；正常停止的退出码、窗口与进程结果来自本对话的控制器和原生工具回读。重启后的新预览、相关 Dark/字号布局与正常停止这三项 C03 待项至此结清。

## 既有 Settings 退役窗口限制与基线复现

本单第一次核心流程之后的崩溃日志为验收根的 `controller-final.log`。第 6–7 行首因是 `accesskit_windows-0.34.0/src/util.rs:380:52` 对失效 HWND 调用 `ClientToScreen` 后 `unwrap`，HRESULT 为 `0x80070578`；随后 COM 回调中的 panic 无法 unwind，整个宿主退出 `3221226505`，当时所属窗口与进程均为 0。后续 `WindowPattern_WaitForInputIdle` 和 UIA property event 栈是该窗口退役路径的证据，不是事务或 WASM 返回的失败结果。

根代理另用 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/native-retirement-baseline` 复现。基线宿主 `bin/editor-app-baseline.exe` 来自 `de5397645d05c5d91e8cd74108d46ad9d7a88ff3` 的已验证 38 项源码 SDK 输入，185549312 字节，SHA-256 为 `50D275863FFB1FB152A908AAE082E93DE898A95A31AA97ABC78E156A1D3D6E28`。公开单项目 `--plugin-dev` 使用新的 profile、空工作区和 `accessibility-baseline` 声明式资源包；该包仅有 `plugin.toml` 元数据，没有组件、命令、面板、权限或可见业务贡献，没有本单事务或两个本单 WASM。启动准备的 shipped JavaScript 属于既有基线资源。

基线在稳定布局下打开 Settings，点击当帧 X 后立即 UIA/截图刷新；Main 短暂出现后同样崩溃。其 `controller.log` 第 6–7 行与本单完全相同：`ClientToScreen`、`util.rs:380:52`、`HRESULT 0x80070578`，继而 non-unwinding panic，所属进程数为 0。基线再次实跑证明本单事务和两个消费者不是复现所需条件，排除 03 生产变动首次引入该崩溃；不代表上游问题已修复。

已安装源码给出对应生命周期：AccessKit `node.rs:1109–1121` 的几何 COM 回调通过 `context.rs:72` 进入 `util.rs:376–381`；GPUI `gpui-pre-windows-0.3.7/src/window.rs:608–627` 异步销毁窗口，其 `events.rs:343–365` 处理 modal 关闭和父窗口焦点。基线与本单的 Settings/Dialog 关闭代码、AccessKit/GPUI 版本均相同。本轮没有升级依赖、fork GPUI Kit、修改关闭行为或关闭可访问性；同一冻结输入下完成的新增预览完整 UIA 及正常停止结果按上文保留。
