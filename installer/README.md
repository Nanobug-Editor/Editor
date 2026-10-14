# 原生安装器与直接打包

2026-10-08 起，构建、插件归档与发行打包直接使用工具命令。仓库中的 `scripts/` 已移除；旧
PowerShell/Python 脚本只在本机 Codex 工作区存档，打包不得调用这些脚本或通过包装器间接调用。
本规则也适用于后续 CI 打包；不将本机 Codex 路径写入构建依赖。

## Windows

运行 `cargo build -p editor-app --release` 后，将 `target/release/editor-app.exe` 复制为
`dist/editor/Nanobug.exe`；复制品牌 ICO 到 `dist/editor/Nanobug.ico`，宿主依赖许可到
`dist/editor/licenses/`。安装器和便携包均不携带插件 ZIP、源码或首次提供索引；运行不依赖同级
`plugins/` 目录。官方插件由独立仓库发布，经市场审核后由用户主动安装到既有私有目录。
不要把编译器、宿主源码或脚本放入运行目录；旧的用户安装、设置与私有数据继续保留。

终端、上游 Alacritty 核心及默认主题编译进宿主，无独立终端 ZIP 或 WASM。旧终端安装记录仅用于有限数据导入，原始数据和备份保留。

内置终端的 Apache 许可证随编辑器发行，从已核对上游版本的资源直接复制：

```powershell
# 安装器收录整个 licenses 目录；不再从已退役的终端包提取许可证。
New-Item -ItemType Directory -Force -Path dist/editor/licenses | Out-Null
Copy-Item -LiteralPath crates/editor-app/assets/licenses/alacritty-terminal-LICENSE-APACHE -Destination dist/editor/licenses/
```

准备程序、图标和宿主许可后，从仓库根执行以下命令。重新使用旧载荷目录前先检查其文件清单，确保没有旧插件 ZIP 或 bundle-defaults.json；不要把用户的私有插件存储当作发行目录清理。

```powershell
# 版本来自 Cargo，不通过旧打包脚本读取；ISCC 接收准备好的运行目录。
$version = ((cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages |
    Where-Object name -eq 'editor-app').version
$payload = (Resolve-Path .\dist\editor).Path
$output = Join-Path $PWD 'dist/installers'
ISCC.exe "/DPayloadDir=$payload" "/DInstallerDir=$output" "/DAppVersion=$version" .\installer\windows\nanobug.iss
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }

# 校验文件直接来自最终产物。
$setup = Join-Path $output "Nanobug-Setup-$version-x64.exe"
$digest = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
"$digest  $([IO.Path]::GetFileName($setup))" | Set-Content -LiteralPath "$setup.sha256" -Encoding ascii
```

Inno Setup 需事先准备，未加入 PATH 时直接指定 ISCC.exe 完整路径。x64 是第一期验收架构；
检查原生 EXE 架构、载荷清单和最终安装器/便携 ZIP 摘要后再发行。安装配置保留按用户安装、运行中 mutex
保护、双语向导与用户数据保留行为，不包含外部程序或脚本执行项。

安装向导的资源管理器集成选项首次默认勾选，同时添加文件夹对象和目录空白处的“使用 Nanobug 打开”
（英文为 “Open with Nanobug”）。升级保留该选项；明确取消时撤销本产品原有入口，卸载核对归属后只移除
本产品写入的值。Windows 11 的静态入口可能显示在“显示更多选项”中，不修改系统菜单样式。
菜单将可执行文件和目录分别引用，目录末尾的 `\.` 避免盘符根目录的反斜杠影响参数引号；应用按实际目录路由。
便携运行不会注册这些菜单。

普通 GUI 的再次启动按用户 profile 将请求交给已有实例：同项目激活原窗口，不同项目新开窗口，无参数进入或
激活首页。目录或文件参数均可显式写在 `--` 后。`Nanobug.exe --profile "<独立目录>" -- "<目录或文件>"`
选择独立 GUI profile；它与 `ME_EDITOR_PROFILE_HOME` 保留相同隔离语义，不修改全局环境。工具 CLI 和
`--plugin-dev-instance` 先按既有流程分流，独立开发实例不被普通 GUI 接管。递交失败或应用正在关闭时明确提示重试，
不删除存储锁或启动第二个冲突的数据所有者。

## 原生语言服务资源

HTML、JavaScript 的 `native/server.cjs` 是已构建的分发资源；共享项目描述将它和
`licenses/native/` 一起归档。常规打包不安装 npm 依赖。只有修改 `service/` 源码或锁文件时，
才在已准备 Node/npm 的环境中直接重建。以下从仓库根运行，对两个项目分别执行：

```powershell
# 锁定依赖在忽略的构建目录安装，不调用仓库或存档脚本。
$packageName = 'html' # 另一个项目为 javascript。
$project = (Resolve-Path '../HTML').Path # JavaScript 使用 ../JavaScript。
$buildDir = Join-Path $PWD "target/language-services/$packageName"
New-Item -ItemType Directory -Path $buildDir -Force | Out-Null
Copy-Item -LiteralPath "$project/service/package.json", "$project/service/package-lock.json", "$project/service/server.cjs" -Destination $buildDir
Push-Location $buildDir
try {
    npm ci --ignore-scripts --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw 'Language service dependency preparation failed' }
    # esbuild 的公开 CLI 直接生成单文件原生服务资源。
    node node_modules/esbuild/bin/esbuild server.cjs --bundle --platform=node --format=cjs --target=node24 --legal-comments=inline "--outfile=$project/native/server.cjs"
    if ($LASTEXITCODE -ne 0) { throw 'Language service build failed' }
} finally { Pop-Location }
Get-FileHash -LiteralPath "$project/native/server.cjs" -Algorithm SHA256
```

重建后更新 `manifest.json` 中服务资源的 SHA-256；核对锁定依赖的许可、NOTICE 和
TypeScript 的第三方 Unicode 通知，保留 HTML 的固定 `@vscode/l10n` MIT notice。
依赖变更时同步维护 `licenses/native/` 并提升插件版本。然后直接调用 `--plugin-package`；
宿主完整校验失败时不替换既有 ZIP。XML 的 WASM 组件由共享描述通过内嵌 SDK 构建，
LemMinX 按清单在获批的插件私有版本目录准备，不打入 ZIP。

## macOS/Linux（本期未验收）

这些系统必须先准备对应架构的原生 Nanobug。旧 Python 包装器不作为
当前入口；用以下原生工具直接打包，不能用 Windows EXE 替代 Mach-O/ELF。

- macOS：准备 `Nanobug.app/Contents/MacOS/Nanobug`、`Contents/Resources/licenses`；
  将品牌 ICNS 复制到 Resources。Info.plist 保留产品名称、版本、bundle ID
  `io.github.t-miracle.nanobug`、CFBundleExecutable Nanobug 和实际最低系统版本。
  DMG 的暂存目录包含 `.app` 和指向 `/Applications` 的链接。
- Linux：程序和相邻 licenses 放到 `/usr/lib/nanobug`，`/usr/bin/nanobug` 链接到
  `../lib/nanobug/nanobug`；补充 `.desktop` 与 256px 品牌 PNG。DEB 的 control 填写实际
  version/architecture/maintainer，使用 dpkg-shlibdeps 计算依赖；RPM 用实际原生 Requires。

```bash
# macOS：直接调用系统工具；暂存 .app 和 Applications 链接需事先准备。
hdiutil create -volname Nanobug -srcfolder dist/macos-stage -format UDZO dist/installers/Nanobug.dmg
pkgbuild --component dist/macos-stage/Nanobug.app --install-location /Applications dist/installers/Nanobug.pkg

# Debian 系：准备完整包根及 DEBIAN/control 后直接打包。
dpkg-deb --build --root-owner-group dist/deb-root dist/installers/Nanobug.deb

# RPM 系：准备原生 spec 与 SOURCES 后直接打包。
rpmbuild -bb nanobug.spec
```

macOS 签名、公证、平台依赖和最低系统版本另行准备；本期继续不执行 macOS/Linux 构建或
安装验证。第一期 Windows 验收记录保留当时命令，不能用历史脚本命令替代当前流程。
