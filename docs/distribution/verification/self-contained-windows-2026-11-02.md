# Windows 静态运行库发行验收（2026-11-02）

按用户要求，Windows 安装后的 Nanobug 无需另外安装 Visual C++ 运行库。范围为 Windows
x64 宿主、安装器及插件随包附带的 Windows 辅助程序；Windows 自带的系统 DLL 继续由系统
提供，语言工具沿用各插件已有的授权准备流程。

## 宿主构建与安装

- `.cargo/config.toml` 对 Windows MSVC 目标启用 `+crt-static`，不影响 WASI 插件目标。
- `cargo build -p editor-app --release --locked` 通过；`dumpbin /dependents` 确认最终 EXE
  不导入 VCRUNTIME、MSVCP、CONCRT 或 UCRTBASE 可再发行运行库。
- 受检 EXE 为 94,522,368 字节，SHA-256 为
  `e6e6323a4d0f25ce19ce77cb67acb2dab46dcdfdfdaf943ef9ccbfb7abcc8551`。
- `cargo fmt --check`、`cargo test --workspace --exclude editor-app --locked --jobs 1`
  和 `cargo check --workspace --locked --jobs 1` 通过。测试共 877 passed、0 failed、
  446 ignored；ignored 项未执行，不计为通过。
- 首轮默认并行验证遇到页面文件不足、mmap 失败及编译器内存异常。限制构建并发后，在没有
  修改产品 Rust 源码的情况下完成终端定向编译和上述验证；不将资源错误归因于缺失接口。
- 从官方 Inno Setup 6.7.4 签名包验证摘要和签名后，在项目外任务目录解压使用编译器。
  没有安装系统级工具或修改 PATH；简体中文翻译来自同一官方源码标签。
- 安装器成功编译。测试前确认没有既有 Nanobug 安装或资源管理器菜单，在独立目录执行
  按用户安装；安装后 EXE 与受检载荷摘要一致，真实安装 EXE 的 SDK 导出成功。
- 隔离 profile 以受限模式打开测试文档，实际验证中文文本输入、保存、Undo、Redo、正常
  退出及卸载。卸载后文档保留；没有操作真实用户项目或其已安装插件。
- 实体中文 IME 候选窗、macOS/Linux 和全部产品交互矩阵未在本次重验。

## 发行身份与复验

新的宿主内部测试版标签为 `v0.1.0-internal.20261102.2`，数字程序版本仍为 `0.1.0`。
安装器通过 `BuildLabel` 显示构建标记，不改变稳定 AppId、用户数据路径或权限。
此前 `v0.1.0-internal.20261102` 的附件保持原始字节，新构建使用新的标签和产物名称。

最终安装器、便携 ZIP、构建信息和完整 SHA-256 以
[GitHub Release](https://github.com/Nanobug-Editor/Editor/releases/tag/v0.1.0-internal.20261102.2)
的实际附件为准。详细命令、日志及安装生命周期证据保存在项目外
`C:/Users/Tmiracle/.codex/tmp/nanobug-self-contained-release-2026-11-02/`。

插件分别通过同一宿主的公开 `--plugin-package` 和 `--plugin-check` 构建、准入，按清单
ID 输出到主项目 `dist/<id>/`。四个原生辅助程序分别检查导入表；插件实际包回归和各仓
Release 交付另行记录，不以静态准入代替业务验收。

## 插件兼容验证

本次候选版本如下，均通过当前宿主的公开打包与包检查入口；安装 ID、权限和能力声明
保持原有内容。所有访客测试合计 360 passed、0 failed；Java、Maven、XML、Vue 的访客
单元测试为零，不据此宣称业务通过。

| 插件 | 版本 |
| --- | --- |
| Git | 0.1.53 |
| Java | 0.1.35 |
| Maven | 0.1.19 |
| Rust | 0.6.4 |
| HTML | 0.3.2 |
| JavaScript | 0.3.2 |
| TOML | 0.2.4 |
| XML | 0.2.3 |
| Image（ID `svg`） | 0.6.3 |
| Markdown | 0.18.7 |
| TypeScript | 0.1.18 |
| Vue | 0.1.13 |

实际最终 ZIP 经 Package / Manager 验证了 Git 安装与卸载、Java 语义诊断及进程退役、
Maven 离线项目导入、Image 文件资源权限、Markdown 增量预览与隐藏恢复，以及 XML
独立结构与精确源码范围，六个场景均通过。

Markdown 和 XML 的两项历史 ignored 测试最初失败；使用旧固定包也复现相同失败。
原因是测试仍假定安装时立即激活，未经过当前公开的按需激活入口。本次复用既有
`support/demand.rs`，分别请求已声明的预览视图和结构角色，保留原有行为断言并复验
通过；没有为插件新增宿主分支或放宽协议校验。

各归档完整摘要、实际运行日志、原生辅助程序最终导入表和后续 GitHub 交付身份保存在
上述项目外证据目录。本次不自动更新市场 registry 或批准收录；其余 ignored 场景不计为
通过。

返回[安装与分发](../README.md)。
