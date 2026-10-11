# 05：Windows 原生树与文档装饰

日期：2026-10-11。状态：首轮真实操作发现两处缺陷，修复后的当前宿主仍待原生复验；不据本记录关闭 #97。

## 首轮固定输入

- 源码 `7ab592878ed71943479a9e20e4423abb27249354`，后续纯文档提交 `d9b06d4b30da398fec681f073ff47ee7f9bd7aca` 不改变这些构建输入。
- 自有 QA 目录 `C:/Users/Tmiracle/.codex/worktrees/plugin-api-trees/native-qa-05`，隔离工作区 `workspace/`、profile `profile-final/`；原项目工作区及其它应用未操作。
- 宿主 `bin/editor-app-trees-final.exe` SHA256 `27189941951236959DC137B44F5AF0F58468A5DE24316B1E31CE15B07CB9ED20`。
- 独立源码 SDK key `93226c2989d2714785bb6d2b9562b5c4afa98e3e191417df8d2fbd127dc34ae2`，41 个源码/Cargo/WIT/JSON 文件、0 Markdown；数据库、Todo、书签、覆盖率均使用主验收记录的正式 0.1.1 release ZIP/WASM。
- 数据库静态开发项目 `database-project-final/` 从同一正式 ZIP 解包，`nanobug-plugin.json` 直接打包现有 WASM，无新编译或仓库路径 SDK。使用宿主公开 `--plugin-dev`、`--profile`、`--workspace` 入口，数据库仅显式授权 `ui.panels`。
- `workspace/community-markers.txt` 为 204 行 UTF-8、CRLF，无 BOM，6481 字节；初始 SHA256 `6DE6707F787B5C592502CD62AB0343F6956C18FD7F474E1AF8088FD4844D0BD0`。
- 所有操作通过 `@oai/sky` 读取实际窗口、截图/UIA 后输入；窗口 `22677346`、控制器 session `13578` 属于本轮。临时窗口标识不用于后续重放。

## 已实际观察的行为

1. 数据库目录通过真实选择、Right/Down 展开 `main`、`users (512)`。错误分支显示服务暂不可用；选择 Retry 后 Return 才恢复，不自动重试。恢复节点的激活、查看详情、查看结构分别显示 `recovered:summary`、`recovered:details`、`recovered:schema`，同一命令的不同参数保持正确。
2. 延迟分支展开后显示 Loading；折叠后完成旧请求，显示 `StaleRevision` 且不插入旧子节点。重新展开并完成当前请求，显示 `Delayed child`。用户分页通过实际 More 行与 Return 加载到 User 031；实际 UIA 只出现有限可见节点。512 项完整加载上限沿用同输入的真实 GPUI/WASM 测试，不宣称手工遍历 32 页。
3. Todo 树显示两行中文/emoji 文件位置；选中第二项 Return，编辑器 caret 与状态栏定位到 2,1，随后显示“已定位待办”。前缀插入后位置更新至第 2/3 行；文档关闭后为 0 项。
4. 书签在原生编辑器 Ctrl-Home 后通过公开菜单标记 byte 0；读取命令返回 revision 0、point 0/0。覆盖率扫描返回两个区间，实际 gutter 图标及填色出现在源第 3/4 行。真实点击图标并等待 900ms 后，截图及 UIA 出现“用户书签”“已覆盖行”的工具提示；即时帧不计作 tooltip 验证。
5. 正常编辑输入 `前置 中文🙂\r\n`（实际 CRLF，19 UTF-8 字节），书签变为 revision 1、point 19/19，覆盖率与 Todo 跟随文档移动。一次原生 Ctrl-Z 恢复原文本，读取为 revision 2、书签 0/0、覆盖率 40..61 与 63..79。上述未保存输入和 Undo 均未改变磁盘 hash。
6. Settings 仅用截图操作 Dark 与字号 14→16，关闭后主窗口 UIA、文本、菜单与树正常；书签/覆盖率图标可见。未触发 Settings 的 UIA provider，未把既有 Settings HWND 退役崩溃说成已修复。正式菜单两语可读；未手工切换启动时固定的宿主语言，也未将普通中文输入称为实际 IME composition。
7. 第一次关闭文档被既有 dirty 规则阻止，消息为“关闭此标签页前请先保存更改”，不计作关闭成功。随后 Ctrl-S 保存已 Undo 的原内容，hash 仍相同；再关闭成功，三个 gutter 标签撤下，书签/覆盖率显示文档已关闭，Todo 为 0，数据库目录仍独立可用。

## 首轮未通过及真实回归

Dark、字号 16 下，编辑区真实向下滚动 820，首个可见源行为第 21 行；原第 4 行的黄色覆盖率图标仍停在首个可见行，连续稳定帧未消失。UIA 仍出现三个本应屏外的 gutter 标签。真实 GPUI 回归 `community_decorations_hide_offscreen_markers_and_restore_after_scroll` 首轮 0 passed / 1 failed / 0 ignored；固定文件收紧后再次到达滚动断言并复现同一 12×12 bounds。中间一次夹具加载失败没有记作缺陷 Red。

文档正常关闭后，控制器输入 `reload`。`controller-final.log` 第 7–9 行记录候选已准备、代次 1 已准入，随后 `operation=snapshot: InvalidHandle: Tree was revoked`，恢复又失败为 `operation=prepare: InvalidHandle: Resource owner is no longer available`。稳定原生窗口中数据库面板为空且显示一个插件错误；日志里的“previous version retained”不等于旧实例仍可使用。

公开 Manager、同一正式数据库 WASM 的最小回归 `community_database_tree_update_captures_live_snapshot_before_retirement` 稳定失败 0/1/0，40.18 秒，排除编辑器布局、其它插件和文档输入。快照前资源撤销修复后为 1/0/0，41.92 秒。另以公开 `InstallControl` 在迁移入口取消，`community_database_tree_failed_cutover_restores_fresh_owned_tree` 真实失败 0/1/0，41.12 秒；从已安装旧包重建 WASM 并恢复逻辑快照后为 1/0/0，58.19 秒。新测试纯归位的 E0583 编译失败另存日志，不记作行为 Red。

当前 marker 行范围修复的真实 GPUI Green 为 1/0/0，39.19 秒：首次可见三个图标，滚出后连续三帧无绘制/命中 bounds，滚回后恢复精确源行几何；SDK 与正式四包未改变。当前宿主 EXE、相关旧热更新回归、规定门禁及两个缺陷的物理复验仍需逐项补入，不沿用旧 EXE 的成功项目冒充新宿主全验收。

## 首轮清理

控制器正常 `stop`，退出 0；自有 EXE 窗口列表为空、精确路径进程数为 0。最终 fixture SHA256 仍为 `6DE6707F787B5C592502CD62AB0343F6956C18FD7F474E1AF8088FD4844D0BD0`。所有日志和自有数据保留，没有删除或覆盖首轮失败证据。
