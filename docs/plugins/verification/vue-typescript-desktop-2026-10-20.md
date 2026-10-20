# Vue / TypeScript 普通 Windows 桌面验收

> 最新：约定的语言／编辑可见操作及最后的配色、键盘重开修复均已取得实证。宿主既有 AccessKit 窗口退役异常单列保留，不将功能截图冒称为退出通过。最终双包和三仓交付另见交付账本；下文是逐时点过程记录。

2026-10-20，实际普通宿主，不以 TestAppContext 代替。本页首轮主验收启动记录 `target/s6-desktop-hover-green-launch.json`：normal SHA-256 `37fd31356ca8445b73326cb0fb8d841c9e7e8af8889cecfd87ee148f8c518aad`，匹配 PDB `e96d6ee990f6822dd7e18dc9df65c57036de60f0d125832817d3775cf061d50c`；PID 60348。最终宿主与新增修复见末节，不把早期启动称为最新构建。使用成功 S1 的既有 private profile `target/s6-native-ui/profiles/s1-verified-Im6Nju`，原项目 `C:/Projects/RustProjects/Vue-for-Nanobug`；仅沿用匹配的已保存信任，未操作安全提示或改写信任记录。

输入插件为 pending 元数据桌面候选：TypeScript `.15` SHA `422074c65d63d89b2f8b060feb15ceaaffaf4b7a3c77b19f6c6c665ebdb39f43`，Vue `.12` SHA `058cb1eec9dbef169aba8095c2e94c0f29935b845735d1c7488108e8a02e4a5e`，SDK `4af726e9a018be16dd8062f3d90ae443c8a655f8d5ce42341c1882983e2896bb`。候选与先前 `.14/.11` 及后续支持提升包分别记证。

## 修复前后身份与自动回归

普通旧 normal `996bf4…8448` 的两次 Ctrl+I 真实崩溃及匹配符号栈保留；新增真实三版本暖缓存回归 **RED 116.40s → GREEN 97.62s**。共享源关闭／同路径重开 **PASS 26.25s**，普通 hover **12 PASS / 0 FAIL / 0 ignored**；两轴增量各 0 项可行动发现。详情、首次误选 fixture 的前置失败及完整输入身份见[恢复记录](vue-typescript-resumed-2026-10-20.md)。

新 normal 直接构建 **PASS 58.30s**。随后 `cargo fmt --check`、`cargo test --workspace --exclude editor-app -j 1`（319 PASS / 0 FAIL / 327 ignored、85 suites）和 `cargo check --workspace -j 1`（3.52s）均实际 exit0；日志 `target/s6-cached-hover-final-{fmt,non-ui,check}.log`。ignored 并未据此全部通过。

## 可见操作日志

15:55 启动后恢复三个 App.vue 页签及原树目录；Vue 3 的 script setup / TS 字符串与模板已有可见不同颜色。点击声明 `message`，原生光标落在第 3 行第 13 列；仍显示插件加载中，尚不将静态颜色当成语义结果。后续每项只按实际观察追加。

服务就绪后实际显示 `const message: "hello"` 与原始 `tests/projects/vue3/App.vue:3` 定义片段。Escape 后保持源不变，再按 Ctrl+I，可再次显示同一正缓存详情；随后鼠标移动到组件，实际显示 `import Child` 及原始 `Child.vue:1` 片段，进程持续响应，stderr 无 panic。最初异步截图尚无卡片的帧没有算作成功，等待实际可见内容后才记证。

`message → desktopMessage` 的真实 F2 输入与应用同步改到声明、模板两处，均仍在未保存 EditorState；一次 Ctrl+Z 将两处恢复为 `message`。模板内 F12 实际选中第 3 行声明；Shift+F12 弹窗显示 `tests/projects/vue3/App.vue:3:7` 与 `:5:26`，来源为 TypeScript。关闭引用后编辑焦点恢复；组件 F12 又打开原始 `tests/projects/vue3/Child.vue`，没有暴露生成文件 URI。

当前已对 Child 的 `title` 发起 `desktopTitle` 重命名请求，正在等待真实跨文件计划；未先记为预览或应用通过。

随后实际预览列出 App.vue 的 `title → desktopTitle` 和 Child.vue 的类型／模板块，两个原始路径各一项操作。第一次取消后 Child 声明和模板均保留 `title`。第二次同计划在深色主题显示完整差异；点击应用得到 `已完成 2 / 2 项操作`，Child 两处与 App 消费属性均实际改成 desktopTitle，未保存。分别在 App、Child 使用 Ctrl+Z 均恢复各自原文；这属于各文件原生 Undo，不能据此冒称一次整组 Undo。正在另起完整事务验证独立的“撤销整组操作”入口。

新 normal 再次通过公开 `--export-plugin-sdk` 导出，54 文件／0 Markdown，与旧 SDK 逐文件 0 差异，仍为 `4af726e9…96bb`；实际收据 `target/s6-cached-hover-sdk-proof.json`。生产修复未改变 SDK 内容。

独立第三次 `title → groupTitle` 计划实际预览两文件并完成 2/2；直接点击本次新增的“撤销整组操作”（可访问按钮 1165）后，回执变为 `已整组撤销 / 已完成 2 / 2 项操作`。Child 类型及模板恢复 title，切换 App 后消费属性也恢复 title；期间没有在两文件中先做本地 Undo。该观察与前一次各文件 Undo 分开。

在 Vue 3 script setup 原生新行输入 `message.toU`，实际出现 1 个错误；鼠标说明为 TypeScript / 第 4 行第 9 列 / 2339，`Property 'toU' does not exist on type '"hello"'`。关闭说明后调出补全；原生字母键更新前缀后，真实列表显示 toUpperCase 与 toLocaleUpperCase，已按 Enter 选取首项，等待写回和诊断清除。批量文本输入与 Ctrl+Space 的早期空帧未算作补全成功；只以实际列表和随后文本为证。

实际补全写回 `message.toUpperCase`，2339 下划线和问题计数清除。只读核对默认键表没有注册 Ctrl+Space；当前入口是单字母／匹配 trigger 自动补全，因此早期批量 paste 不触发符合策略，未为此改生产代码。SFC 末尾加入 CSS `.card{color:red;padding:0  2px}`，Shift+Alt+F 后脚本补分号、引号统一，CSS 展开成独立属性并规范为 `padding: 0 2px;`；script/template/CSS 块均有可见高亮，内容仍未保存。

真实 OS 中文输入法：先保留字面前缀 `<!-- ni`，切换当前输入模式后逐键输入 n、i；原生候选窗实际显示第 1 项“你”，位于组合光标下方。空格提交后为 `<!-- ni你`，仅一个汉字，光标列 9。再逐键 n、i 显示新的组合及候选，Escape 后仍为 `<!-- ni你`，列 9、无末尾拼音残留；一次 Ctrl+Z 撤销该汉字回到 `<!-- ni`、列 8。此证据来自真实候选与键入，未使用 Unicode paste 代替 composition。随后恢复本窗口英文模式，补齐注释并添加临时滚动行。

滚动实际将顶行从 1 移至 16，行 16 正确显示 `native scroll 3`；设置 → 编辑器设置将共享字号从 14 改为 15 px，原生视口和控件同步变大。关闭设置后滚回顶部，在 15 px 的模板 message 上 F12 仍选中原始第 3 行声明（列 7），未因旧几何命中错误符号。

该阶段也稳定复现深色悬浮的白底浅字，不能据此标记深色悬浮通过。只读定位为本地 card 固定白底与 Markdown 当前主题前景冲突；已补实际 painted-quad 深浅主题／三种文档形式回归，生产修复尚未应用。其它已观察操作按当前 37fd 身份保留；配色修复后需重新构建和普通桌面针对性复验。

## 待完成门槛

17:20 前后补齐两个普通窗口场景。树默认隐藏 target，故在原已信任工作区中创建独立临时目录 `acceptance-desktop/`，六文件摘要登记在 `target/s6-desktop-visible-fixture.json`，未修改原项目配置。A/use.ts 中逐键触发 `desktopTota` 后真实候选为 `desktopTotal`；Enter 异步 resolve 后，正文变为 desktopTotal，顶部同时出现 `import { desktopTotal } from "./math";`。一次 Ctrl+Z 同时删除导入并恢复前缀 desktopTota。早期帧尚未写回不计为完成。

共享源实际列出 A、B 两个经编译器确认的配置根；前两次弹窗消失且没有偏好收据，不计为选择成功。重新发起后立即点击 B 行，生成 revision=1、source=`acceptance-desktop/shared.ts` 的私有偏好。其 project=`typescript:9baffd8e30f9150412d4e72bd34d0d3127bd7f31e8e1893480b3789deff3245b` 与 B 的规范化配置身份精确匹配，见 `target/s6-desktop-context-choice-proof.json`。关闭再打开共享文件后没有再次请求选择；实际原生“插件管理 → TypeScript → 运行日志”显示 B 根、analysis/engine 阶段、语言服务准备完成及进程启动，页头为插件 0.1.15。页头插件版本不冒充引擎版本；完整引擎状态及故障恢复沿用已记录的专项证据。

曾在原 Vue 3 fixture 尝试 nextTick 库导入而未出现候选，已恢复原文，不将其记为通过或扩张验收范围。Vue SFC 自动导入的正式证据仍为 E41 八行真实宿主矩阵，普通窗口补充上述本地 TS 导入。

已实际完成的暖缓存、语言操作、事务、格式化、项目选择、原生中文 IME、滚动与字号观察按上文保留。下文继续记录配色复验与新发现的键盘重开问题，未据早期空帧或局部通过关闭整个门槛。尚未完成最终包、提交推送及工单关闭。

## 配色回归与修后普通窗口

卡片真实绘制回归 RED：暗色正文对比度 1.31，0 PASS / 1 FAIL、0.18s，`target/s6-hover-colors-red.log`。统一使用当前主题 popover 后，普通绘制／补全组 6 PASS、1.33s，另 language::hover 1 PASS；冻结测试程序 `7ce52d3d64367884603010753a1fb5c9e649033ffeb5ac9f5e36554f3e4c6252`。normal `16bae3c6…a523` 构建 37.95s；首轮 fmt 仅新测试 tuple 换行失败，修正后 fmt、非 UI 319 PASS / 0 FAIL / 327 ignored（85 summaries）、check 2.80s 均 exit0。SDK 54 文件／0 Markdown，仍与 4af 逐字节一致，收据 `target/s6-hover-colors-sdk-and-gates-proof.json`。

16bae 的浅色正文已可读；第一次关闭设置紧接访问性读取时发生 `accesskit_windows 0.34.0/util.rs:380` 的 `ClientToScreen` 无效 HWND / COM cannot-unwind，`target/s6-hover-colors-desktop-stderr.log` 完整保留，不计该次深色通过。它与仓库已记录的[窗口退役问题](../../ui/verification/project-home.md)同签名；错误 HWND 未记录，不能断言只有自动化会触发，本轮没有升级或 fork 上游。之后用设置关闭 X 和 screenshot-only 读取，在存活实例完成深色观察。一次启动控制台遮挡由工具识别后没有代操作终端；后续用 NoNewWindow 启动原生 GPUI 窗口。

实际深色正文虽已修复，行内定义路径仍白底浅字，故补独立 inline span 回归。首次短名称配 `--exact` 仅构建并匹配 0 tests，不当作 RED。随后从实际 Running 路径冻结 `1d13fb2b…b536`，精确全名得到 0 PASS / 1 FAIL，`target/s6-hover-inline-red.log`：暗色 inline 字色为 None 而未采用当前前景。基础 Markdown 显式设置 inline glyph/fill 后，真实样式组 2 PASS、0.01s，原卡片／补全组 6 PASS、1.39s；消费者自有 document_style 后置覆盖保持。两次增量审查的 Spec / Standards 均 0 可行动发现。

最终配色 normal `a45aed2f7e8818c76ef4f0227c066fef273628192a82921f0e02d868ae75efc2`、PDB `e8e81ec396f4947190f38dde6ad904ed7c9a8f3b9b4ec38a531fd039cf7ea92c`，39.44s 构建。17:57 启动 PID 52912，原 private profile / 匹配信任不变。浅色 Ctrl+I 与改为深色后的 Ctrl+I 均实际显示清晰的 const message、原始 App.vue 路径与代码片段；深色路径已为深底浅字。源码 SHA、日志和启动回执见 `target/s6-hover-inline-{green,native}-inputs.json`、`target/s6-hover-inline-desktop-launch.json`；该时点三项仓库门禁均 exit0，check 2.66s。

配色完成后的额外对照发现：实际可见卡片 Escape 后，鼠标留在原符号时显式 Ctrl+I 没有持续显示；切 Child 再回 App、鼠标移到页签后，不改光标 Ctrl+I 立即恢复清晰卡片。stderr 空、进程响应，不能把前一空白序列计为成功。静态读取发现 Escape 的 pointer 抑制与显式键盘路径未统一，正补轻量真实按键／同符号 MouseMove／延迟结果回归，最终交付仍待该问题闭合。

六个临时文件先逐摘要核对后通过 apply_patch 删除，再只删除三个已确认的空目录；`target/s6-desktop-visible-fixture-cleanup.json` 确认临时根已不存在。27 项原项目文件磁盘摘要均与创建前一致，见 `target/s6-desktop-visible-fixture-precleanup.json`。原项目未保存本次试验内容。

## 最后键盘修复与交付冻结点

两个轻量原生路由回归分别取得真实 RED：同符号 MouseMove 清除显式卡片（0 PASS / 1 FAIL、0.22s），以及 pending 时 Escape 后迟到结果复活（0 PASS / 1 FAIL、0.15s），分别为 `target/s6-hover-reopen-red.log`、`target/s6-hover-pending-red.log`。第一次共享 generation 修复仅得到 6 PASS / 1 FAIL、1.94s，`target/s6-hover-reopen-green.log`；文件名不代表已过。无可见卡片时 Base 没有原 bubbling Escape handler，因此再让既有 shell capture 撤销 pending hover，并令重复 dismiss 保留抑制范围。

完整修复后的冻结测试程序 `beed53fdea083781c2a724e7185308430f2cbc8d44e3ae3fbb101d438ede40f1`：hover 几何／键盘组 **7 PASS / 0 FAIL / 0 ignored、1.98s**，卡片／补全组 **6 PASS、1.31s**，文字样式组 **2 PASS、0.00s**，相关 Escape 过滤组 **9 PASS / 1 ignored、3.53s**。各组有重叠，不相加冒充独立测试数；`editor::language_features` 过滤为 0 tests，不计覆盖。命令和来源摘要见 `target/s6-hover-reopen-complete-inputs.json` 及同前缀日志。

最终 normal `1cae48c982197430fbbe0daf8c0664780d0ce5a1c30999c836d2f1c71ac89f61`、PDB `b448e1cf7081edb379696747059ca2caf3ff51129d5e43677479dbc5dd23c0af`，构建 **42.94s**。最终 `cargo fmt --check`、非 UI workspace **319 PASS / 0 FAIL / 327 ignored、85 summaries**、`cargo check --workspace -j1`（**2.82s**）均实际 exit0；日志 `target/s6-final-supported-{fmt,non-ui,check}.log`。公开 SDK 再导出 **54 文件 / 0 Markdown / 0 byte differences**，仍为 `4af726e9…96bb`，`target/s6-final-supported-sdk-proof.json`。

18:23 该 normal 启动 PID 27012，使用原可信 profile / 原 Vue 工作区。真实 Ctrl+I 先显示 const message、原始 App.vue 路径及定义代码；Escape 后卡片确实消失。不点击、不换页签、不改光标、鼠标仍留 message，再按 Ctrl+I 立即重开同一清晰卡片。本序列由普通窗口实际按键和可见结果确认，未用直接 handler 或状态写入替代。颜色源码与前述 a45 深浅复验逐字节相同。

点击退出后该进程已结束，但 `target/s6-final-supported-desktop-stderr.log` 再次记录与既有基线同签名的 AccessKit `ClientToScreen` 无效 HWND / COM abort；不能声称普通退出为 exit0，也未声称本轮修复该上游问题。该限制与语言请求、事务、真实进程回收／checked shutdown 的专门回归分开。当前不扩大为 GPUI／AccessKit fork 或依赖升级。
