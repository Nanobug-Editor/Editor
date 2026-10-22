# S4 生态组合：阶段证据

状态：实施中，尚未满足全部验收，不关闭 #124。

## 已实现的公开接缝

- `plugin.services 1.2`：提供者与消费者都显式声明 `collection:true` 后才可 `OpenSet`。
  普通 `Open` 继续单选；用户/项目明确选择限制集合，不被组合发现绕开。
  最多 16 个成员，成员引用仍由调用者持有。`ValidateSet` 校验同实例实际打开的集合代次。
- `language_control::Request.service_sets`：引用该实例已有的集合快照；空集合也有授权边界。
  集合成员、选择、配置或实例更替会收窄请求、绑定与所有导入模块的存活期。
  没有导入的元数据请求仅封闭其结果，不虚构待卸载模块。
- TS 候选 0.1.2 的 `typescript.adapter 1.1.0` 在异步 attach 准备中发现
  `language.contribution`，自己持有服务引用并导入 immutable exports。只读配置 hook 不调用服务。
  组合声明包含角色、slot、compose/exclusive、ABI、配置和 before/after，冲突与环显式失败。
- Vue 候选 0.1.5 的 `vue.language-plugin/1` 接口限制 template/style/project/types 各角色 hooks。
  外部 factory 在上游 warn/filter 之外初始化，错误与不兼容版本不被吞掉；新增或改变的映射
  校验源块与 UTF-16 边界，坏映射使完整候选失败。原生模块仍不具有 WASM 沙箱保证。

## 实际验证

1. `cargo check -p plugin-runtime --tests -j1`：OpenSet 首轮编译通过。
2. 独立 `tests/fixtures/service-set` 通过宿主 `--plugin-cargo ... build --target wasm32-wasip2 --release -j1`
   构建，使用导出 SDK `8dacb107...`；随后
   `cargo test -p plugin-runtime --test service_sets -j1 -- --ignored --nocapture`：首两条 **2/2 通过**，182.38 秒。
   覆盖单选与组合区分、1.1 未声明消费者拒绝、首个成员激活、选择变更后的旧引用失效，以及模块
   已借出且 exporter 仍存活时旧 lease 不能再次 attach、受管进程被回收。
   早先 debug WASM 输入 39.8 MB 的尝试因夹具缺少委派权限和冷构建超时失败，不计作通过。
3. TS 独立仓库 `node --test test/composition.test.mjs test/vue-contributions.test.mjs`：**4/4 通过**。
   实际 TS 5.9.3/LSP 进程与封闭 Vue legacy/modern 模块接受未知 factory 组合；真实类型贡献和
   未保存 `.vue` 映射诊断可见，合法编辑清除错误，项目磁盘内容不变。坏 factory、独占冲突、顺序环
   与越界源映射拒绝；未将模拟诊断作为证据。

## 待验证与限制

- 新 SDK `06f3a760...` 已经通过公开入口重新构建 release service-set 夹具；四条公开测试 **4/4 通过**，188.18 秒，
  新增空集合→成员、贡献返回空→成员变化及纯元数据晚结果封闭均实测。随后
  `cargo test -p plugin-runtime --test language_contexts -j1` **7 通过、1 ignored**；忽略的真实 SDK 项目 hook 测试未在本轮执行。
- TS .6、Vue .7 与未知生态 .0 的完整生产 actor/原生 UI 首轮 **失败**（352.05 秒）：
  三包安装启用正常，但首次 Vue→TS resolve 在 URI 大小写转换后报 `Document is not open`，
  尚未到贡献准备。记录 `target/s4-ecosystem-package.log`；不将 native worker 的单独通过替代此验收。
  已增加下一轮 provider-ready 阶段失败即报，准备 180 秒、诊断 30 秒，不加宽原有总预算。
  独立未知贡献包源码位于 TS 仓库 `test/fixtures/ecosystem`，不进入宿主 SDK 或发行。
- `test/vue-role-contributions.test.mjs` 对当前封闭模块 **7/7 通过**：legacy/modern 的未知模板、
  样式编译产生真实 TS 诊断且定义/重命名回原始源位置；项目布尔选项贡献生效、相反设置整包拒绝；
  缺少 quill/ink 专属扩展只标记对应块，保留 script 2322 与悬浮。样式首轮 RED（内建 CSS 先吞掉
  所有语言）后修为外部处理器优先声明、undefined 回退；项目配置首轮 RED 后加入限定数据接口。
  legacy 过程中发现复制 options 丢失延迟 helper resolver，已保留同一 options 对象修复。
  Vue 代理另补整个临时项目与 junction 依赖目标的前后 SHA 树比较；其结果独立记录，不靠只检查 App.vue。
  Package/UI 的角色组合、事务及撤销联合验收仍未完成；角色 worker 通过不等于 S4 完成。
- 旧 TS 0.1.1 与 Vue 0.1.4 ZIP 保留；源码候选版本提升不等于已经发布或完成验收。

## S1 共用路径阻塞修复与候选

- TS .4 加入固定 JS/JSX/TS/TSX grammar/queries，.5 修 Windows verbatim workspace realpath；
  .6 将同一修复扩展到 host-sealed export 路径。canonical-import 真实 stdio RED `EISDIR lstat C:`
  → GREEN，额外邻居模块仍被拒绝。
- .7 修复上述原生 UI 首轮阻塞：path control 只采用 host sealed sources 的精确原始 URI 和版本，
  不从磁盘推测 revision；定义、引用与重命名保留已打开文档的小写盘符 URI/version；重复文件身份拒绝。
  `node --test test/control-path.test.mjs test/project.test.mjs test/control.test.mjs` **10/10 通过**，
  两个新路径回归都先观察到真实失败。`tsc -p jsconfig.json` 通过。
- 最新已打包 TS：`dist/typescript-0.1.7.zip`，SHA-256
  `eb308449151e5d8db34e6944e1aa11bc090dba2653189c9f9b4566344fbadc39`。
  通过宿主公开 `--plugin-package .` release、SDK `06f3a760...` 构建，旧候选 ZIP 均保留。
  .7 宿主 mixed 首轮已越过 URI 阻塞，但在 resolve 后宿主停止并重启三个 native 进程，随后 describe
  报 `Unknown or retired project context`；84.98 秒失败，日志 `target/s1-mixed-release017-ts017-test.log`。
  新增真实 stdio 的独立业务调用回归：同一 consumer 四轮 resolve/describe 保留同一引用，另一 consumer
  退休不影响该引用，只有准确所属 consumer 退休才拒绝。`node --test test/project.test.mjs test/control-path.test.mjs`
  **5/5 通过**；没有为此修改 TS 生产实现，宿主持续需求保留边界由父任务定位。
  不以打包或原生独立进程成功报告整链完成。本轮只推进新 S4 #124，旧工单保持原状态。

## 适配器原生结果身份边界

- `test/adapter-uri.test.mjs` 真实 Vue CSS 定义首轮 RED：返回 `file:///C:/...`，而原生打开身份是
  `file:///c:/...`。统一标准适配器结果出口后 GREEN，定义、两处引用和重命名保持原始 URI；重命名
  转为带实际 wire version 12 的 text-only `documentChanges`，项目磁盘未改。
- 同文件独立未知适配器通过真实受管 worker 提交不合法编辑，过期 version、重复目标、resource
  operation、annotation、工作区外路径均整包拒绝，随后同连接合法请求仍成功。该夹具验证协议边界，
  不将预设编辑当作真实 TypeScript 类型分析证据。
- `NANOBUG_VUE_FIXTURES=C:/Projects/RustProjects/Vue-for-Nanobug` 下完整
  `node --test --test-concurrency=1 test/*.test.mjs` **35/35 通过，无跳过**，43.41 秒；
  `tsc -p jsconfig.json`、`cargo fmt --check` 与 `git diff --check` 通过。
- TS `dist/typescript-0.1.8.zip` 经公开宿主 `--plugin-package .` release 构建通过，12.86 秒；
  SHA-256 `6265d3db2a19798e803fb700ccf9b2d9abd6fe00cd355f1179252847787f8229`。
  .7 保留用于宿主单次 control lease 生命周期问题的独立复现；.8 不包含该宿主修复，整链仍待验收。
