# 插件管理更新入口与详情呈现验收

日期：2026-10-14。依据：用户追加的四项要求及[共用布局规格补充](../specs/plugin-manager-shared-layout.md#2026-10-14-管理呈现补充)。这次只调整宿主管理窗口的呈现和已有操作入口，没有新建工单、修改公开 SDK、升级依赖或发布插件包。

源码、最后回归、规定检查与两轴独立复审均已完成，无遗留问题。未提交或推送，没有替换用户正在运行的程序。

## 实际行为

| 用户要求 | 本次行为 |
| --- | --- |
| 已安装也显示更新 | 列表显示可更新版本，详情使用审核目录的兼容新版本直接更新；全局标记、列表和动作共用升级选择。概览与权限资料仍来自实际安装版本 |
| 隐藏不可执行按钮 | 同版、旧版、不兼容、撤回、未获得新目录授权及无实际实例的操作不渲染点击/焦点目标。忙碌时隐藏其他写操作，当前操作保留加载反馈；全局启停、本地包检查和空选择通过固定进度区反馈 |
| 优化其他信息 | 基本资料、兼容性与权限、目录与统计分组；字段名称对齐，长内容换行，来源/反馈入口保留；未知统计明确显示不可用，不使用假零值 |
| 其他版本使用列表 | 版本降序排列，整行可选择，显示发布时间、下载统计、可用性和已安装/浏览标记。已安装页标记本地实际版本，历史选择仍只改变浏览对象，不能降级或切换实例 |

可执行操作与加载反馈复用本地 Base Button、SegmentedTabs 和 Checkbox。按实际锁定的 `gpui-base 0.7.0` 核对 Button 的禁用、选中及输入接口；样式使用本地主题。新可见文本均提供中英文资源，没有复制上游成品界面。

## 比较点与保留范围

复审前重新读回的最新 `origin/main` 和 HEAD 均为 `1379f58df76622bdf4d6a7c4167469f124d17a0b`。本轮未提交，因此两轴沿该固定点审查限定工作区差异及新增测试文件，提交列表为空。

源码范围为 `extensions/management.rs`、`management/actions.rs`、`management/details.rs`、`marketplace.rs`、`marketplace/details.rs`；测试扩展既有 `management_tests.rs`、`worker/marketplace/tests.rs`、`tests/layout.rs`、`tests/delivery.rs`，跨功能辅助与进度用例位于 `crates/editor-app/src/tests/plugin_manager_presentation.rs`、`plugin_manager_progress.rs`；文案位于 `crates/editor-app/locales/app.yml`。

主工作区已有冷预览修复、菜单、运行工具栏、Markdown HTML 和图片等其他任务改动。验证复用 `C:/Users/Tmiracle/.codex/tmp/preview-error-20261014-1` 的候选：固定点源码、上一轮已经验证的四个冷预览文件、本轮十二个源码/文案文件；其他任务改动不纳入。每次同步使用绝对来源路径，刷新候选文件的修改时间，防止共享构建缓存误用其他导出目录的较新产物。

本轮十二文件与工作区逐一核对 SHA-256。`Cargo.lock` 保留本轮开始时的原始字节状态，SHA-256 为 `46f6fb29216824eac10793a136bd74ab8e089cba2fcba93554fe0148a8eacb11`。没有改动冻结 ZIP 或其账本；其他任务在主工作区添加的包和账本变化不计为本次验证输入。

## 真实入口与集中验证

沿用已确认的真实 ZIP → 公开 Package/Manager/Worker → 生产 GPUI 管理窗口接缝。HTTP 只在传输端使用本地受控服务器；下载摘要、权限确认、实际升级和私有数据保留走原有逻辑。扩展两组既有原生场景，统一验证新版提示、历史、资料、版本目标及生命周期，避免为每个字段或按钮重复安装夹具。

`marketplace_browsing_version_preserves_local_lifecycle_and_fixed_layout` 在两语言/主题下检查可更新 1.1.0、本地仍为 1.0.0，长维护者字段实际换行、标签列对齐、14 行历史滚到最后一行、850×600 窗口和 1.5 缩放，以及正文滚动时固定头不变；随后验证本地启停、重启、卸载和真实失败可见性。

`unbundled_official_market_install_and_existing_user_upgrade` 验证首次安装与从已安装页直接升级 TOML 0.2.2 → 0.2.3；安装成功后更新入口与标记消失，私有数据保留，撤回版本仍可浏览但没有安装按钮。既有默认市场组复用权限增量、同版/旧版、离线、撤回、摘要失败和受限工作区验证。

`global_toggle_keeps_progress_and_hides_other_write_actions` 复用真实声明式 ZIP/公开 Manager 初始化与生产管理控件，通过实际点击产生生命周期 receipt；既有记录传输只保留这份真实队列回执，测试未设置 progress 或伪造完成。两语言/主题下验证当前进度、其他写入口隐藏，以及搜索无匹配后进度继续显示；每个场景关闭自己的测试窗口，避免旧窗口影响下一场景。

## 红绿与检查记录

使用现有 VS 18 x64 环境、锁定依赖，`CARGO_TARGET_DIR=C:/Users/Tmiracle/.codex/worktrees/plugin-api-community/target01`、`CARGO_BUILD_JOBS=2`、`RUST_MIN_STACK=16777216`；没有安装工具或调用历史构建脚本。本机日志均位于仓库根 `target`。

| 场景/实际命令 | 结果 | 日志 |
| --- | --- | --- |
| 已安装更新入口，原行为 | 0 passed、1 failed；0.35s | `plugin-manager-polish-red-installed-actual.log` |
| 已安装更新入口，修复后 | 1 passed、0 failed；2.47s | `plugin-manager-polish-green-installed.log` |
| 安装后隐藏同版按钮，原行为 | 0 passed、1 failed；0.84s | `plugin-manager-polish-red-actions.log` |
| 隐藏动作与实际已安装页更新 | 1 passed、0 failed；2.41s | `plugin-manager-polish-green-actions.log` |
| 版本行与资料布局，原行为 | 0 passed、1 failed；0.65s | `plugin-manager-polish-red-details.log` |
| 新版列表、资料、长字段与滚动 | 1 passed、0 failed；4.00s | `plugin-manager-polish-green-details-2.log` |
| 启停进行中提示，修复前 | 0 passed、1 failed；0.30s | `plugin-manager-polish-red-global.log` |
| 空搜索选择时提示，修复前 | 0 passed、1 failed；0.50s | `plugin-manager-polish-red-empty-progress.log` |
| `cargo test -p editor-app extensions::management_tests -- --nocapture --test-threads=1` | 7 passed、0 failed、0 ignored；4.81s | `plugin-manager-polish-management-closed.log` |
| `cargo test -p editor-app extensions::worker::marketplace::tests -- --nocapture --test-threads=1` | 9 passed、0 failed、6 ignored；11.02s | `plugin-manager-polish-market-closed.log` |
| `cargo fmt --check` | exit 0 | `plugin-manager-polish-fmt-closed.log` |
| `cargo test --workspace --exclude editor-app` | exit 0；267 passed、0 failed、255 ignored | `plugin-manager-polish-workspace-tests.log` |
| `cargo check --workspace` | exit 0；2.53s | `plugin-manager-polish-check-closed.log` |
| 本轮范围 `git diff --check`、新文件空白、文案/文档链接检查 | exit 0 | 终端读回 |

初次测试准备误用相对来源路径，复制失败后执行了旧用例，该结果未计为红绿证据；绝对复制并刷新时间后才取得真实失败。新版滚动探针曾错误要求末行滚到第二行原始位置，改为实际滚到末行并检查其进入视口；没有放宽行高度或固定头断言。全局操作回归的第二语言曾误取上一场景窗口，补充场景窗口关闭后通过。中间失败不计为通过。

此次最后空选择调整只影响 editor-app 呈现；非 UI 源码与依赖保持同一候选，复用本阶段已执行的 267 项结果。默认跳过的 255 项非 UI、6 项市场用例未计为通过；本轮未执行额外真实 WASM/线上 Pages 用例，没有用历史通过记录替代执行。编译仍有既有未使用项与 Tree-sitter/Wasmtime 链接警告。

最终相关 editor-app 两组共 16 项不同用例通过，最后测试二进制 SHA-256 为 `daa26bc94c0f783a6024371bd054fb6c5ab1bac0cbd13e91b089e2f62ab6aa95`；两组最后记录对应同一产物，之后只补充文档。

## Standards

最终独立审查硬性违规 0、Fowler 判断项 0，无阻断项。按仓库规范、完整十二项 Fowler 基线审查限定差异，其他工作区任务不纳入。最后将通用进度移至空选择判断前的增量已复核：真实队列回执、控件行为、注释与模块可见性保持一致，没有新增授权入口或重复进度提示；审查只读，没有自行构建或编辑。

## Spec

独立审查曾发现“全局启停进度被隐藏”及其“空选择时再次消失”边界，均沿真实队列与原生搜索回归修正。最后固定区域增量复核确认缺失 0、范围蔓延 0、错误实现 0，未解决问题 0；首次本地检查、空搜索和其他目标的操作仍显示真实对象，没有改变安装授权或生命周期。审查只读，没有自行执行构建。

## 交付边界

本轮为宿主源码变更，需要重新构建并重启 Nanobug 才会显示新界面。Windows 自动化原生交互覆盖实际指针、键盘、文本输入、滚轮及缩放；没有人工桌面验收、独立 IME 组合过程或 macOS/Linux 实测。没有提交、推送、关闭 issue、修改用户安装和私有数据或替换运行程序。
