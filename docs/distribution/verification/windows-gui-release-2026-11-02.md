# Windows 内测包 GUI 启动修复（2026-11-02）

用户反馈正式内测包附带命令行窗口，并明确要求修复后替换现有 Release。继续使用已确认的
当前 main；本次附件替换是用户对默认不可覆盖发行规则的明确例外。

## 原因与修复

直接检查已发布便携 ZIP 中的 `Nanobug.exe`：PE Subsystem 为 3（Windows Console），
而桌面发行应为 2（Windows GUI）。安装器快捷方式直接指向 `Nanobug.exe`，没有 Shell
包装。旧程序 SHA-256 为 `e6e6323a4d0f25ce19ce77cb67acb2dab46dcdfdfdaf943ef9ccbfb7abcc8551`。

- Windows 非测试 Release 显式链接 GUI 子系统，Debug 与测试运行器保留控制台。
- 启动时仅尝试复用父进程已有终端，供公开 CLI 使用；不分配新控制台。Explorer 和安装器
  没有父控制台时，附加失败是正常情况，不作为启动错误。
- 保留调用方传入的重定向句柄；使用实际发行程序验证 JSON stdout、错误 stderr 和退出码。
- 静态 CRT 配置保持，最终导入表没有外部 VC++ 运行库；公开 SDK 和插件协议没有变动。

依据：[Rust Windows 子系统](https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute)、
[Windows 标准句柄及附加语义](https://learn.microsoft.com/en-us/windows/console/getstdhandle)。

## 实际验证

`crates/editor-app/tests/windows_distribution.rs` 只接受显式指定的发行 EXE。先用已发布程序
执行：2 passed、1 failed，失败项精确报告 Console 3 与 GUI 2 的差异；修复后 3 passed、
0 failed。该检查不读取测试运行器自身的 PE，CLI 检查也实际运行同一发行 EXE。

```powershell
$env:NANOBUG_WINDOWS_DISTRIBUTION_EXE = (Resolve-Path target/release/editor-app.exe).Path
$env:NANOBUG_WINDOWS_SMOKE_PACKAGE = (Resolve-Path dist/toml/toml-0.2.4.zip).Path
cargo test -p editor-app --test windows_distribution --locked --jobs 1 -- --ignored --nocapture --test-threads=1
```

`cargo fmt --check`、`cargo test --workspace --exclude editor-app --locked --jobs 1` 和
`cargo check --workspace --locked --jobs 1` 通过；工作区 877 passed、0 failed、446 ignored，
未执行的 ignored 项不计为通过。

从指向最终 GUI EXE 的 Windows Shell 快捷方式启动独立 profile，实际观察到原生 Nanobug
首页、项目入口和设置控件，没有附带命令行窗口；正常关闭测试实例。安装器和便携包重新
生成后，继续核对其实际载荷、PE、SDK、摘要及安装生命周期。

受检 GUI EXE 为 94,522,368 字节，SHA-256 为
`132e7cddf52c34fc5d8f1462df59ad0514965d2ba83773bf3ef28d2b1c6e4407`。

## 本次替换身份

保留 `v0.1.0-internal.20261102.2` 的下载入口和内部测试标记，数字程序版本仍为 `0.1.0`。
替换前五个附件的完整字节、摘要与原 Release 元数据已备份到项目外任务目录；发布时将
安装器、便携 ZIP、两份 SHA-256 和构建信息一起对齐到修复提交，并核对实际标签来源。
没有改写 main 的已有提交；不修改各插件既有 Release。

最终来源与附件以
[现有内部测试版 Release](https://github.com/Nanobug-Editor/Editor/releases/tag/v0.1.0-internal.20261102.2)
和项目外 `C:/Users/Tmiracle/.codex/tmp/nanobug-gui-release-replacement-2026-11-02/` 的实际
读回记录为准。安装包仍未签名；本次不宣称完整 UI 或实体 IME 矩阵重验。

返回[安装与分发](../README.md)。
