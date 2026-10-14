# GitHub README 原生呈现

日期：2026-10-14。本记录保存提交前的本地制品与验收；远程交付候选的隔离复验见文末。

用户要求 README 图片按 width/height 显示、遵循 GitHub 的居中和 alt 语义，并对齐样式。依据为 [Primer 正文](https://github.com/primer/css/blob/main/src/markdown/markdown-body.scss)、[标题](https://github.com/primer/css/blob/main/src/markdown/headings.scss)、[代码](https://github.com/primer/css/blob/main/src/markdown/code.scss)及[表格](https://github.com/primer/css/blob/main/src/markdown/tables.scss)。默认正文 16 px / 1.5 行高；六级标题采用 2 / 1.5 / 1.25 / 1 / 0.875 / 0.85 比例，H1/H2 有底部分隔线。段落、引用、列表、代码块和表格使用对应间距、边框与表面。

插件业务在 `C:/Projects/RustProjects/Markdown-for-Nanobug`。HTML 的 p/div/标题 align 投影为通用原生对齐，同段多图保留唯一身份和各自链接；成功图片的 alt 是无障碍名称，不生成说明。失败替代文字、尺寸、窄栏比例、受控资源、显式导航及 source/revision 校验保留。

宿主新增公开 `ui.styles 1.0`：Document.content_styles 提供有界 role 字体、行高、标题/代码/表格及边框参数；Layout.align/margin 提供几何。用户 role 字体与配色优先，省略样式保持宿主原行为。数值、角色键及六级标题数组在发布前验证，未协商的文档返回 CapabilityUnavailable。SDK 导出包含 styles.rs。宿主没有 Markdown ID、语言或 GitHub role 的专属分支。

## 红色反馈与修正

- `editor_app-markdown-github-red.exe explicit_image_dimensions_size_pixels_without_an_alt_caption --nocapture` 连续失败，128 px 图片块实际为 147.5 px，0.04–0.05 秒。宿主原来追加可见 alt。修正后实际块与像素高度相等，alt 进入无障碍名称；高度单独声明时额外限制整个图片元素宽度，避免 flex 容器留下多余高度。
- 旧 0.18.1 Package 的真实居中回归失败：图片中心偏左 146.75 px。属性在访客过滤器中丢失，现投影为公开对齐和包内布局。
- 初次新包发布被现有 64 项 RGB 配额拒绝。移除未使用的 rich_text/github-rule 颜色声明后为 63 项，不扩大配额；回归验证完整 file surface，而非尚未添加配色的中间树。

## 制品

- Markdown 0.18.2，protocol 7；新增 ui.styles 能力要求，权限集合不变。
- `C:/Projects/RustProjects/Markdown-for-Nanobug/dist/markdown-0.18.2.zip` SHA-256：`61a648070f6d659ffbadd9cf8a7f8da04110ba47d007a55f6ddcd28c93a31ca2`。
- 匹配 Debug 宿主 `target/debug/editor-app.exe` SHA-256：`cca18a10ac4aa8d65d0eea0495bc7ec195c1df9c0445628bf71da5902f8d41e6`；含其他任务已有改动，不计为本任务成果。
- 实际 SDK 摘要：`2180e97920c48bcaca5c98805573b50c7018a46c3919038bc8a60d572f24eff8`。本地源码与宿主未提交，账本使用 null revision，不冒充 host-sdk.json 的历史已发布来源。
- 独立最小 SDK 输入为 sdk-styles-fixture-1.0.0.zip，SHA-256 `91f41f5c959071208e2a985d2cae5ae50fef50f8b663a681e71fe5d7484e2552`。通过公开 --plugin-package 构建，只发布有样式的通用原生文本，不使用 assets、服务或文档权限；源码/包摘要进入固定账本。Markdown 0.17.2 市场输入及 0.18.0/0.18.1 候选字节均保留。

## 验证与边界

插件 85 项单元通过；两项快速通用 GPUI 回归通过，实际正文、H1、代码块和用户覆盖的高度分别为 24 / 50.6 / 51.72 / 36 px（容差 1 px），深浅主题均绘制；图片覆盖双边、单边、放大、原尺寸及零高。

七项真实 0.18.2 Package → Manager → 原生场景显式运行 --ignored，全部通过、0 ignored：HTML 图片精确 128×128、中心偏差小于 1 px、无 alt 说明与真实图片链接；HTML 深浅主题和未保存输入；独立表格单元格；实际图片和逐项失败；宽图随栏缩放；1/2/5 px 表格滚动不回弹；混合 Markdown、任务框、Undo/Redo 及关闭后的回收。各独立命令日志为 target/markdown-github-delivered_markdown_*.log，未重跑整个 Markdown ignored 组。

公开样式协商的实际 WASM 用例通过（1 passed、0 ignored）：删除 ui.styles 声明时明确拒绝；保留声明后，通过现行公开 begin_demand(View) / poll 激活通用文本视图，确认样式保留。早期沿历史示例在 install 后直接读取 live 的测试失败；现行按需激活不会在首次安装时启动业务，最终测试遵循该生命周期。失败尝试保留为诊断记录，不计通过。

cargo fmt --check、cargo test --workspace --exclude editor-app -j 1 与 cargo check --workspace 通过；非 UI ignored 项不计通过。为避免共享 target 中旧依赖产物复用，测试命令另传 --config profile.dev.package.plugin-protocol.debug=1；编辑器真实用例从 cargo rustc --tests 的 compiler-artifact 路径运行复制的测试程序，确保新增用例存在。相关 cargo test -p editor-app content_styles_control_native -- --nocapture 也实际执行为 1 passed。独立插件格式及两仓库 git diff --check 通过。

渲染使用 GPUI/Base 的公开行为接缝。浏览器 CSS 引擎、脚本、嵌入页面及 HTML id 锚点不在本次范围；Base 行内代码比例为 0.875，与 Primer 的 0.85 有细微差异，字体塑形及原生表格列宽也不保证逐像素相同。上述初次验收阶段未安装到用户 profile、提交、推送或发布，未执行人工 OS IME 或跨平台验收。

## 远程交付候选的隔离复验

用户随后授权普通提交、推送、创建 PR 并自动通过。宿主候选从 main 的 1379f58df76622bdf4d6a7c4167469f124d17a0b 创建隔离工作树，只包含本次公开样式、图片几何、测试和文档；原工作区其他任务的改动及暂存状态保留。插件候选从独立仓库 main 的 d0e58e8b0d0d4f546754c1a12fc81531daa6e6b7 创建。

隔离宿主的 Debug 程序 SHA-256 为 c10d290110f191951f3af7424120b2f7643833c00a9da24a8ccd41d0de402daa，实际 SDK 摘要为 997d337a5c4e07aca0094244bdfc0e426e9d6a3f061adc932875074f11057a8a。通过该宿主公开 --plugin-cargo 运行插件测试：85 passed、0 failed、0 ignored；公开 --plugin-package 生成新候选，完整 ZIP SHA-256 为 f3b03114865812b943af91fa3858c5dc4c79462efcfe3e8916dd5cf143bfa769。67 项包内容不含源码、SDK、Cargo 清单/锁文件或 target。

新候选单独冻结为 markdown-0.18.2-readme-pr.zip；原 0.18.2 本地制品及已发布输入均保留。最终原生测试显式选择新候选，8 项全部通过：HTML 图片尺寸、居中及图片链接；HTML 深浅主题与未保存编辑；HTML 文本链接；独立表格；逐图错误；宽图缩放；细微滚动；混合编辑、任务框、Undo/Redo 与关闭回收。两项通用 GPUI 几何回归及一项 SDK 样式协商真实 WASM 回归另行通过。

隔离目录中 cargo fmt --check、cargo check --locked --workspace -j 2、cargo test --locked --config 'profile.dev.package.plugin-protocol.debug=1' --workspace --exclude editor-app -j 2 均通过；非 UI 统计为 268 passed、256 ignored，忽略项不计通过。相关 cargo test -p editor-app 定向用例及两仓库 git diff --check 通过。本机日志为临时目录中的 nanobug-readme-pr-*.log，构建目录为 target/markdown-readme-pr-validation；不借用原工作区含其他任务的二进制。提交后插件 host-sdk.json 固定到已验证宿主候选的普通提交，供 PR CI 独立重建。此记录是源码 PR 候选验收，不变更 Release 或市场审核状态。
