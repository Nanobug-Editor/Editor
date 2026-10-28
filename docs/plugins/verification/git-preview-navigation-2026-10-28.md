# Git 差异预览恢复、完整文件树与边界导航

日期：2026-10-28。状态：代码、配套 Release 宿主及 Git 0.1.48 已生成，原生功能与性能回归通过；未安装到用户配置，待用户手动验证实际工作区。

## 本轮范围与来源

用户确认修复连续点击后预览不再切换、文件导航禁用错误、树选择未同步、差异边界导航、改动文件树与预览加载，以及差异标签外观。

- 宿主：`C:/Users/Tmiracle/.codex/worktrees/git-diff-scroll-fixes/Editor`，本轮基线 `066b005b2c4788419c567d5dbf06e881a7394eb9`，开始时无未提交修改。
- Git：`C:/Projects/RustProjects/Git-for-Nanobug`，仓库基线 `92ad8df`；保留此前 0.1.47 首屏修复的已有改动，本轮增加 0.1.48。
- 沿用本任务独立 worktree 与构建目录；不覆盖用户正在运行的宿主、不清理 profile、不默认生成 Windows x64 宿主 ZIP。

## 已确认的行为

1. 连续文件点击采用最后一次选择；只撤销本轮历史比较的只读请求，不取消提交面板写操作。比较成功后才同步当前提交树的选择；原预览所属提交与当前树不一致时，保留原预览序列，不重载当前提交。
2. 前后文件按完整文件树的目录优先顺序导航，不受折叠影响；显现目标时展开全部祖先、选中并滚动到可见位置，不产生第二次业务选择回调。
3. 第一／最后差异再次向外导航时，先在差异内容中心显示 2 秒提示；同向第二次点击须在这 2 秒内，才进入前一文件内容底部或后一文件内容顶部。超时重新提示，文件列表端点不循环。普通打开仍定位第一处差异。
4. 改动文件的元数据一次提交完整森林；展开目录不继续加载。文件内容仍按需读取，使用不可变 OID 缓存。等待另一提交结果时保留已显示树及布局，旧结果不能覆盖新选择。
5. 差异预览保留 preview 生命周期，使用正常标签字体与关闭按钮、双向差异图标及 `仓库差异: 文件名`／`Repository Diff: filename` 标题；修订信息移至提示文本，不在标签显示哈希后缀或眼睛图标。

## 诊断与机制

旧 0.1.47 在完整文档通知流下连续六次切换可以通过，因此不能仅凭截图断言每次失败均由同一原因触发。实际包测试通过公开 Manager 终止文档通知流后复现：已关闭虚拟源的版本仍留在插件缓存，后续请求被归属校验拒绝，编辑器停留在上一文件。旧包失败日志为 `C:/Users/Tmiracle/.codex/task-artifacts/git-startup-2026-10-28/ownership-stream-red.log`。

修复在通知流失效时清除源版本及失败回滚副本，保留冻结导航上下文；下一次显式打开可重新建立观察。失效缓存的刷新请求仅允许一次正常新建源，不放宽宿主归属校验、不无限重试。原生比较成功前的临时资源继续按原生命周期回收。

历史文件冷读原本每侧先查对象再读取，两侧共启动四次 Git 进程；现在使用一次 `git cat-file --batch -z` 读取缺失缓存的两／三侧。NUL 分隔输入保护特殊文件名，按声明字节数解析输出；完整响应验证后才更新缓存，截断、二进制、子模块和超限文本不会伪装成空白文件。

完整文件树使用现有原子 ApplyPatch，沿用 4096 节点／2 MiB 的公开树配额；超限明确失败并保留原树，不静默截断。重复选择同一提交不重启查询；新提交只淘汰尚未执行的旧详情查询，保留正在执行的只读请求并丢弃其过期结果。完整发布后的迟到分页 Request 不再生成旧页，宿主同时拒绝退休 epoch 的 Reply。

实际包进一步复现了永久加载状态：完整文件树已准入，最后一条提交详情回执却被后台静默刷新过滤，界面没有收到解除禁用的场景。重复点击还可能取消当前必需的元数据查询，再被“相同提交正在加载”去重。现在完成回执显式发布就绪状态，用户输入只取消可选后台查询，保留当前提交的必要查询；只有真正排队／执行的查询才按正在加载去重，孤立加载状态允许恢复。

就绪发布独立于随后启动的新后台观察，覆盖加载期间收到焦点／可见性观察的情况。快速 A→B→A 时，匹配 OID 的旧消息回执还必须携带当前完整文件集合，才能启用树；否则继续等待新查询，不能冒充当前树。五个直接进入序列化 dispatcher 的回归先红后绿；原生单元测试只替代进程启动传输，实际状态机及场景过滤不替换。

新增通用 `ui.tree 1.9` 的 Reveal 和 `editor.diff 1.5` 的初始内容位置／边界命令，均通过原有公开能力、版本、实例及权限准入，宿主没有 Git 标识专用分支。旧请求默认仍为首处差异，旧基础字段的线格式不变。0.1.48 显式要求上述能力版本。

头尾定位采用整个对齐显示区的范围，包含只有左／第三侧存在的开头或结尾删除行，避免把右侧字节 0／EOF 错当作完整差异区域边界。统一模式的差异跳转也使用静默居中定位，不再依靠移动光标使目标勉强可见。

## 性能样本

本机 Git 2.53.0.windows.2，对主项目 HEAD 与 parent 的 `crates/editor-app/src/extensions/worker.rs` 做五次只读比较；新旧输出逐字节一致。

| 读取方式 | 五次耗时（毫秒） |
| --- | --- |
| 旧四进程 | 240.95、237.66、244.96、241.68、231.76 |
| 新单进程 batch | 61.72、58.53、62.44、57.46、59.77 |

这是 Git 内容读取阶段的样本，不是完整 UI 打开延迟或任意规模仓库的保证。

最终真实插件包与原生窗口的三文件夹具，测量点击至目标内容可见；采样时无并行编译，不计首次 WASM 准备时间：

| 导航目标 | 内容缓存 | 可见延迟（毫秒） | 是否启动 Git |
| --- | --- | --- | --- |
| b.novel | 未命中 | 490.8729 | 是 |
| c.novel | 未命中 | 475.0627 | 是 |
| b.novel | 命中 | 430.5539 | 否 |
| a.novel | 命中 | 374.1204 | 否 |
| b.novel | 命中 | 372.5581 | 否 |

同时断言 Git 历史面板场景 revision 未变化、树 epoch／节点／分支／展开状态未重建；深浅主题的实际插件语法高亮和禁用后的比较视图清理通过。此数据是本机夹具样本，不等于大型仓库全部场景或冷启动耗时保证。完整样本记录为 `git-navigation-adjacent-native-child-final.log`。

## 验证记录

- Git 最终全量 228 项测试通过（5.13 秒），包含完整森林、重复选择、最新查询优先、源失效恢复、batch 内容边界、导航位置及就绪场景发布；日志 `git-startup-2026-10-28/git-navigation-ready-race-full.log`。新增 dispatch 回归的失败与成功证据分别为同目录 `git-navigation-ready-red.log`、`git-navigation-ready-green.log` 和 `git-navigation-ready-race-red.log`、`git-navigation-ready-race-green.log`。插件 `cargo fmt --check`、`git diff --check` 通过。
- 通用 Reveal 三项、能力版本门禁一项，以及固定 WASM 包的公开树准入测试通过；覆盖完整路径校验、归属与权限、完整发布后迟到 Reply 拒绝及展开无新 Request。
- 比较默认线格式一项、运行时比较八项、原生 DirectWrite 标签字体一项通过；字体覆盖深浅主题与 100%／125%／150% 缩放。日志 `git-navigation-protocol-tests.log`、`git-navigation-runtime-tests.log`、`git-navigation-tab-tests.log`。
- 新 debug 宿主编译通过，使用其内嵌 SDK 构建 Git 0.1.48 成功；日志 `git-navigation-2026-10-28-host-build.log`、`git-navigation-2026-10-28-package.log`。
- 最终工作区以 `cargo test --workspace --exclude editor-app --jobs 1` 通过 848 项、跳过 445 项（127 个结果组），包含后续新增的对齐边缘定位；日志 `git-navigation-2026-10-28-workspace-tests-final.log`。`cargo fmt --check`、`cargo check --workspace --jobs 1` 通过，日志 `git-navigation-2026-10-28-fmt-final.log`、`git-navigation-2026-10-28-workspace-check-final.log`。
- 完整内容边缘两项原生几何回归通过（0.27 秒），覆盖双栏／三栏、只有其他侧存在的头尾删除、无末尾换行、折叠与展开及选择不变；日志 `git-navigation-2026-10-28-content-edges.log`。独立视口底部定位与严格 2 秒边界确认各一项通过，日志 `git-navigation-bottom-locator.log`、`git-navigation-boundary-confirmation.log`。
- 最终 b51 包通过相同冻结 V2 的实际包原生 ownership 测试（1 项，显式 `--ignored`，80.66 秒包含冷 WASM 准备）：连续 6 次逐文件打开、4 次快速选择以最后目标为准、通过公开 Manager 终止文档通知流后再切换 4 次，均检查实际原生文本和单一比较视图。结尾 disable 与 flush 成功；未额外断言隐藏源数量，不能将此项扩述为资源数量测量。日志 `git-startup-2026-10-28/ownership-native-green.log`。
- 实际包双栏／三栏居中导航通过：首处差异、上／下一处差异居中，源文本和选择不变，以及没有可折叠项时的禁用状态；日志 `git-navigation-native-final.log` 中该项为通过。同次运行的边界项暴露的是 fixture 把 `nested/c.txt` 错当作 `nested/beta` 的子项，错误要求展开其兄弟目录；目标 b 的 nested 与 beta 展开、c 的 nested 展开及选中已经通过，修正额外错误断言后单独复验。
- 最终冻结 V4 的两项导航原生测试全部通过（显式 `--ignored`，2/2，214.60 秒包含各自冷 WASM 准备），日志 `git-navigation-native-v4.log`。覆盖目录优先完整文件顺序、前后按钮、折叠祖先展开与选中、严格 2 秒提示超时、跨文件内容头尾及列表端点不循环；双栏／三栏及补充的统一视图均检查真实工具栏导航与原生像素居中，源和派生文本／选择保持不变。统一视图只新增 `cfg(test)` 只读状态检查接缝，未改变已构建 Release 的产品代码。
- 相同 V4 的实际包相邻文件性能测试通过（显式 `--ignored`，1/1，76.64 秒含冷 WASM 准备），全部原有隔离、缓存、高亮与清理断言保留；日志 `git-navigation-adjacent-native-final.log`，具体点击数据见上述子进程日志。最终格式及差异检查通过。
- 最终 `cargo build -p editor-app --bin editor-app --release --jobs 1` 通过（9 分 30 秒），日志 `git-navigation-2026-10-28-release-build.log`。保留既有警告和 MSVC 链接器符号导入警告，未作为零警告构建报告。
- 以上未写绝对路径的日志位于 `C:/Users/Tmiracle/.codex/task-artifacts/`。功能交互与工作区最终检查已通过，实际用户配置仍待用户手动安装验证。

初轮宿主 `cargo test -p editor-app --no-run` 被既有 `svg_preview_render` 示例的旧组件路径／枚举匹配错误阻断；后续明确使用 `--bin editor-app`，未修改无关示例。工作区默认并行测试编译因 Windows `os error 1455`（页面文件不足）无法映射依赖元数据，随后触发 rustc ICE；核对工作区与 target 均为本任务后改用 `--jobs 1`，未清理或伪造源码接口。

最初原生探针仅凭进程归零和场景稳定结束等待，可能在异步原生比较尚未完成时断言失败；保留该轮失败日志。新版接缝额外观察公开运行状态中的待完成请求，并等待目标提交／文件真正显示。更严格的 V2／V3 探针仍复现文件树永久禁用，由此确认并修复上述产品缺陷，而非只延长测试等待。失败证据为 `git-startup-2026-10-28/ownership-native-v2-failure.log`、`git-navigation-native-v3.log`。

## 候选产物

Git ZIP：`C:/Projects/RustProjects/Editor/dist/git/git-0.1.48.zip`，SHA-256 `b51bc8a7397717a60a272b0a14d38ce646edf81e5e3a363f0dc5cf488a082e38`。本轮扩展公开能力，须配套本轮宿主，不能仅更新插件到旧宿主后验证。候选未自动安装，未发布远程版本。

配套 Release 宿主：`C:/Users/Tmiracle/.codex/worktrees/git-diff-scroll-fixes/Editor/target/release/editor-app.exe`，SHA-256 `b78b0eb380534017ffa2a0f433dea079a056f37f6dc6822c19dc998fee88404f`。本轮仅生成可执行文件，不生成宿主 ZIP。

就绪发布修复前的失败包单独留在 `C:/Users/Tmiracle/.codex/task-artifacts/git-0.1.48-before-ready-publication.zip`，SHA-256 `969a9919ebc4b94a0537c6ea32ce2ba6882c28a0aecde16ed8792dec2b792e80`，仅用于保留失败复现，不作为交付包。
