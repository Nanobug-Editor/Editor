# Git 分支树文件夹与分支图标

日期：2026-10-19。范围：用户要求分支栏目录显示资源管理器文件夹图标，分支显示分支图标。独立候选 0.1.13 与配套宿主完成下述验证及身份核对，未自动安装或发布。

## 实现

- 独立源码 `Git-for-Nanobug/src/trees.rs` 为 branches 树的全部目录（本地、远程、分支前缀、远程名）发布 `icons/folder.svg`／`icons/folder-dark.svg`；本地和远程引用叶节点均发布现有 `icons/git.svg`。不改节点身份、排序、描述、分支筛选或菜单动作。
- 两枚文件夹 SVG 与宿主资源管理器默认资源逐字节一致。浅色 SHA-256 `82456d9de714442295c25c150490b9af9796102ab8ab8c3a4d74d7351511b92b`，深色 `275b833d6e3695560b66ae04b6e9232d3cef2a36c550ab7ccce2f162ad72e045`。分支 SVG 继续使用用户提供的资源，未复制上游组件或项目。
- 复用既有公开 `ui.icons`／`ToolIcon` 契约和 `assets.read` 权限；更新时由插件管理器确认新增资源读取权限。资源只能来自本插件包，仍经现有 SVG、路径、归属、大小和页面预算检查。
- 本地 `ui/controls/tree/icon.rs` 为通用树行增加彩色绘制：GPUI 的 svg 控件是 alpha mask，不能保留文件夹的填充／描边配色；小尺寸字面色 SVG 改走 GPUI 的内容寻址图片解码，currentColor 分支图标仍继承行前景色。固定 16px 图标槽保留几何及交互，未新增 Git 专属宿主逻辑。
- 彩色解码前禁止外部图片解析，检查 SVG 固有尺寸不超过 64×64；更大图标保留原有有界 mask 绘制，不让树行触发无界图片分配。未变化的图标按内容复用 GPUI 图片缓存。

## 已执行验证

- 公开宿主 `--plugin-cargo`：101 passed、0 failed、0 ignored；本地／远程多级目录和引用类型对应图标的回归通过。日志 `Git-for-Nanobug/target/tree-icons-unit.log`。
- 最终配套宿主执行相同公开入口，101 项再次通过（5.92 秒，`Git-for-Nanobug/target/tree-icons-unit-new-host.log`）。独立 58 文件源码归档逐项与当前源码校验一致。
- 本地组件两项通过：真实资源管理器深浅文件夹图标解码后保留多个原始 RGB 值、相同内容保持相同图片 ID；百万像素固有尺寸保留安全 mask 路径。日志 `target/git-tree-icons-colors.log`。
- 实际独立 ZIP → 公开 Manager → 真实 Git 懒加载树：4 个目录、3 个本地／远程引用均有正确图标，两套文件夹与 Explorer 资源完全相同，重复节点共享三项已准入 artwork，退役资源归零。通过，58.67 秒；`target/git-tree-icons-manager.log`。
- 同包原生 GPUI 两项通过，118.07 秒：深浅主题中右键菜单禁用项不派发，审查确认后实际切换分支；完整 history 在两种主题、三种宽度下保持原有布局。`target/git-tree-icons-native.log`。颜色由实际 SVG 解码像素回归检查，未将布局边界检查冒充截图像素验收。
- 最终 `cargo fmt --check`、`git diff --check`、`cargo check --workspace --jobs 1` 均通过；`cargo test --workspace --exclude editor-app --jobs 2`：78 组、292 passed、309 ignored、0 failed。ignored 未计作通过。日志 `target/git-tree-icons-format.log`、`target/git-tree-icons-diff-check.log`、`target/git-tree-icons-workspace-check.log`、`target/git-tree-icons-nonui.log`。

## 候选产物

- `target/git-tree-icons-2026-10-19/git-0.1.13.zip`：810762 bytes，SHA-256 `1170bdf6cd7c98e512c4846fe4e7309f2aa63dd563658f307b747d654a5ce2ee`。
- 独立源码 `C:/Projects/RustProjects/Git-for-Nanobug/dist/0.1.13/git-for-Nanobug-0.1.13-source.zip`：58 文件、167562 bytes，SHA-256 `e27086f7dc5230fe63a37815fd60ddee85c0b3a64ea9cd9e55736d06e2ea1023`。旁附文件清单、源码摘要及同一候选包；不包含 SDK、上游源码副本、构建缓存或原生二进制。
- 配套宿主 `target/git-tree-icons-2026-10-19/Nanobug.exe`：118460928 bytes，SHA-256 `af70fca2f6e7667e424d3667cf1218807cabb88e6af7c130be4cd948fd9ccaad`。直接 `cargo rustc -p editor-app --release --bin editor-app --jobs 1 -- -o C:/Projects/RustProjects/Editor/target/git-tree-icons-2026-10-19/Nanobug.exe` 构建成功，保留既有编译和链接警告；不覆盖正在运行的默认 release 程序。使用新的通用树行彩色绘制；旧宿主无法保留包内文件夹原始配色。
- 包内 `git.wasm`：2489490 bytes，SHA-256 `82a9279baeb5500465269535f1464a3534090f239b4148e20177794a4ff2664c`；`manifest.json`：4166 bytes，SHA-256 `c5f8ca9bfdf7302f0f51f0611d9c30388861e63a7e6516c9fed293228605fb18`。四枚图标和原生辅助程序合计七项包内容，原分支／暂存图标及原生程序摘要保持不变。

本轮不自动安装、提交、推送或发布。宿主保留已有工作区改动；本记录只对本次图标及相关回归作结论。未重跑完整中文 IME、窗口、DPI 与长时定时器矩阵；图标没有新增定时器或更改后台刷新策略。
