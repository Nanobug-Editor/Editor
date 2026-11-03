# 安装与分发

本目录记录维护者的安装布局、构建约束与实际验收证据。基本运行与打包入口维护在仓库根双语 README。

- [第一期安装包规格](specs/installer-phase1.md)：Windows 按用户安装与验收；macOS/Linux 兼容入口不验收。
- [GitHub 宿主更新入口与验收](verification/host-updates-2026-11-03.md)：应用菜单、关于与更新、下载校验、退出确认及安装/便携更新。
- [Windows 安装验收](verification/windows-installer.md)：实际产物、命令、安装生命周期和已知限制。
- [Windows 静态运行库发行验收](verification/self-contained-windows-2026-11-02.md)：无需单独安装 VC++ 运行库的构建、导入表、安装与编辑验证。
- [Windows 内测包 GUI 启动修复](verification/windows-gui-release-2026-11-02.md)：实际发行 PE、CLI 重定向、原生快捷方式启动及现有 Release 替换身份。

开发者直接使用 `cargo run` / `cargo run --release`；构建入口见仓库根 README。
当前打包直接调用工具，完整准备步骤见[原生安装器说明](../../installer/README.md)。
