# Vue2 / JavaScript 独立导航修复

日期：2026-11-02。状态：实现、原生交互验收和最终集成检查均通过，交付提交与包见下文。

## 需求与授权

用户要求 JavaScript 插件管理 JS 的 import 解析、定义跳转和悬浮，Vue 插件管理 Vue 文件；没有启用 TypeScript 插件时仍可使用 Vue2 + JavaScript。另修复跳转失败导致整份 Vue 文件失去高亮，以及缺少 Vue 图标。使用各仓库当前目录/分支，保留已有改动；检查通过后自动提交并本地合并 main，不推送、不发布。

## 根因与变更

- JavaScript 0.3.2 只有识别、高亮与格式化，实际 `textDocument/definition` 返回未处理方法。0.4.0 加入自己拥有的主分析进程，支持相对 import、jsconfig 路径别名、关联文件未保存快照。
- Vue 0.1.13 将 `typescript.adapter` 等服务声明为必须，单独安装会在按需激活时复现截图中的 0 compatible providers。0.2.0 的原生 Vue 角色通过公开文档/control 能力使用自有进程；旧服务契约改为可选，显式 peer 分析调用仍保留原协议。
- Vue2 适配器只在声明 props 时补齐 JS 构造器推断，普通 data-only 组件的模板字段退化为 any。legacy 0.1.12 在映射的虚拟代码中处理 Vue.extend、直接对象与有界静态 const 对象，不改用户源码；modern 0.1.11 保持原字节。
- 宿主原来在运行时错误后同时撤下不可变语言资源。现在保留已验证的识别、grammar、图标；禁用、移除、不兼容仍撤下资源，失败的可执行角色仍不会发布。没有新增语言或插件 ID 专属分支。
- Vue 声明深浅主题文件图标。TypeScript 0.1.19 主角色只管理 TS/TSX；内部服务保留关联 JS 输入和兼容契约。

两个新所有者使用相同固定 Node 24.19.0 / TypeScript 5.9.3 工具代码，运行时版本 0.1.17、SHA-256 `5532b77fcb6fe31612525caa140a1e499a0d10efa67326cd4ce3201fde0faf07`。这是工具依赖，不是启用或调用 TypeScript 插件；每个所有者有独立进程与清理路径。源版本与许可证信息随仓库保存。JS/Vue 更新新增权限按正常更新流程确认。

## 验证

- 原始 JavaScript 服务：定义请求失败；新增独立 LSP 回归通过相对导入、路径别名和未保存内容悬浮。
- 原始 Vue 包：实际 Package/Manager 按需激活失败，错误与截图一致。
- 高亮回归：先失败于 Vue 识别变成 None；通用宿主修复后通过，同一失败包仍显示 Vue 语言与 grammar。
- Vue2 JS data 回归：6 项初始 5 失败；修复后 6/6 通过，覆盖 Vue 2.6.14/2.7.16 和三种常见导出写法。
- Vue 源工具矩阵及最终封印模块各 108/108 通过，均为 0 跳过。legacy 0.1.12 修复已进入实际打包闭包，modern 模块未改变。
- 实际 JS/Vue 双包、没有安装 TypeScript 的宿主测试通过：每个文件的服务所有者正确，定义与悬浮返回原源码，F12 确实执行宿主跳转且保留高亮。
- Vue 2.6/2.7/3 混合工程、未保存源码同步与原源码导航测试通过，三个上下文均由 Vue 拥有；首次调用的夹具根目录参数错误，改为 Vue 仓库根目录后通过。
- TypeScript 配置工程/推断工程在 Vue 禁用后仍可分析 TS；新的 Vue 包单独进入 Manager 按需激活也通过。后者只证明 admission，完整语言服务就绪由宿主测试证明。
- 资源发布的 6 项普通测试通过；旧 Vue 0.1.13 的真实缺依赖回归另显式运行并通过。原有不兼容资源测试改为自行创建 Providers 与 CatalogView，避免依赖其他测试的全局状态。
- 主工作区 `cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --exclude editor-app` 通过；非 app 测试为 879 passed、0 failed、447 ignored。ignored 不计为通过，相关真实包测试按其要求单独运行。
- 所有插件格式检查、真实公开打包均通过。Vue 合并候选另在独立 checkout 通过 `--plugin-cargo Cargo.toml check --locked --offline --target wasm32-wasip2 --release --target-dir target -j 1`，固定运行时与协作 helper 摘要与已验收源码一致。

宿主任务改动另放入项目外 `host-validation` checkout，与主项目各用自己的 `target/`。集成到最新 main `0a41d271` 后，资源测试为 6 passed；第一次全量编译出现 `E0463` / `only metadata stub found`。相同工具链下限定单任务并发重新检查，`cargo test --workspace --exclude editor-app -j 1` 为 877 passed、0 failed、447 ignored，`cargo fmt --check` 与 `cargo check --workspace -j 1` 均通过；未修改产品代码或清理主项目缓存。主工作区与隔离候选的测试总数不同，是因为后者不包含其他任务的未提交测试。

### Windows 可见交互

使用真实公开 `--plugin-dev` 入口和只安装 JS/Vue 的隔离配置，Vue 版本为 2.6.14：

- Vue 模板 `message` 的 Ctrl+I 弹窗显示 `(property) message: string` 与 `App.vue:4`；F12 选中 data 中的原始声明，高亮保留。
- JS `@/model` 导入引用的 Ctrl+I 弹窗显示别名与 `model.js:1`；F12 实际打开 model.js 并选中声明。
- 文件树和标签页均显示 Vue 图标，浅色/深色主题都可见。
- 对应证据为 `vue-hover-light.png`、`vue-f12-light.png`、`javascript-hover-light.png`、`javascript-f12-light.png`、`vue-icons-dark.png`。

首次使用访问性树自动化、关闭设置弹窗后出现一次进程中止，未将此退出路径当作本次已修复。重新启动后完成深色取证，并通过开发控制器正常停止；`desktop-dark-report.json` 的启动、激活、清理均 passed，termination 为 normal。该报告的 business 字段是 not_exercised，业务交互依据上述人工可见截图与独立宿主测试，不由生命周期报告冒充。

桌面使用的宿主可执行文件 SHA-256 为 `f46fbe3d88c1fc7b96b4ee64a6356ca8834f2aa738af0bae7238d3132ad8195a`，包含本次高亮修复；其构建早于随后 main 合入的插件消息路由任务，后者由隔离候选检查覆盖。

真实包由公开 `--plugin-package` 入口构建，输出到主项目 `dist/javascript/`、`dist/vue/`、`dist/typescript/`。测试与 UI 使用独立临时项目/配置，不改用户安装。日志和截图位于项目外 `C:/Users/Tmiracle/.codex/task-artifacts/vue-js-navigation-20261102/`。

### 本地交付

| 仓库 | 提交 | 本地 main |
| --- | --- | --- |
| JavaScript | `430a060` | 当前 main 普通提交 |
| Vue | `0061aeb` | 验证独立合并候选后 fast-forward；原任务分支保留 |
| TypeScript | `a32df80` | 当前 main 普通提交 |
| Editor | 本记录所在的 `fix(language)` 提交 | 当前 main，只包含本任务文件与文档索引条目 |

| 包 | SHA-256 |
| --- | --- |
| `dist/javascript/javascript-0.4.0.zip` | `1aaaac101b326ff4d77e77bb7e5a817d6ca018c6a364116a54ec006167b44d46` |
| `dist/vue/vue-0.2.0.zip` | `6f6a59b0e7b1d8ca2ab647b4e8bf32ef3e0c3b48415384c8bebdbc8c21c53b47` |
| `dist/typescript/typescript-0.1.19.zip` | `9ea80e0850137b6ac0c7287ec47917b69da6dfb5d8f38a4e3eabca3994f0892d` |

未推送、创建 Release 或替换用户现有安装。使用本轮包时需要包含高亮保护修复的宿主；Vue/JS 自身分析不再要求安装或启用 TypeScript 插件。

## 边界

固定框架/平台支持范围沿用原矩阵，不把历史桌面记录算作新包验收。工作区存在其他任务的未提交改动，提交仅包含本次范围；不替其他任务提交、回滚或清理。
