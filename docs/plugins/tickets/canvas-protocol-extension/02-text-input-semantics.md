# 02：可编辑目标与语义

**来源：**[Canvas 扩展方案](../../specs/canvas-protocol-extension.md)。稳定标识：`canvas-protocol-extension/02-text-input-semantics`。

**What to build：**普通表格与流程图插件在稳定目标中编辑文字，通过完整键盘/指针、原生输入法和语义树实现可操作的二维界面。真实编辑器文档继续复用唯一文档会话。

**Blocked by：**[01 / #128 — 场景图形与空间操作](https://github.com/Nanobug-Editor/Editor/issues/128)。

**Status：**已验收并推送独立分支，真实议题 [#129](https://github.com/Nanobug-Editor/Editor/issues/129)，已读回 closed/completed，前置 #128 已关闭。仅在独立 worktree 的 `codex/canvas-protocol-extension` 分支实现，四单完成后统一合回 main；[本单记录](../../verification/canvas-editable-targets-2026-10-25.md)。

**Why separate：**依赖 01 的场景身份和坐标；文字塑形、目标输入和辅助功能共同构成一套可编辑目标，必须一并验收才能作为 03 的稳定接缝。

## Acceptance criteria

- [x] AC1：多行段落的塑形、换行、对齐和溢出使用宿主原生字形结果；位置查询返回绑定场景与字体环境的 UTF-8 边界、行矩形、选区和最近文本位置。
- [x] AC2：中文、emoji、组合字符、双向文本、连字、软换行和省略后的原文偏移不会因字节/UTF-16/字素混淆产生非法范围。
- [x] AC3：目标级焦点、Tab 顺序、逻辑/物理键、按下/释放/重复、全部修饰键及独立文字提交可观察且有序；原生快捷键与 IME 不重复提交。
- [x] AC4：指针身份、进入/离开、捕获、光标及取消符合声明，离开画布仍向原目标完成手势；失焦、模态、撤销和退休清理所有权。
- [x] AC5：目标保持期间重绘、滚动和缩放保留中文预编辑与选择，候选窗定位正确；目标换代取消旧会话，迟到文字回执不能覆盖新文本。
- [x] AC6：真实编辑器文档继续使用原 DocumentSession 的文本、选择、IME、Undo/Redo，画布会话不建立第二份可变文档；插件领域文本和 Undo 留在插件。
- [x] AC7：稳定语义树提供名称、角色、值、状态、行列、焦点和动作，投影/裁剪与画面一致；原生控件语义不重复，虚拟内容有界且可导航。
- [x] AC8：实际表格和流程图包完成键盘、中文 IME 与 Windows 原生辅助功能验收，旧 Canvas 级事件与输入消费者保持可用。

## Verification

AC1/AC2/AC5 经公开布局/事件/版本接口验证固定文本样本的已知位置和替换结果，再用真实原生输入补足 OS 行为。AC3/AC4/AC7 通过原生焦点/指针和 AccessKit 可见语义动作验证，覆盖模态、移除、ABA 和队列顺序。AC6 观察公开文档变更、选择与 Undo/Redo 结果，不检查私有撤销栈实现。AC8 以实际 WASM 包进入公开管理器，并记录中英文、主题、DPI 与未实测平台。

本单执行仓库阶段门禁、相关 editor-app 定向/原生测试及实际包验证；只回归受影响的 01 图形、坐标、资源路径。04 再执行与原生混排和保留更新共同存在时的短组合验收。
