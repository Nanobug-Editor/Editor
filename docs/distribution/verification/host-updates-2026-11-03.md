# GitHub 宿主更新入口与验收

日期：2026-11-03。宿主更新实现、自动回归及原生入口验收通过；最终集成、安装器和 Release 状态见[统一发行验收](release-all-2026-11-03.md)，包含尚未解决的 UIA 并发关闭异常。

## 用户行为

- 空窗口与项目窗口的左上角应用菜单均提供更新入口，设置中的“关于与更新”显示完整发行版本、状态及发行说明。
- 版本和附件来自 `Nanobug-Editor/Editor` 的 GitHub Releases。稳定通道忽略预发布；内测发行默认接收预发布，用户可在个人设置中调整。
- 启动后及每六小时自动检查。下载与“重启并更新”均需用户主动操作；下载完成保存已校验包，取消退出时保留文档、窗口和下载结果。
- Windows x64 安装版调用发行安装器，便携版仅替换程序、图标、构建信息和许可。便携更新失败恢复已替换文件；安装目录中的插件、配置和历史不在替换列表。
- 安装助手等全部窗口完成退出确认及清理后启动，复核附件摘要，并等待原进程和其他实例正常退出。更新期间阻止新的 GUI 进入。开发构建和其他平台提供发行页入口。

更新状态、偏好、下载及安装结果保存在所选 profile 的 `updates/` 下，默认遵守[安装根目录存储](../../project/portable-data.md)。不读取工作区配置或 Git 凭据，不自动安装项目工具链。

## 实际验证

使用任务工作区自身 `target/`；主工作区的输出没有与任务工作区共享。日志和 PNG 位于 `C:/Users/Tmiracle/.codex/task-artifacts/nanobug-release-20261103/`。

| 检查 | 结果 |
| --- | --- |
| `cargo test -p editor-app --bin editor-app host_update --locked --jobs 1 -- --test-threads=1` | 11 passed、2 ignored；忽略项另行执行 |
| `host_update_native_navigation_and_cancelled_restart --ignored --nocapture --test-threads=1` | 1 passed；真实 Windows/D3D 窗口、中英文、深浅主题、100%/150% 缩放、空窗口/项目菜单、取消未保存文档退出 |
| `host_update_github_release_download_smoke --ignored --nocapture --test-threads=1` | 1 passed；真实 GitHub 发现、下载、SHA-256、缓存恢复和便携 ZIP 解包替换 |
| `cargo fmt --check` | 通过 |

自动回归覆盖完整 SemVer、通道选择、草稿/非官方 URL/缺失摘要拒绝、重定向边界、传输取消与大小/摘要校验、缓存改动拒绝、退出取消、更新互斥以及真实文件替换失败回滚。截图已检查。

补全先前中断的测试时修复了缺失宏/trait 导入、原生夹具的主题初始化，以及在 GPUI 退出前清理测试拥有的 HWND；保留原始失败记录，没有跳过断言。上述网络测试不会安装或替换用户的日常程序。尚不将全部平台、实体 IME 候选窗和完整产品矩阵计为通过。

返回[安装与分发](../README.md)。
