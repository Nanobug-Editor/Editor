# GPUI Kit 依赖迁移与退出修复

创建于 2026-10-26，更新于 2026-10-28。当前来源已切回包含退出修复的官方 main 固定提交；本轮验证与打包结果见下节。后续章节保留历史版本的实际结果。

## 2026-10-28：退出修复合并后切回官方 main

官方 [PR #3697](https://github.com/longbridge/gpui-kit/pull/3697) 已于 2026-10-28 09:44（北京时间）合并，合并提交为 `9d5cd7c41041c9bb500fc3e76f5a71677fcb75bc`。本轮通过 GitHub 元数据及 `git ls-remote` 核对，采用查询时官方 main 最新提交 [`ec89cfc36404667b7947aeb3d40b1c2859d729dd`](https://github.com/longbridge/gpui-kit/commit/ec89cfc36404667b7947aeb3d40b1c2859d729dd)。Kit、Base、Assets 的 Git URL 与完整 SHA 保持一致；继续固定提交以确保构建可复现。

三项包仍为 0.8.1，GPUI Pre 仍为 0.3.10，继续关闭 Kit 默认功能，仅选择 Assets。原任务 worktree 已回收，因此从当前 main `45a39a82d74a23cfdfaf70c873e91364150d3a6b` 重建 `codex/gpui-official-main-20261028`，位于 `C:/Users/Tmiracle/.codex/worktrees/gpui-official-main/Editor`。构建使用该工作区自身的 `target/`；日志与交付物集中在项目外 `C:/Users/Tmiracle/.codex/task-evidence/gpui-official-main-20261028/`。

独立来源审计确认修复提交位于该 main 快照的祖先中，`crates/base/src/text/inline.rs`、上游原生退出回归和三项包清单与此前修复 Fork 逐字节相同。后续 Base 提交涉及光标闪烁配置、可选文本绘制与 Markdown 引用图片；现有主题构造方式兼容，无需修改宿主 Rust 实现。

本轮验证使用最终锁文件；锁文件仅变更三个 Git 来源，其他包版本和依赖边保持原样。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过 |
| `cargo test --workspace --exclude editor-app --locked --offline -j8 --no-fail-fast` | 126 组，838 passed、0 failed、441 ignored |
| `cargo check --workspace --locked --offline -j8` | 通过，存在既有警告 |
| `cargo test -p editor-app hover --locked --offline -j8 -- --test-threads=1` | 16 passed、0 failed、2 ignored |
| `cargo test -p editor-app native_initialized_windows_exit_without_tls_failure --locked --offline -j8 -- --ignored --nocapture --test-threads=1` | 1 passed；Kit Root 与原生编辑器两个独立子进程正常退出，共约 1.14 秒 |
| 全目标、全部功能的 Cargo 依赖树 | 无 Component／宏库、GPUI Fast 或 Fork 来源 |
| 文档链接审计 | 82 个相对链接有效 |

原生退出回归使用 debug 测试程序和真实 Windows/D3D11；断言包含实际绘制、原生编辑输入、窗口销毁、`Application.run` 返回及子进程退出码 0。仅执行这一项 ignored 原生回归，其他跳过项不计为通过。未改变退出期限或任何测试断言。

日志为上述项目外目录的 `workspace-test-final.log`、`workspace-check-final.log`、`app-hover.log`、`native-exit.log` 和 `dependency-tree-final.log`。

### Release 便携包

`cargo build -p editor-app --release --locked --offline -j8` 通过，耗时约 3 分 54 秒。产物为真实 AMD64 PE，复制到项目外交付目录后使用新的独立 `--profile` 启动，已查看正常首页；通过标准 Alt+F4 关闭后，进程约 0.29 秒自然退出，退出码 0。记录为 `production-visible-start.json`、`production-visible-result.json` 和 `production-visible.*.log`。初次隐藏窗口启动未进入可见交互，已清理其自有进程，不计为通过；有效验收使用正常窗口和另一个新 profile。

直接使用 7-Zip 生成并执行完整性检查，未调用存档打包脚本。`delivery/Nanobug-Windows-x64.zip` 为 31,784,398 字节，包含 `editor/Nanobug.exe`、品牌 ICO、Apache 许可、`LICENSE-ALACRITTY` 与终端 `NOTICE` 五个文件；无插件包、源码或 PDB。ZIP 内程序的 SHA256 与本次 Release 编译产物完全一致。

| 产物 | SHA256 |
| --- | --- |
| `delivery/Nanobug-Windows-x64.zip` | `6d9d9a5bf0045ba5c82e7dc3f3616902a5d237010deee6833e78f1671322fd3b` |
| ZIP 内 `editor/Nanobug.exe` | `63f0e5e12ba3f152ebaae106de8196ddc4a6e467917477c353580315b7b6cc66` |

同目录 `.zip.sha256` 与上一级 `delivery-manifest.json` 保存校验记录。运行时使用独立 profile，没有替换用户正在运行的旧程序或修改其数据。本机没有 Inno Setup，本轮交付便携包，未生成安装向导、安装工具或发布远程版本。

## 2026-10-27：采用退出修复 Fork（历史）

用户明确要求“目前 gpui-kit 退出死锁的修复还未合并，切换使用我的 Fork 仓库”。本次统一锁定 Kit、Base、Assets 到 [`T-miracle/gpui-kit@1ffc0dd462c88b5dc15f61da453dc6800e862d8f`](https://github.com/T-miracle/gpui-kit/commit/1ffc0dd462c88b5dc15f61da453dc6800e862d8f)，远端分支为 `fix-text-cleanup-shutdown`，对应未合并的官方 [PR #3697](https://github.com/longbridge/gpui-kit/pull/3697)。已通过远端分支和 PR head 核对 SHA；没有使用 Fork 的旧 `main` 分支。

该提交的唯一父提交就是官方 `611b55e3`，因此包含已合并的换行优化 #3691。修复让 `RetainedLayoutCleanup` 保留前台执行器，直到通道和清理任务销毁之后再释放调度器，避免关闭通知同步析构接收器时重入事件锁。包版本仍为 Kit／Base／Assets 0.8.1、GPUI Pre 0.3.10，继续关闭 Kit 默认功能和 Component。

沿用原任务工作区和 `codex/gpui-kit-official` 分支。先保存本任务差异，再将任务分支 fast-forward 到最新 main `025e56f8d643160b2de43e1d93360ea683f2fd51`，重新应用接口迁移；保留 main 新增的 Canvas 行为和 profiler 功能，并适配其新增滚动调用。主目录当前干净，未把旧工作区版本覆盖到新 Canvas 实现。

本轮普通检查输出目录为 `target/fork-20261027/`，原生 Release 为 `target/fork-native-20261027/`，均与此前官方版本的输出隔离；构建前用 `cargo metadata --no-deps --format-version 1 --locked` 核对了实际根目录与输出路径。

### 本轮 Nanobug 验证

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --check`、`git diff --check HEAD` | 通过 |
| `cargo check --workspace --locked --offline -j 6` | 通过，保留已有警告 |
| `cargo test --workspace --exclude editor-app --locked --offline -j 6 --no-fail-fast` | 修复输入适配后重跑通过：126 组，831 passed、0 failed、441 ignored |
| `cargo test -p editor-ui local_source_find_uses_base_query_and_restores_source_focus --locked --offline -j 6 -- --test-threads=1` | 原断言不变，1 passed；首轮失败及两轮精确复现保留在日志中 |
| `cargo test -p editor-app shortcuts --locked --offline -j 6 -- --test-threads=1` | 最终源码重跑：24 passed、5 ignored |
| 同上过滤 `canvas` | 31 passed、7 ignored |
| 同上过滤 `code_sync` | 5 passed、14 ignored |
| 同上过滤 `hover` | 16 passed、2 ignored |
| 同上过滤 `scroll` | 30 passed、15 ignored |
| 全平台、全功能依赖树及源码审计 | 三项 Kit 统一 Fork／SHA，无 Component 或活动 Fast；68 个滚动事件均保留 Pointer 来源，无旧滚动接口 |

app 过滤集合可能重叠，不相加作为独立用例总数。未执行的 ignored 测试不计为通过。原始日志见 `target/fork-20261027/verification/`，包括 `workspace-test-final.log`、`workspace-check-final.log`、`app-final-*.log` 和 `audit-cargo-tree.txt`。

原生程序使用最终源码重新构建，随后在无编译负载时串行运行 `cargo test -p editor-ui --release native_window_drag_tracks_display_refresh --locked --offline -j 3 -- --ignored --nocapture --test-threads=1`。`NANOBUG_DRAG_SURFACE` 分别为 `main`／`split`，分隔条场景设置 `NANOBUG_DRAG_SOURCE_ROWS=1500`，浅色场景另设 `NANOBUG_DRAG_LIGHT=1`。

| 原生场景 | fps／显示刷新率 | p95／最大帧间隔 | 退出 |
| --- | --- | --- | --- |
| 普通主窗口，深色 | 143.95／144 Hz | 7.398／9.670 ms | 正常，父进程约 3.65 秒结束 |
| 1500 行分隔条，深色 | 133.89／144 Hz | 11.374／76.363 ms | 正常，父进程约 3.91 秒结束 |
| 1500 行分隔条，浅色 | 134.47／144 Hz | 11.306／78.556 ms | 正常，父进程约 3.91 秒结束 |

三场景均满足原有拖动断言，父子进程正常返回；没有放宽阈值、超时或跳过析构。已查看导出的深浅主题画面，中文／Emoji、行号和实时折行显示正常。日志与画面为 `target/fork-native-20261027/verification/{main-dark,split-1500-dark,split-1500-light}.{log,png}`；分隔条跨断点仍有表中最大间隔所示慢帧，不将旧 Fork 性能数字替代本轮实测。

### 独立生产退出复测

新建非 Git 临时包 `C:/Users/Tmiracle/AppData/Local/Temp/nanobug-gpui-fork-exit-repro-20261027`，使用独立 Cargo 输出目录和普通 `main`。唯一直接依赖为上述 Fork 的固定提交，关闭默认功能；其余传递依赖与旧官方复现包逐项一致，没有窗口、输入注入或测试支持。每轮外层期限仍为 8 秒。

| 条件 | 本轮实测 |
| --- | --- |
| Fork，跳过 Base 初始化 | 3/3 正常退出，0.416–0.478 秒 |
| Fork，正常 Base 初始化 | 3/3 正常退出，0.390–0.398 秒 |
| 官方 `611b55e3` 的既有复现二进制 | 退出请求后仍卡住，8 秒超时；已终止并回收本轮子进程 |

六轮 Fork 均退出码 0，并输出 `application returned normally`。构建使用 `cargo build --locked --offline -j 3`；格式检查通过，功能树没有启用 `test-support`、Component 或 Fast。原始源码、锁文件、构建日志和运行检查器保存在上述临时包；运行结果另复制到本任务 `target/fork-20261027/verification/standalone-exit-results.json` 与 `standalone-exit-runs.log`。此结果只确认退出修复，Nanobug 的原生交互以其独立验收为准。

以下为 2026-10-26 官方版本的历史调查，失败和旧版本数据不作为本次 Fork 的通过依据。

## 历史来源与范围（2026-10-26）

用户要求在自己的 PR 合并后，把 Nanobug 的个人 Fork 引用迁移至官方最新提交。[PR #3691](https://github.com/longbridge/gpui-kit/pull/3691) 已在 2026-10-26 合并；本次通过 GitHub PR 元数据和 `git ls-remote` 核对，官方 `main` 最新提交为 [`611b55e331170ca375214d580b6c5409db858f83`](https://github.com/longbridge/gpui-kit/commit/611b55e331170ca375214d580b6c5409db858f83)，包含实时换行缓存修复。

- 根 `[patch.crates-io]` 将 `gpui-base`、`gpui-kit`、`gpui-kit-assets` 统一锁到官方仓库该 SHA。
- 官方该提交的三项包版本为 0.8.1，依赖 GPUI Pre 0.3.10；同步更新三个 UI 消费 crate 的 Base 版本与直接启用测试支持的 GPUI/平台版本，避免混用快照类型。
- Kit 继续关闭默认功能，仅启用 Assets；本地控件继续使用 Base 与 GPUI 原语。未启用 Component、内建语言 grammar 或 GPUI Fast。
- 旧 Fork `4db6da3927224d2d856b5f3c21b9aa5103eac083` 的数据属于历史修复验收，不能当作本次官方版本的性能结果。

## GPUI 快照适配（本次继续保留）

0.3.10 的键盘与滚动接口发生变化，因此此次迁移包含必要的宿主调用与测试适配：

- 动态快捷键改用 `Keystroke::parse_with_os`；解析、捕获、序列保存、键帽展示与解析器匹配统一使用原有 `App` 的 `platform_info().os`。`KeyBinding::load` 传入原有 `App`，辅助修饰键判断使用 `secondary(cx)`；固定导航键仍用静态解析入口。
- 滚动类型、分发枚举与监听器改为 `ScrollEvent`、`PlatformInput::Scroll` 和 `on_scroll`。已有鼠标／触控板合成事件显式标为 `ScrollSource::Pointer`，保留原有位移、阶段和断言；不把它们变为触屏输入。
- 新增配置保存／重载及捕获回归，覆盖 Windows、macOS、Linux 的 `win-/cmd-/super-` 规范序列，以及 `secondary-` 的平台语义。
- 输入框可访问性投影改用 Base 公开的 `InputAccessibility`／`set_accessibility`。原适配器依赖渲染结果是 `Stateful<Div>`，而新版测试支持将其包装成 `Observed<_>`，使下转型早退并丢失角色和值元数据；首轮完整测试据此捕获源码查找框回归。此次移除具体类型依赖，由原 Base 节点提供实时值和受禁用／只读／掩码策略约束的动作，保留原编辑状态、焦点与单一 Tab 注册；原失败测试断言不变。

## 官方版本的工作区与验证（历史）

本任务从本地 main 的 `ffd9efbe5176a23d6caf4424c83903f6d409f64e` 建立 `codex/gpui-kit-official`；路径为 `C:/Users/Tmiracle/.codex/worktrees/gpui-kit-official/Editor`，使用该目录独立的 `target/`。主目录已有未提交改动未带入隔离工作区。

日志保存在本工作区 `target/gpui-kit-upstream-20261026/`。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --check`、`git diff --check` | 通过；临时诊断源码撤回后再次通过 |
| `cargo check --workspace --locked -j 6` | 通过，保留既有未使用项等警告 |
| `cargo test --workspace --exclude editor-app --locked -j 6 --no-fail-fast` | 112 组，760 passed、0 failed、407 ignored |
| `cargo test -p editor-ui source_resize_preserves_composition_and_reflows_immediately --locked -j 8 -- --test-threads=1` | 1 passed；含深浅主题、中文／Emoji IME、即时换行与 Undo |
| `cargo test -p editor-app shortcuts --locked -j 6 -- --test-threads=1` | 24 passed、5 ignored；含新增跨平台配置／捕获回归 |
| 同上过滤 `code_sync` | 4 passed、14 ignored |
| 同上过滤 `hover` | 16 passed、2 ignored |
| 同上过滤 `scroll` | 24 passed、14 ignored；不同过滤集合可能重叠，不相加作为独立测试数量 |
| `cargo tree --workspace --target all --edges normal,build,dev --all-features --locked` 与全部源码检索 | 无 `gpui-component`／宏库、无 Fork 来源、无旧滚动接口；活动依赖树未启用 GPUI Fast |
| Release 原生拖动 | 1500 行 split 在 25 秒退出时限内未结束；最小 1 行 split 与纯主窗口对照同样失败，不能计为性能通过 |

首次离线依赖树审计缺少跨平台包缓存，允许联网下载后重跑通过；未修改依赖范围来绕过它。隔离基线的 `source_resize_frame_cost` 使用模拟字体，本次没有用它报告真实字体性能。其他未执行的 ignored 用例不计为通过。

## 官方版本的原生退出阻碍（历史）

Release 程序使用独立输出目录 `target/native-release/`，构建前通过 `cargo metadata` 核对根目录与输出路径。实际命令为：

```powershell
$env:CARGO_TARGET_DIR = 'C:/Users/Tmiracle/.codex/worktrees/gpui-kit-official/Editor/target/native-release'
$env:NANOBUG_DRAG_SURFACE = 'split'
$env:NANOBUG_DRAG_SOURCE_ROWS = '1500'
cargo test -p editor-ui --release native_window_drag_tracks_display_refresh --locked --offline -j 6 -- --ignored --nocapture --test-threads=1
```

最小化为 1 行、再改为不带源编辑器的 `main` 场景仍超时。阶段探针确认创建窗口、首帧、定时器、实际鼠标拖动、窗口移除和退出观察器均已完成；使用公开 `run_embedded` 保留强引用后，消息循环能够返回，但正常释放该强引用仍卡住。所有阶段探针和此临时入口改动均已撤回。

只对本任务自己启动的测试子进程使用系统 DbgHelp 与本次 PDB 读取线程栈，随后清理进程。关键栈保存在 `native-stack-output.log`：

```text
AppCell::drop_slow
→ globals map drop
→ gpui_base::text::inline::RetainedLayoutCleanup::drop
→ Event::notify
→ receiver future drop
→ event_listener::InnerListener::drop
→ Mutex::lock_contended / WaitOnAddress
```

官方固定提交的 [Base 清理实现](https://github.com/longbridge/gpui-kit/blob/611b55e331170ca375214d580b6c5409db858f83/crates/base/src/text/inline.rs#L407) 在析构中先调用 `sender.close()`，而它拥有的前台任务正在 `receiver.recv().await`。线程栈与代码共同将阻碍定位到此处：执行器退出后的关闭通知同步销毁监听器，重入等待事件锁。当前没有暴露该私有全局的正常生产清理接口；`App::clear_globals` 仅在测试支持下可用，不能据此绕过生产退出问题。没有采用泄漏对象、跳过析构、延长超时或修改依赖缓存源码的办法。

对照运行主目录中 2026-10-25 已存在的旧 Release 测试二进制，同一最小 split 场景约 3.8 秒正常结束（143.41 fps，144 Hz）。这只用于排除桌面环境不可用，不代表当前官方依赖通过原生验收。

### 完全独立于 Nanobug 的复现

后续建立 `C:/Users/Tmiracle/AppData/Local/Temp/nanobug-gpui-exit-repro`，独立 Cargo 工作区和输出目录，不引用 Nanobug 的任何 crate。唯一直接依赖为官方固定提交的 `gpui-kit`，关闭默认功能，不启用 Assets、Component 或任何 `test-support`。使用普通 `main` 与 dev profile，完全没有窗口、鼠标输入或测试框架。根目录／输出目录通过 `cargo metadata` 核对，锁文件确认 Base／Kit 0.8.1、GPUI Pre 0.3.10。

同一二进制保留相同原生平台和 150ms 后的正常 `cx.quit()`，只切换是否调用 `gpui_kit::init(cx)`：

| 条件 | 实测 |
| --- | --- |
| `official-kit-exit-repro.exe --skip-base-init` | 0.422 秒正常退出，exit 0，输出 `application returned normally` |
| `official-kit-exit-repro.exe` | 输出退出请求后卡住；外层 8 秒超时终止，未输出正常返回 |

源程序、独立锁文件、feature 树和结果保存在该临时目录；结果副本为本工作区 `target/gpui-kit-upstream-20261026/standalone-results.json`。外层使用 `subprocess.run(..., timeout=8)`，超时子进程已回收。

该控制实验确认故障无需 Nanobug 代码或测试框架即可出现，问题归属官方 Base 初始化后的退出生命周期。具体清理任务仍由前述原生线程栈定位；“哪条历史提交首次使退出失败”还需要版本二分，不能仅凭问题代码的引入提交直接认定。用户的 #3691 修改的是四个换行文件，没有改动此清理模块。

## 官方迁移阶段的交付边界（历史）

当时原生退出门禁未通过，用户选择的“通过检查后普通提交并自动本地合并”条件未满足，因此保留任务分支与所有适配结果，没有提交、推送或修改上游仓库。

当时主目录的未提交内容与本任务 14 个路径重叠。2026-10-27 继续任务时已确认这些改动由原任务合入 main，主目录干净；本任务已同步最新 main，后续仍按原授权在检查通过后普通提交并自动本地合并。
