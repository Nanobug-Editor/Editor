# 07：独立双轴审查记录

状态：最终独立双轴复核已完成；Standards与Spec均无未解决项，行为与Windows原生证据已读回。普通源码提交 `8ee28952014a6533253fc83e17c3410d88a76e48` 已推送核对，#99随后已读回closed/completed，详见[主验收交付记录](07-activation-and-compatibility.md#普通交付及全批次议题读回)。源码审查通过与作者实际执行的测试分别记录。

固定起点为 `90ceab915d63c2e6b8cb260c6f28acfc45adc7fc`，规格为[社区生态方案](../../specs/plugin-api-community-foundation.md)、[07 工单](../../tickets/plugin-api-community-foundation/07-activation-and-compatibility.md)和[共同完成条件](../../tickets/plugin-api-community-foundation/README.md)，平台不变量仍按 [plugin-api-platform](../../specs/plugin-api-platform.md)执行。规范依据为仓库及用户当前 `AGENTS.md`。Standards 与 Spec 由两个独立代理审查；作者负责实际执行测试和验证交付。

全部审查快照通过外部临时索引及无引用 Git 对象冻结，不移动实际分支 HEAD，也不修改真实索引。完整源码点 `aacb39db23b12b2580f680b2511ae4fad1d03d5e` 及随后各增量见主记录。旧 SDK 迁移修正点为 `5eaf5604854d1cb0fc3d3d36592807d8874ecd15`，tree `522aaf12103a9aa744c6351889a21df248574e54`；与前一 `95bfa8a0f3638cb33260200d7fedd1c595362a9e` 的差异仅为实际旧 SDK 迁移测试。应用矩阵后续点见下节；分支在此时仍未提交本单。

## Standards

完整审查已覆盖 147 个文件。初轮 native 错误消息未走 i18n、跨功能测试放置不符职责的两项已在 `aacb39d` 复核清零，后续增量保留独立结果。生成代码附模块或行为注释，公开接口维持通用能力边界，SDK 导出不包含 Markdown。

最近 `95bfa8a → 5eaf560` 仅审查 `crates/plugin-runtime/tests/support/lifecycle_scenarios.rs` 的迁移阶段测试。硬规范违规 **0**：prepare 失败保留旧 owner；activate 失败恢复新 owner，两支仍验证旧版本、latest 私有数据及后续成功升级。新增注释解释时序与生命周期，原磁盘及 shutdown 断言保留。

十二类 smell 均逐项检查：神秘命名、重复代码、依恋情结、数据泥团、基本类型偏执、重复切换、霰弹式修改、发散式变化、夸夸其谈的通用性、消息链、中间人、被拒绝的遗赠，**无新增可行动问题**。迁移测试中的重复写入承担不同准备时序的验证，不建议机械抽取。

最近修正版 exact 实测 `1 passed / 0 failed / 0 ignored`，160.76 秒，exit 0；旧 Red 79.22 秒保留。审查代理未自行运行该用例，这个 Green 只对应迁移用例。

## Spec

完整及后续增量所发现的排队配置 witness、native plan 热应用、确切 warm owner、需求祖先取消与自然 deadline、旧 eager 夹具等问题已分别修正和独立复查。真实行为对应主记录中的 Red/Green；尚未执行的矩阵项不由本段意见推定通过。

最近 `95bfa8a → 5eaf560` 无新增问题，保留此前结论：

- **遗漏：0。** 准备失败分支检查真实 Guest 拒绝及原 owner 不变，符合平台准备与恢复不变量；激活失败分支仍在成功 prepare 后写入 latest，再检查失败和恢复 owner，覆盖最终数据一致性要求。
- **范围蔓延：0。** 仅修正实际旧 SDK 测试的阶段前提，未改变生产或 Guest，符合 T34。
- **错误实现：0。** 两支保留 registry 1.0.0、私有 latest、最终成功迁移 v2、原项目文件不变及 shutdown 资源零检查。

Spec 已读回原 Red `0/1/0`、79.22 秒和修正版 Green `1/0/0`、160.76 秒、exit 0，EXE SHA256 `F8FD99D72BE288AAD8D464FB1D5E1D5C3F638CAF8936E357C2733903733FD510`。该轴未运行测试，这一项通过不代表整体验收完成。

原生配置输入夹具随后冻结为 `923971fc4c09bbae9ff9e1c0ce98191fce82dc2f`，tree `75c8a63b6f8789afc16a1c8929be910a2704c28c`；相对 `5eaf560` 仅 `settings_hot_tests.rs` 的 18+/3−。原 App5 真实 Red 为 `0/1/0`、128.06 秒：GPUI 的空字符串输入不删除选区，表单继续正常拒绝原 129 字节草稿。修正使用真实 Ctrl+A、Backspace，再输入非空文本，并检查真实 InputState 生成的 `Work::SetSetting.value`；全部原 UTF-8、空引用、Guest Validate、冷态不激活及 owner/resource 断言保留。该单文件源码 SHA256 为 `54BD91DEB2CFE943EB3BAE4FABCBED746D935901AF3610E31512E30C6C5BE9A9`。

该增量 **Standards** 硬规范违例 0，十二类 smell 均无新增可行动问题；**Spec** 遗漏/范围蔓延/错误实现 `0/0/0`。两位审查员未执行 Cargo 或原生交互。随后 Root 的修正版 exact 实测为 `1/0/0`、154.39 秒、exit 0，EXE SHA256 `00A4DFD98D49853EF000F8DAED13E66B35BDA3D774802B762B04ABEB5425824A`；日志 `../target01/07-app-empty-input-current.log` 与同名 `-result.json`。这是实际行为结论，不从源码意见推导通过。

## 应用矩阵失败后的增量

以下是后续独立审查的实际阶段；源码意见不代替 exact 运行。原十二项均已执行，逐项真实结果与历史失败详见主记录。

| 冻结点 | Standards | Spec | 行为边界 |
| --- | --- | --- | --- |
| `317ef3d4cb970488880d21f132c818c599ffed8d` | 硬违例 0；十二类 smell 无新增项 | 错误实现 1，P2 | 结构树测试的最后绘制可能代替完成回调清理；不能凭旧树已空或原 Arc 退役推定通过 |
| `3a58404b6e36bd8c3b9183f8b748a3473e4f7acd` | 硬违例 0；十二类 smell 无新增项 | 遗漏/蔓延/错误 `0/0/0` | 快捷键 restart 保持冷态，以独立公开 View 需求获得新 owner，再交付原排队请求；FileView 仅增加失败诊断 |
| `cea81174cf8ce085e0967729ba08bfcb0dd59e32` | 硬违例 0；十二类 smell 无新增项 | 遗漏/蔓延/错误 `0/0/0` | 删除一行私有缓存诊断，保留公开 Manager owner；E0616 编译失败单独记录，不计行为 Red |
| `ee8029827a9fd32bb4145bab6513e28217148c9d` | 硬违例 0；十二类 smell 无新增项 | 遗漏/蔓延/错误 `0/0/0`；原 P2 已闭合 | 结构树退出前观察真实完成通知与私有 target/pending 清空，最终绘制后置；当前 exact 实测 Green |
| `2515a09a0a4f2f7640d8b11d71b994571dcf379f` | 硬违例 0；十二类 smell 无新增项 | 遗漏/蔓延/错误 `0/0/0` | 所选冷文本 FileView 的正面积绘制驱动既有需求探针；保留唯一 EditorState、错误回退与取消机制 |

`ee8029827` 的 tree 为 `916a16a15d6ec1c9ddc66476b3c4d828b95e5f71`。物理跨功能文件仍位于 `src/tests/outline/lifetime.rs`，由现有 `outline/model/tests.rs` 通过测试模块挂载；共享 helper 仅 `cfg(test)` 可见，没有扩大生产接口。保留的 `Window.observe` Subscription 仅在同一 changed 文档、原 Arc inactive、target/pending 均 None、树空时记录通知；等待循环同时要求这些状态和通知标记，八秒期限内没有额外 pump/draw/notify，之后才绘制并检查 empty、无节点和无 loading。

Root 从最终 `855EF3859267485F3956615601B619BD2F0E0801BC2D9073015635F116756377` EXE 实际运行新的 `outline::model::tests::lifetime::native_structure_trap_clears_tree_without_another_document_event --exact --ignored --nocapture --test-threads=1`，结果 `1/0/0`、108.21 秒、exit 0；原 114.17 秒 Red 保留。原 P2 同时取得修正源码与真实完成通知的行为证据。相同最终 EXE 的快捷键生命周期 exact 也实际 `1/0/0`、138.18 秒、exit 0，保留三次冷 restart、旧前缀/排队/延后拒绝与 fresh 正控、受限工作区及持久绑定断言；原 54.69 秒 Red 保留。

App12 的诊断 exact 实际为 `0/1/0`、42.18 秒：所选 SVG FileView 可见，目标及 source version 正确，但公开 Manager owner 为 None、布局不存在、真实工作队列为空。它确认冷文本回退没有触发首次需求，属于生产缺陷，不是 SDK 构建或手工提前激活可替代的验收。修正版冻结为 `2515a09a0a4f2f7640d8b11d71b994571dcf379f`，tree `c0b72138cb5f78d217ce2216eaff8b01d7ab0533`；只在通用的所选冷 FileView 原生回退区域组合既有 masked paint probe，复用可见性、generation、取消和实例归属机制。两个轴均清零后，Root 实际 exact 为 `1/0/0`、51.81 秒、exit 0，EXE SHA256 `855EF3859267485F3956615601B619BD2F0E0801BC2D9073015635F116756377`；`../target01/07-app-final-three-actual-results.json` 保存原命令、时长与日志。画布操作、未保存文本、传输错误及恢复、禁用回收的原断言全部通过，没有增加提前需求或 pump 轮次。

`../target01/07-final-app-twelve-actual-manifest.json` 已核对十二个唯一 exact 的真实日志与计数，合计 `12/0/0`；清单 SHA256 为 `AABEF5EEB33A384537E29BEB5B941C96D89614E44BE3C33EEA168A4D6D9B6A13`。清单分别标明实际 EXE 指纹及已保留二进制，较早 596/513 EXE 被后续构建替换，仅保留原指纹、命令和日志，不宣称其原二进制仍可运行。Runtime 矩阵与其额外窄探针单独计数；最终文档、Windows 原生行为证据与普通提交/推送仍待复核，不关闭 #99。

## 最终行为、原生及门禁复核输入

源码固定点 `2515a09` 之后未再修改 `crates/` 或 `plugins/`；构建前临时索引确认1236文件相对该点delta0，见 `../target01/07-pre-final-source-verification.json`。Runtime最终十九项清单 `07-final-runtime-nineteen-actual-manifest.json` SHA256为 `7860A8D3DCFA71FFCCD43E7F013C8BB91F34463F884D3E05A645848C8F66F7AB`，真实 `19/0/0`；额外Query探针实际 `1/0/0` 单独记录。上述App十二项已实际全部Green，outline原P2具有最终绘制前真实通知、原source退役及私有target/pending清空的源码与108.21秒行为证据。

最终 `07-final-delivery-build-results.json` 四个实际命令均exit0：fmt2.7153891秒；非UI workspace260通过、0失败、246ignored、14.4667054秒；check2.6448394秒；生产Hostbuild26.1142877秒。245→246仅为额外ignored探针登记，不把默认跳过算通过。最终生产Host `E81B1F9733784CCC41B436525DCAF260A5C6EE80451E9CE1DEC056E58B12DBC0` 的SDK50逐文件相同、0Markdown，未重建输入未变的包。

[Windows生产原生记录](native-07.md)保存实际冷SVG自动激活、原生配置作用域/热应用、独立19秒手动取消、迟到分块拒绝、31bytes完整结果/私有缓存、真实Pinyin候选与提交、Tab/Return、浅/深主题、14→16px、最大化/还原及主窗口UIA。物理英文与SettingsUIA未扩大为通过；中英文新增表单已有App5实际GPUI证据，03/04的SettingsUIA既有限制保留。正常控制器/server stop均exit0，exactHost/QA进程和loopback监听归零，四个workspace文件byte/hash不变，缓存31bytes严格匹配两块UTF-8；`07-native-final-cleanup-result.json`实际记录于2026-10-11T22:00:26.9721095Z。

这些是最终双轴复核输入，尚未由此宣称普通提交、推送或#99关闭。T35实际包组合与本轮物理范围分别记录，不以旧包产物的复用代替受影响行为重新执行。

## 最后独立双轴结论

固定范围 `90ceab915d63c2e6b8cb260c6f28acfc45adc7fc` → `b3bf9fc5342dc0421947d83fdd3727b5a5bc511f`，tree `ff7d7a2d9649250c62e66be44f8457a2f742e435`。`../target01/07-final-delivery-review-snapshot.json`保存165文件清单、时间、真实index未变和HEAD仍90的核对；相对2515源码delta0、仅7份文档变化。主MD在该点542行，SHA256 `BEA041E61FBD622344D9259F559DD381BE112F0209BB6E55D258FD3DF9525121`；此后仅追加实际审查及交付状态，不把新状态文件的hash冒充该审查输入。

- **Standards：硬规范违规0，可行动smell0。** 完整源码及连续增量审查保留，十二类smell逐项均无问题。最后独立读回Runtime19/额外Query1/App12原始日志，11份保留EXE指纹全部匹配，App1–3只保留原日志/指纹的限制正确；SDK实际50文件、0Markdown、字节差异0，链接与差异检查通过。
- **Spec：遗漏0，范围蔓延0，错误实现0。** 独立读回最后三份App原日志、全部清单和门禁，outline原P2在强观察源码与108.21秒真实行为两方面闭合；C01–C06证据足够。四个workspace文件及31-byte缓存再次读回一致，物理中文、GPUI中英文/主题、八包组合、SettingsUIA及平台限制划分准确。

两轴最后一轮只读复核，未重复Cargo或操作UI；实际行为来自作者执行及原始结果。最终复核时#99尚OPEN，93–98已closed/completed，92仍OPEN且正文与执行前baseline逐字相同。普通提交、推送核对及关闭由后续真实交付步骤完成，不以审查报告代替这些动作。
