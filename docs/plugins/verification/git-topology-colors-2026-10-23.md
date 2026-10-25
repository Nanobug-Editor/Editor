# Git 拓扑图分支配色

日期：2026-10-23。状态：实现、插件测试、workspace 检查及最终同包原生复验完成；未安装到用户配置、提交或发布。

## 行为

- 拓扑线条和提交节点按轨道配色，提供八种深／浅主题颜色。第一父提交延续当前轨道颜色，其他合并父提交分配独立颜色；与已有轨道汇合时由较早建立的主线延续共同祖先颜色。Git 先遍历侧支时不会让侧支接管主线祖先颜色。
- 用户追加实心／空心节点区分，默认采用已说明的“普通提交实心、多父合并提交空心”。普通点半径 3px，合并圆环半径 4px、线宽 1.5px；连接线止于圆环，中心透明，选中行也不会出现错误的背景色或穿心连线。节点样式跟随实际父提交数量，不依赖显示标签。
- 颜色随未解析的父提交身份保存，不从横向列号计算；轨道压缩、分页和 SVG 分块边界不会导致既有轨道换色。相同有序历史重新生成时结果确定。颜色用于区分图中的轨道，不承诺一个分支名称在任意筛选历史中始终对应同一颜色；超过八种同时所需颜色时循环复用。
- 着色仅在新提交入图时分配。滚动继续使用有界预绘及缓存的 SVG 块；切换主题重绘配色，保留滚动 epoch、位置和提交选择。列间透明拖动边界及提交表单逻辑继续沿用 0.1.30。
- 修改位于独立插件 `C:/Projects/RustProjects/Git-for-Nanobug/src/graph.rs` 和 `src/history/drawing.rs`；增加相邻测试。宿主仅扩充原生验收夹具和断言，无生产接口、能力或权限变更。

## 验证

证据目录：`C:/Projects/RustProjects/Editor/target/git-topology-colors-2026-10-23/`。

- `color-tests.log`：5 项匹配 color 过滤词的测试通过，覆盖压缩、合并、跨块／分页、主题重绘和配色可见性。完整插件回归另含八轨道调色板复用及任意分页一致性。
- `guest-tests-nodes.log`：最终实现的 167 项插件测试全部通过，包含既有 32768 行历史与有界滚动开销测试，以及新增的“侧支先遍历”颜色回归、普通实心／多父合并空心和连接线不穿圆心检查。首次原生截图暴露祖先颜色被侧支接管的问题，修正后重新构建；早期 165／166 项和旧包的验收不替代最终节点版本结果。
- `package-nodes.log`：通过宿主公开打包入口生成 Git 0.1.31，继续要求 `ui.canvas >=1.3`，协议和权限声明不变。
- `check-nodes-results.json`：最终节点版本的格式检查、workspace 非 editor-app 测试及 workspace 编译检查全部通过；测试合计 519 passed、382 ignored。首次格式检查要求调整新增模块声明顺序，修正后检查通过。ignored 不计作通过，实际包原生测试显式另行执行。既有编译警告和 LNK4217 链接警告仍有输出，无编译错误。
- `native-nodes-recheck.log`、`native-nodes-recheck-results.json`：最终包在深色中文 100%／浅色英文 150% 两种组合完整通过原生复验。临时仓库构造真实分叉／合并提交，最终包经过公开管理器进入原生画布，线条／节点配色、空心合并圆环、列宽拖动与选择保持均通过；两组均正常退出。
- `native-nodes-recheck/<case>/evidence.json`：两组 `solid_normal_hollow_merge`、`matching_lines_and_nodes`、`scrollbar_width.hidden` 均为 `true`。各 32 次滚轮输入累计 160px，反向边界回到 25px，均复用两张已栅格化矢量图像。publication_ms 仅为诊断，不作屏幕帧率承诺。最终包深浅主题的 `history-colors.png` 已目视核对，空心圆心透明，主线在汇合后延续原色。
- 首次最终节点版运行 `native-nodes.log`：深色组合完整通过；浅色组合的 `history_colors`、`history_columns`、`history_scroll` 均通过，随后已有滚动条收窄断言失败，采样宽度一直为 6px。失败日志与 `native-nodes/light-en-150/` 证据完整保留。复验使用同一二进制和同一包摘要，没有放宽断言或改动生产代码；两组复验均通过，但该次不稳定失败原因未确定，不宣称已经修复滚动条缺陷。

```powershell
$nanobug = 'C:/Projects/RustProjects/Editor/target/git-panel-isolation-2026-10-22/Nanobug.exe'
& $nanobug --plugin-cargo C:/Projects/RustProjects/Git-for-Nanobug/Cargo.toml test --lib
& $nanobug --plugin-package C:/Projects/RustProjects/Git-for-Nanobug --output target/git-topology-colors-2026-10-23/packages
cargo test -p editor-app git_native_history_scroll_probe --no-run --jobs 2
cargo fmt --check
cargo test --workspace --exclude editor-app --jobs 2
cargo check --workspace --jobs 2
```

原生验收设置 `NANOBUG_GIT_CANDIDATE`、`NANOBUG_GIT_NATIVE_OUTPUT`、`NANOBUG_GIT_NATIVE_HISTORY_ONLY=1`、`NANOBUG_GIT_NATIVE_SCROLL_ONLY=1`、`NANOBUG_GIT_NATIVE_GRAPH_COLORS=1`，显式运行测试二进制中的 `extensions::worker::native_acceptance::git_native_history_scroll_probe --exact --ignored --nocapture --test-threads=1`。使用临时配置与仓库，不改变用户显示缩放或用户仓库。

交付包：`C:/Projects/RustProjects/Git-for-Nanobug/git-0.1.31.zip`，与证据目录 `packages-nodes/git-0.1.31.zip` 相同；SHA-256：`3337cb058be84c11c381b1c6220301fac2d642aed7f80d29cba12434852cb352`。仅需更新独立插件包，沿用 0.1.30 的配套宿主；未安装到用户配置、提交或发布。已有列宽任务和其他工作区改动不计入本次新增成果。
