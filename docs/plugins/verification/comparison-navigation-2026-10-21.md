# 比较首次定位、居中导航与双折叠按钮

日期：2026-10-21。状态：本轮实现与专项行为验收完成，Release 候选已生成；全仓测试仍有四项非本轮修改的兼容性／校验测试失败，格式检查存在并行文件差异，未宣称全仓门禁全绿。

## 用户行为与范围

- 从改动文件打开比较，默认定位第一处差异；文件邻项箭头打开另一个文件时同样处理。
- 上一处／下一处差异使用实际可视内容窗口居中目标起始行，排除工具栏与来源标题；边界不足时采用合法滚动裁剪。双／三源、软换行和折叠投影共用原生测量，不以逻辑行号估算像素，不移动源选择或焦点来制造几何。
- 全部折叠、全部展开常驻为两个按钮。没有对应可操作区段时该按钮置灰；部分折叠时两者都可以操作；单段展开、标签恢复和内容变化沿用同一折叠模型。
- 不执行此前尚待确认的提交列表虚拟化方案。宿主调整保持通用，不按 Git 身份分支，不修改独立插件包、SDK 或能力版本。

## 现状与回归入口

现有通用比较只在按箭头时定位，且 `AlignmentGroup::reveal_offset` 和独立原生 `Locator` 都将目标对齐到顶部。提供者已提交折叠与展开的两组 artwork；`UnchangedFolds::available()` 已分别报告可折叠／可展开，不需要重新设计公开契约。

沿用仓库规定的实际独立 Git 包 → 公开 Manager → 原生鼠标／布局入口。优先增强已有保留比较测试验证双按钮及状态，再以真实改动文件树和前后箭头验证初次定位与居中。首轮双按钮回归在旧实现上失败于仍存在 `diff-toggle-unchanged`；日志 `target/comparison-navigation-2026-10-21/folding-red.log`。

开工时已有大量未提交改动与并行工作。`target/comparison-navigation-2026-10-21/baseline/` 保存 Git 状态、原有工作区 patch、源码摘要及直接相关原文件；不会将其计为本轮成果。

## 实现

`editor-ui` 的对齐组新增通用 `reveal_offset_at(index, byte, anchor)`，原 `reveal_offset` 保留顶部定位语义。比较导航使用 0.5 锚点，将目标起始行按原生测量后的共享内容窗口居中；等待布局的请求绑定成员 epoch，折叠后等待新几何，不复制文本、不根据逻辑行数估算像素。独立滚动回退复用原有有界 `Locator`，增加居中目标而不更改预览视口使用的顶部／行内比例语义。

显式打开比较将差异序号重置为第一项，并在挂载该提供者折叠模型后发起定位。前后导航使用相同定位逻辑，新请求清除已过期的独立窗格定位，避免对齐恢复后旧请求竞争滚动。折叠和展开使用既有两组图标、翻译键与本地 Button 的禁用外观／行为，执行前再次检查当前活跃视图的实际可用状态。

## 实际验证

测试宿主通过以下命令构建，随后复制为记录目录内的 `host-tests.exe`；它使用 Release 依赖及应用／本地 UI 的 opt-level 0，属于行为测试程序。

```powershell
cargo test -p editor-app --release source_viewport --jobs 2 --config 'profile.release.package.editor-app.opt-level=0' --config 'profile.release.package.editor-ui.opt-level=0' -- --nocapture --test-threads=1
```

| 检查 | 实际结果与日志 |
| --- | --- |
| 原生视口及回退定位 | 5 通过、1 忽略；`viewport-final.log`。新增回归检查开启／关闭软换行的 CRLF／中文／emoji 来源，目标行居中、首尾裁剪、水平偏移及选择保留，无文本 Change 事件。忽略项没有计为通过。 |
| 从实际 Git 改动文件树点击打开及前后导航 | 显式 `--exact --ignored` 执行 `extensions::windows_tests::history_preview::navigation::git_changed_file_and_difference_arrows_center_native_viewports`，1 项通过、16.69 秒；`navigation-verified.log`。覆盖深处首差异、三源长行软换行、双源前后导航、原文本／选择保持，以及根提交无公共区段时两个按钮保持可见且不可操作。目标通过实际 glyph 投影与排除标题的内容轨道比较，容许 12px 的原生边距／行几何差异。 |
| 保留比较标签及折叠操作 | 显式 `--exact --ignored` 执行 `git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs`，1 项通过、14.40 秒；`retained-comparison-final.log`。覆盖两按钮、单段展开、保留标签、草稿与 Undo、源文本／选择及资源清理。 |
| 普通／合并历史比较 | 显式 `--exact --ignored` 执行 `git_inline_history_preview_opens_readonly_two_and_three_pane_tabs`，1 项通过、12.55 秒；`history-preview-final.log`。覆盖双／三源、深浅主题、原生文件箭头、固定原提交及只读来源。 |
| 通用比较绘制 | `host-tests.exe 'editor::comparison::' --test-threads=1`，11 项通过；`comparison-tests.log`。 |
| 本次 11 个 Rust 文件格式 | `rustfmt --check --edition 2024 --config skip_children=true <本次文件>` 通过；`scoped-format.log`。 |
| 空白检查 | `git diff --check` 通过；`diff-check.log`。 |
| 最终 workspace 编译 | `cargo check --workspace --jobs 2` 通过，2 分 48 秒；`check-final.log`，保留既有警告。 |
| 完整 workspace 测试 | `cargo test --workspace --exclude editor-app --no-fail-fast --jobs 2`：87 组，429 通过、4 失败、336 忽略；`nonui-complete.log`。未执行的 ignored 不计为通过，未以专项替代失败的全仓测试。 |
| 全仓格式 | `cargo fmt --check` 失败；`fmt-current.log`。差异位于并行的 SDK 开发／诊断、分支保留及对齐性能等文件，本轮未代改。本轮 11 文件的最终单独检查为 `scoped-format-final.log`，通过。 |
| 依赖边界 | `cargo tree --workspace --edges normal,build,dev --all-features` 通过，无 `gpui-component`；`dependency-tree.log`。 |

所有实际包测试均设置 `NANOBUG_GIT_CANDIDATE` 为先前构建的独立 `git-0.1.22.zip`，SHA256 `239a0033145048333fff43528287b1cf74c8dd2cba6ec3af7484cc0e5c30ff25`。包经隔离 Manager 安装，没有改动用户安装目录或独立 Git 源码。

### 过程失败与工作区复查

首次导航回归在旧实现上失败于深处首差异不可见，见 `navigation-red.log`。中间夹具错误假定合并两侧的 topo-order 固定顺序，随后改为按实际发布的提交标题定位；另外一次来源断言放在打开比较之前，已移到真实文件点击完成后。这些夹具失败不计为产品回归通过。最终 `navigation-verified.log` 才是完整工作流结果，临时调试输出已删除。

首轮全仓测试有两项能力包协商测试失败，编译检查同时报告源码已存在的 `NegotiationFailure`／`negotiate_detailed` 不存在。按仓库缓存诊断记录核对 metadata 后，仅清理主工作区的 `plugin-protocol` Debug 缓存，保留第三方依赖和交付 ZIP；`protocol-cache-clean.log` 记录 4,113 文件／5.0 GiB。重复相同 `cargo check --workspace --jobs 2` 通过，18.49 秒，见 `check-after-cache.log`，没有补造接口或修改无关协商实现。

测试编译还曾遇到并行 SDK 工具工作中的 `sdk_export::digest` 尚未出现；该编译失败后误运行的上一份测试程序没有计入最终结果。最终 `viewport-final.log` 编译成功后才重新复制程序并执行已通过的 `navigation-verified.log`。

完整 workspace 的四个失败如下；均位于本轮没有修改的插件运行时兼容／校验测试，主要断言旧错误文案及字段信息。它们仍需在对应工作中处理，本轮没有通过改变期望或跳过测试掩盖失败：

- `capability_packages::incompatible_required_capabilities_and_base_versions_are_rejected_at_inspection`
- `capability_packages::cloud_capability_versions_are_admitted_independently_from_package_version`
- `compatibility::old_registry_reports_update_requirement_before_accessing_component`
- `retired_ui_contract::retired_presentation_and_host_artwork_are_rejected_before_installation`

普通 Release 首次构建遇到并行 `plugin-runtime::development::report` 文件尚未落盘，见 `release-missing-parallel-module.log`；第二次遇到并行 CLI 尚未补齐 `Descriptor.reporting` 和 `Manager::http_producer_count`，见 `release-parallel-report-fields.log`。确认这些源码已补齐后，重复同一入口，最终成功，没有在本轮修补这些范围外实现。

## 候选与来源

`target/comparison-navigation-2026-10-21/Nanobug.exe` 为普通 Release 候选，79,849,472 字节，SHA256 为 `ea2e529e36b1ec659795d06db4971aa959f24478b08409f9c6e150da778c2c7f`。构建入口与产物记录为：

```powershell
cargo rustc -p editor-app --release --bin editor-app --jobs 2 -- -o C:/Projects/RustProjects/Editor/target/comparison-navigation-2026-10-21/Nanobug.exe
```

`release.log` 记录普通优化构建通过，4 分 05 秒；没有应用测试构建的 opt-level 覆盖。`artifact.json` 保存命令、路径、长度、SHA256 及测试程序摘要。

候选来自包含既有改动和并行工作的当前工作区，不是仅含本轮补丁的冻结源码快照。`release-inputs-before.json`／`release-inputs-after.json` 与 `changed-during-release.json` 如实记录了构建期间其他任务对 pane／性能测试、诊断报告和 worker 等 12 项输入的更新；本轮 11 项导航／折叠相关输入未在该构建期间变化。不将这个候选表述为其他并行任务的完整验收或可重现冻结版本。

用户既有 `docs/窗口布局拖放规则.excalidraw` 仍为开工时 SHA256 `b99d9031fb935772e3ad7c5cec2a3add61622a949c5ce8144ac970dfa9f934c6`。没有改动该文件。

以上原生交互采用实际 GPUI 原生控件、输入和测量，不将 TestWindow 结果表述为 Windows D3D 截图、系统 IME 候选窗验收或性能基准。未自行提交、推送、安装插件或重启用户程序。

返回[插件目录](../README.md)。
