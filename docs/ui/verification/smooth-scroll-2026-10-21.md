# 通用平滑滚动

本页保留第一阶段的有限缓动实现与验收。后续用户要求的速度连续惯性、原生高刷和当前实现见[惯性滚动与原生高刷](inertial-scroll-2026-10-21.md)。

日期：2026-10-21。状态：源码实现、自动回归及 Windows/D3D 组件验收完成；范围和实际命令结果见下文。

## 设计与接入

`editor-ui/src/scroll/` 在既有滚动状态上保留有限的动画目标和最后请求坐标。GPUI Kit/Base 0.7.0、GPUI 0.3.7 的公开 `ScrollbarHandle`、`motion::transition_with_status`、窗口帧调度和 Root 插件承担底层能力，没有复制上游控件或建立第二份文档、选择、布局与 Undo 状态。

Windows 普通滚轮的 `ScrollDelta::Lines` 累计到目标位置，以 Base 默认 cubic easing 在 150 ms 内收敛。连续输入重新定位终点，反向输入丢弃该轴尚未完成的原方向位移；触控板的 `ScrollDelta::Pixels` 继续直接交给原始控件，差异比较的共享投影视口直接更新其原有句柄。系统减少动态效果时使用即时滚动。

| 区域 | 接入方式 |
| --- | --- |
| 普通纵／横滚动容器、设置、菜单、对话框、交互列表 | 共用 `ScrollableElement`，保留调用方原有句柄、键盘 reveal 和滚动条 |
| 文档编辑器、只读代码、多行表单 | 完整原生输入外框上的适配；通过公开 `scroll_offset` / `set_scroll_offset` 访问原始实体，最终布局边界仍由 Base 校验 |
| 两／三栏差异比较 | 共用已有投影垂直位置与原生横向请求；隐藏的全高源输入不竞争共享滚动，原生独立模式保留原事件路径 |
| 文件树、大纲树、运行配置树、插件树 | 复用 Base `UniformListScrollHandle`，保留虚拟列表及选择、键盘和 disclosure 行为 |
| 文档标签、侧边标签、问题、消息、快捷键、代码动作、插件详情与状态列表 | 复用各区域已有句柄；标签横向滚轮沿用原有轴映射 |
| 插件公开 `Scroll` 容器 | 与宿主相同的通用句柄适配，无插件 ID 或业务分支，也没有协议／SDK 变更 |

Canvas、图片预览和终端的原始 Wheel 输入包含插件或终端的缩放、鼠标协议等业务语义，继续交给原有处理器，不把所有 Wheel 事件视为可动画的滚动位置请求。它们的既有滚动条和公开事件保持原契约。

Root 路由在原生 capture 观察器之后处理普通滚轮，保留插件滚动意图、定位撤销等观察行为；候选由原生 capture 监听器按实际命中关系登记，缓冲只保存弱引用并在同一事件结束时释放。GPUI 重用缓存视图的监听器时仍能登记，不依赖每帧重新绘制所有视口。普通嵌套容器从内向外选择仍可移动的句柄；比较视口在自己的隐藏源输入之后登记，后绘制的菜单、弹窗仍保持原优先顺序。鼠标按下、键盘输入、窗口鼠标退出、精密像素输入及其他原生位置变化取消未完成动画。

## 性能约束

- 每个视口仅保留目标、最后请求位置与动画标志；每帧只读取原生位置及内容／视口尺寸，不扫描文本、差异行、树项或列表数据。
- 上游虚拟化和已验证的差异几何缓存继续生效；不会为了缓动复制、重新布局整份内容。
- 只有活动视口调用 Base 动画采样；静止视口不创建缓动状态、不采样动画、不请求动画帧。动画没有独立线程、重复定时器或后台轮询。
- 高频滚轮只更新同一目标，并由窗口合并绘制；不会为每个输入创建任务或执行列表布局。动画只通知所属的现有视图。
- 程序定位、滚动条拖动及 Base 布局钳制优先；关闭／移除后不存在继续修改旧视口的动画任务。原生监听器的弱挂载租约区分缓存复用与移除后重挂载，旧目标不能在重挂载时恢复。原生输入的私有最小偏移由下一次原生布局钳制后终止，未为了预估边界重新测量文档。

以上是实现约束和可观察工作量验收，不等于对所有文件规模、GPU 和设备承诺固定帧率。

## 验证

开工时工作区已有大量未提交修改，包括 `editor-ui` 提取和滚动条宽度动画。既有 785 个 UI／宿主／文档文件保存在 `target/smooth-scroll-2026-10-21/baseline/`；本次变更按此快照核对，不把这些既有工作归为平滑滚动成果。没有提交、推送或修改独立插件包。

`cargo metadata --no-deps --format-version 1 --offline` 已确认 `workspace_root` 为当前 `Editor`，`target_directory` 为其 `target`。所有命令直接调用 Cargo，没有构建其他源码副本或调用归档脚本。

| 检查 | 实际结果 |
| --- | --- |
| 首轮 `cargo test -p editor-ui scroll::tests:: --offline --jobs 2 -- --test-threads=1` | 4 passed；中间帧与终点、原生编辑／树／标签／比较、连续与反向、精密像素、嵌套边界、程序定位、移除及减少动态效果；不是后续完整回归的替代 |
| 高频输入、缓存视图、移除／重挂载、原生延迟定位、指针中断及深浅主题／150% DPI | 8 项新普通回归通过；一万项树的 120 次滚轮在绘制前不调用行渲染器，后续绘制仍仅处理可见行，终点无额外动画帧需求 |
| 真实 Windows/D3D 独立窗口 | 显式 1/1，2.09 秒；另在输入组合回归中再次通过；日志 `native-windows.log` |
| `cargo fmt --check` | 退出 0；日志 `fmt-final.log` |
| `cargo test --workspace --exclude editor-app --offline --jobs 2` | 84 组结果，423 passed、336 ignored、0 failed；日志 `workspace-tests-final.log`，ignored 未计为通过 |
| `cargo check --workspace --offline --jobs 2` | 退出 0；日志 `workspace-check.log`；保留已有 warning |
| 原生输入组合回归，见下方完整入口 | 26/26，0 ignored；包括本次 8 项普通回归、独立真实窗口，以及原生选择、指针释放、IME preedit/cancel、Undo、比较共享滚动和 DPI；日志 `native-input-regressions.log` |
| 相关 `editor-app` 普通回归，见下方筛选 | 36 passed、3 ignored、0 failed；比较呈现、预览标签／IME、源视口、插件视口与 Canvas 滚动条；日志 `app-tests.log` |
| 冻结 database-tree 0.1.1 真实 Package 经 Manager 的树交互 | 上一行的 3 项 ignored 已另外显式 3/3，119.13 秒；Package 入口校验既有 ZIP 的固定摘要，无需重建或更改包；日志 `package-tree-tests.log` |
| 显式 1800 行长差异，普通与密集背景两项 | 2/2；普通 8 轮约 537–550 ms，密集背景 2 轮约 756–763 ms；逐行几何探测、对齐重建、背景原生行查询均为 0；日志 `long-diff.log` |
| `git diff --check` | 退出 0；保留既有 CRLF 转换提示；日志 `diffcheck-final.log` |

以上日志均位于 `target/smooth-scroll-2026-10-21/`。长差异数字是未优化 `test` 配置下整次滚轮分发和有限后续绘制的耗时，不能当成单帧时间、Release 帧率或本轮性能提升百分比。缓存与虚拟化测试以实际工作次数断言，避免机器负载造成误判。

真实窗口使用正常 `gpui_kit::application`、Base Root 和 D3D 绘制，不使用 TestPlatform。通过生产 `Window::dispatch_event` 投递标准平台滚轮，观察到约 45.9、87.7、115.0、124.9、126.0 逻辑像素，首个可见源行索引（从 0 计）随之从 2 变为 6，源文本始终一致。没有把这个组件验收表述为物理鼠标／触控板驱动及系统 IME 候选窗的完整人工矩阵。

显式与定向入口：

```powershell
cargo test -p editor-ui smooth_scroll_real_windows_paints_intermediate_native_positions --offline --jobs 2 -- --ignored --nocapture --test-threads=1
cargo test -p editor-ui --offline --jobs 2 -- scroll::tests:: aligned_pointer::tests:: aligned_editor::tests:: --include-ignored --skip long_diff --skip long_marked --test-threads=1
cargo test -p editor-ui --offline --jobs 2 -- long_diff_scroll long_marked_diff --ignored --nocapture --test-threads=1
cargo test -p editor-app --offline --jobs 2 -- editor::viewport::tests:: editor::comparison::render_tests:: tests::preview_tabs:: ui::plugin::tree_tests:: ui::plugin::tree::refresh_tests:: ui::plugin::viewport_tests:: ui::plugin::scrollbar_tests:: --test-threads=1
cargo test -p editor-app --offline --jobs 2 -- ui::plugin::tree_tests:: ui::plugin::tree::refresh_tests:: --ignored --test-threads=1
```

首次 workspace 检查曾遇到项目窗口 profile 字段和两处插件安装接口参数不一致；位置均在本次改动范围外，且安装调用的旧形式可在开工快照中核对。随后这些位置由并行工作区任务修正，本次未修改它们；以最终重复检查结果为准。

本次交付本地源码，没有替换用户正在运行的程序、打包发布、提交或推送。普通目录与输入行为继续由原生控件承担；自定义 Canvas、图片和终端 Wheel 的业务语义边界见上文。

返回 [UI 文档目录](../README.md)。
