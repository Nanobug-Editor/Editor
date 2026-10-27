# Git 差异主题、相邻文件导航与视口列布局

日期：2026-10-27。状态：实现与本机验证完成；尚未合并、安装或发布。

宿主任务分支 `codex/git-diff-scroll-fixes`，起点 `176760ee6264ff63368689cb53037fda4a9fd35d`。独立 Git 源码位于 `C:/Projects/RustProjects/Git-for-Nanobug`；候选版本 0.1.46。主 checkout 的其他未提交修改没有复制进本任务。

## 依据与基线

本轮 Hindsight 查询成功，采用历史面板隔离记录 `4015d46f-a9e8-403d-a719-13acf75ec57c` 与比较源修复记录 `26c61d39-c756-475b-8955-ee4028bf0775`，再与当前源码核对。历史记录没有改变本轮授权。

用户实际运行 `C:/Projects/RustProjects/Editor/target/release/editor-app.exe`；安装注册表的 Git 是 0.1.45，原包摘要 `903d2ebf0b6e493dbee06c89f22c0e6e46998718d338f08825631deaea3ff505`。基线测试 ZIP 从该已安装目录只读复制并重新归档；归档摘要与原始 ZIP 不相同，内部 manifest/WASM 的身份另行记录，没有改动用户安装。

证据保存在项目外 `C:/Users/Tmiracle/.codex/task-artifacts/git-diff-scroll-2026-10-27/`；构建只使用本 worktree 的 `target/`，已核对 Cargo metadata。

## 复现与修复边界

- 差异主题：历史只读源原来传 `language: None`，宿主创建虚拟源时也未为显式语言接入 native highlighter。新增 `editor.virtual 1.2` 的可选 `OpenVirtualSource.language_path`，仅用于已注册语言提供者的文件识别，不读取文件，不建立额外源文件标签或 LSP 会话。显式 language 优先；语言提供者变化时重新识别并更新同一原生会话。
- 相邻文件：`diff_history` 原来改变提交列表的 disabled 状态，导致每个文件开始／完成时重新发布历史面板。只读文件导航保留面板及树数据；独立比较源在同次 dispatch 准备；不可变提交内容按工作区、OID 和真实路径使用有界缓存。工作区与 index 不使用此缓存。
- 底部空白：原生回归确认 180px 窗格仅有 168px 内容区，即便软换行也保留 12px 空横向轨道。无横向溢出时回收轨道，有真实横向溢出时保留滚动功能。
- 列抖动：GIF 操作的是分支树／提交列表的外层分隔条。提交列表已经使用 CanvasView；其位置先随原生视口移动，再等插件旧宽度绘图回包校正。`canvas_columns_follow_viewport_before_guest_resize_reply` 复现尾列右边缘 588px，期望 548px。新增 `ui.canvas 1.11` 可选视口弹性列声明，由原生帧按实际宽度投影文本和选中背景，保留后三列宽度与右侧位置；无需重写整个列表。

协议主版本与安装权限不变。0.1.46 需要支持上述小版本的配套宿主，旧宿主应按能力协商明确拒绝。

## 验证记录

以下仅记录已经运行的检查，不把计划和跳过项记为通过。

| 检查 | 当前结果 |
| --- | --- |
| 软换行底部轨道红灯 | 0 passed / 1 failed；实际 168px，期望 180px |
| 视口变化列投影红灯 | 0 passed / 1 failed；实际 588px，期望 548px |
| 实包相邻导航首次测试 | 夹具未准备好改动文件树，尚未进入导航；不作为行为红灯或性能数据 |
| 本地列状态针对性测试 | 4 passed / 0 failed |
| 初始 workspace 编译检查 | 通过，仅为开始修改前基线 |
| 冻结后完整 workspace 测试（排除 editor-app） | 838 passed / 0 failed / 441 ignored；ignored 项未计为通过 |
| Git 独立访客测试 | 208 passed / 0 failed / 0 ignored |
| 最终宿主列绘图测试 | 3 passed；包含窄视口、迟到回包与内部拖动 |
| 虚拟语言测试（显式 ignored 执行） | 1 passed；真实 WASM 语法、两侧只读源、深浅主题及语言提供者启停 |
| 原生差异渲染测试 | 7 passed |
| 实际 Git 候选包导航与高亮（显式 ignored 执行） | 两次完整通过；历史场景与文件树稳定、重复文件零 Git 进程、实际两侧语言和主题正确 |
| Windows 原生拖动与滚动（显式 ignored 执行） | 深色／中文／100%、浅色／英文／150% 两组通过；外分隔首帧与回包稳定、三个内部列分隔拖动及原有滚动验证通过 |
| 最终 workspace 编译检查、格式检查 | 通过 |
| 宿主 Release 构建 | 通过；没有替换用户正在运行的程序 |

首个高亮测试使用过旧的固定 WASM，因不认识 `open_virtual_source` 被拒，尚未进入目标路径；这次失败不作为高亮缺陷红灯。修正为夹具支持的虚拟文档操作后检查相同宿主创建路径，并通过当前 Git 候选额外核对真实只读源提示。

## 实包导航与原生窗口验收

`git_adjacent_file_arrows_preserve_history_scene_and_cached_sources` 使用临时三文件仓库、同一宿主测试可执行程序，记录点击至目标内容在原生编辑器中可见的时间；JIT/激活在采样之前完成。旧 0.1.45 包实际读到目标断言：5 次切换约 936 / 910 / 976 / 1008 / 990ms，三次重复文件仍启动 Git 进程，历史场景 revision 被替换。文件树节点和展开状态断言正常；失败是历史场景被替换。日志 `adjacent-native-baseline-final.log`。

候选 0.1.46 通过相同断言。使用正常隔离 runner 和私有 TEMP/TMP 捕获成功子进程日志，没有绕过隔离认证或改变 Git 配置。日志为 `adjacent-native-green.log`、`adjacent-native-green-timed.log`、`adjacent-native-green-child.log`。

| 测试环境中的可见切换 | 0.1.45 | 0.1.46 | 变化 |
| --- | --- | --- | --- |
| 首次打开文件，两次均值 | 923ms | 798ms | 减少约 13.6% |
| 返回已看文件，三次均值 | 991ms | 404ms | 减少约 59.2%，三次均无 Git 进程 |

首次读取仍用 Git 验证对象是否存在及类型，再按 OID 读取完整内容；没有依赖 stderr 猜测缺失，也没有缓存可变 index/worktree 内容。测试中的已安装语言提供者使用 `.novel` 扩展名，验证不依赖宿主写死 Rust 识别规则。

早期导航夹具缺少选中提交、临时语言包根目录不一致，以及隔离 Git 无模板导致 `.git/info` 不存在，均在进入有效采样前修正。这些失败不计为产品红灯。首次外分隔原生测试的窗口已处于列压缩状态，原“所有列屏幕 x 恒定”断言错误地排除了正常压缩；验证改为相同视口下即时原生投影与迟到回包的全部列坐标/宽度一致，再检查恢复原宽度、选择与滚动。

性能采样只覆盖测试事件处理、轮询和原生内容可见的延迟，不代表屏幕呈现帧率。

最终原生验收记录在 `native-columns-final/`。两组均实际移动分支树／提交列表分隔条，比较第一帧的列坐标与宽度和等待 worker 后同尺寸的结果，并在按住分隔条时等待其稳定。验证选择、垂直滚动、提交面板 revision 及恢复原尺寸。之后继续运行三个内部列边界拖动、滚轮像素累计／反向边界、图块复用、滚动条拖动与隐藏动画检查；临时窗口退出清理通过。主进程日志 `native-columns-final.log`，两组子目录各有 `evidence.json`、原生截图和退出清理记录。截图已人工查看，无文本重叠。

## 候选交付物

`delivery/Nanobug.exe` 为此 worktree 的 Release 宿主，SHA256 `8630AE9DB177D677B7FBA4E7E9E7F39D682A08FFF2620D039411735720DC834F`；`delivery/git-0.1.46.zip` 的 SHA256 为 `B99DBC2925CB766B318CECA7490EC9E0D7A36D7C2CBF06C2419D5A147CE6F0BD`。路径相对于上述项目外证据目录，`delivery/sha256.json` 保存机器可读摘要。

两者须配套验证；未替换用户安装、未合并主 checkout、未提交或发布。候选宿主不含主 checkout 其他任务的未提交改动。
