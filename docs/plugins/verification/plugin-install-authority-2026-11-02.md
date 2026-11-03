# 插件安装授权与项目启用回归

日期：2026-11-02。状态：三项问题已复现并修复，相关回归、workspace 测试、编译及 Release 构建完成；全库格式检查的非本轮限制见下表。

后续交付：2026-11-03 已按新授权提交、合并并推送，使用 Git 0.1.55 重跑真实首次安装与更新回归；最终全库格式检查通过。安装器及发行状态见[统一发行验收](../../distribution/verification/release-all-2026-11-03.md)。下文的未提交状态和测试数量仅描述原专项结束时的证据。

本次按用户选择直接修改主工作区 `main`，不自动提交或推送。问题来自 Git 0.1.54 安装／更新时同时出现的安装授权撤回、`Plugin is disabled` 和面板可见性权限拒绝。

开始时主线为 `60412e95`。工作结束时另一个任务已将文件加载／高亮修复 `31429641` 合入 `main`，并有 Vue 测试改动继续写入；这些不属于本次成果。下表记录各命令实际运行时的结果，不将后续其他任务的变更计入已验证范围。

## 已复现的问题与修复

1. `Manager::event` 校验面板显隐时读取持久化的全局 `enabled`，遗漏当前工作区的 `project_enabled`。因此管理页和原生面板认为插件可用，运行时却返回 `PermissionDenied: Visibility requires an enabled declared surface and current ui.panels consent`。回归测试原先在 0.02 秒内失败；修复后，项目覆盖与全局默认共用相同的面板归属、权限、信任和能力检查。其他工作区、应用作用域及已撤销权限不借用项目覆盖。
2. 管理界面的启动、定时和管理页刷新共用 `refresh_online_market`；安装期间刷新会推进市场目录代次，使已经确认的安装收到 `Installation authority was withdrawn`。使用真实本机 HTTP 下载并在接收包体前暂停，调用原生管理界面的刷新入口，可以稳定得到同一错误。现在界面在安装控制对象释放后再刷新，不提前记下检查时间；运行时显式刷新仍会撤销旧授权。
3. 安装、更新和退休会保留 `instance_epochs` 中的计数，即使尚无存活实例。面板轮询及管理页通知路由把该计数当成存活证明，会向冷实例发送主题／焦点通知。回归测试原先在 0.18 秒内得到两次 `Plugin is disabled`。现在通过已有的 `live_instance_epoch` 查询实际存活拥有者；旧界面的回调仍保留原始代次，不能转借替换实例。

变更均位于通用宿主路径，不增加 Git 专属分支，不改变公开能力、包格式或权限要求。无需重打 Git 插件包。修复中的状态边界已同步写入源码注释。

## 回归入口

在编辑器主工作区执行，沿用自身的 `target/`：

```powershell
cargo test -p plugin-runtime --lib panel_visibility -- --nocapture
cargo test -p editor-app -j 1 cold_native_observations -- --test-threads=1
cargo test -p editor-app -j 1 marketplace -- --test-threads=1
cargo test -p editor-app -j 1 panel_visibility -- --test-threads=1
```

市场测试中的两个实际包测试显式标为 ignored，需准备独立构建的包再单独执行。本轮复用已存在的 Git 0.1.53／0.1.54 ZIP，没有重新打包或替换固定夹具：

```powershell
$env:NANOBUG_MARKET_INSTALL_CANDIDATE = 'C:/Projects/RustProjects/Editor/dist/git/git-0.1.54.zip'
$env:NANOBUG_MARKET_INSTALL_PREVIOUS = 'C:/Projects/RustProjects/Editor/dist/git/git-0.1.53.zip'
cargo test -p editor-app -j 1 marketplace_candidate_project_ -- --ignored --nocapture --test-threads=1
```

实际包测试通过本机 HTTP、完整 ZIP 摘要校验、生产安装 actor、公开 Manager、真实 WASM 按需启动、显隐事件和 FIFO 完成边界，覆盖仅项目启用的首次安装与存活旧版本更新。运行数据及 Git 仓库都在临时目录，不读取用户现有插件安装及私有数据，不改动全局 Git 设置或用户项目内容。

| 输入 | SHA-256 |
| --- | --- |
| Git 0.1.53 | `f9aca7fbb36d55796360dfebc07c3789fa306e799e642b8b99de0b320fbea538` |
| Git 0.1.54 | `90703755fd398e5f7a10017dbe248ca593e847b7c88277880a49638e681e609c` |

## 最终验证

| 检查 | 结果 |
| --- | --- |
| 运行时面板可见性 | 5 项通过 |
| 冷实例主题／焦点及替换拥有者 | 2 项通过（0.44 秒） |
| 市场安装、界面刷新与授权撤回 | 与面板交互合跑：16 项通过，8 项按需测试跳过（15.44 秒）；其中实际候选包的 2 项另行显式执行 |
| 实际 Git 0.1.54 首次安装及 0.1.53 更新 | 最终显式重跑 2 项通过（202.13 秒），均保持全局禁用、本项目启用且显隐通知无操作错误 |
| GPUI 面板显隐及双语安装交互 | 包含于上述 16 项；实际点击确认、更新操作及 dock／独立窗口显隐通过 |
| `cargo fmt --check`、`git diff --check` | 本轮 8 个 Rust 文件单独格式检查及 diff 检查通过；全库格式检查受同时出现的非本轮 `contributions/tests.rs`、`vue_standalone.rs` 格式差异影响，未改动这些文件 |
| `cargo test --workspace --exclude editor-app -j 1` | 879 项通过，447 项按需测试跳过，0 项失败；128 个测试／文档测试目标完成 |
| `cargo check --workspace -j 1` | 通过 |
| `cargo build -p editor-app --release -j 1` | 通过；已生成 `C:/Projects/RustProjects/Editor/target/release/editor-app.exe` |

默认并发的 workspace 测试曾因 Windows 页面文件不足（`os error 1455`）及 LLVM 内存分配失败中断；后续使用单并发，未更改系统页面文件、依赖版本或输出目录。最初的 UI 失败断言在持有发布锁时触发，导致退出清理二次 panic；测试现已在断言前释放锁并完成 actor 关闭，保留原始错误信号。

完整日志已保存至项目外的 `C:/Users/Tmiracle/.codex/task-artifacts/nanobug-git-install-20261102/`，统一使用 `nanobug-git-install-*.log` 前缀。最终交付为主工作区未提交的源码与上述 Release 宿主；未发布或打包安装器，未修改现有用户插件安装。按需测试的跳过项没有计为通过。

返回[插件系统目录](../README.md)。
