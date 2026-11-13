# SwitchPlus 个人版

基于 CC Switch 4.0.28 的个人分支，保留现有供应商管理、路由、聚合、账号与工具功能，
集成 V9 历史过滤和重试，并支持可移植文字摘要压缩。

## 独立身份

| 项目             | SwitchPlus                      |
| ---------------- | ------------------------------- |
| 软件名           | SwitchPlus                      |
| 应用标识         | `io.github.saber570.switchplus` |
| Windows 程序名   | `switchplus.exe`                |
| macOS 应用       | `SwitchPlus.app`                |
| 默认数据目录     | `~/.switchplus`                 |
| 默认代理端口     | `15731`                         |
| 系统注册链接协议 | `switchplus://`                 |
| 应用更新         | 手动合并源码、重新构建安装      |

Tauri 的窗口状态、WebView 存储、路径设置及单实例标识随新的应用标识隔离。
数据库、设备设置、登录副本、日志、Skills 和恢复备份写在新的应用目录；
数据库默认位于 `~/.switchplus/cc-switch.db`，沿用已安装版本的数据；若仅存在远程分支版本创建的 `switchplus.db`，则继续使用该文件。不会读取原版 `~/.cc-switch` 的数据库或设置。
Rust 库名仍为 `cc_switch_lib`，客户端路由标识使用独立的 `switchplus`。
版本号从 1.0.0 开始独立递增，作者为 chow，关于页面显示“个人自用版”。安装包 publisher 保持 saber570，以便沿用已安装版本的注册表路径。此前的 4.0.28/4.0.29 视为早期个人版本，不与上游版本同步编号。

## 与原版共存与配置导入

安装和应用数据相互独立，首次启动不自动搬走或修改 `~/.cc-switch` 数据。
需要原有供应商时，在原版导出配置，再在 SwitchPlus 的导入功能中导入，或手动添加。
完整数据库导入会带入其中的代理端口等设置，导入后请检查端口。
不要把 SwitchPlus 自定义数据目录指向 `~/.cc-switch`。

两款软件管理的外部工具仍是同一套：Codex、Claude 等客户端配置和系统登录状态
不会因为改名而另建一份。接管同一个客户端前先退出原版和外置过滤进程，
让它们恢复配置，再由 SwitchPlus 接管。不要让两款软件同时修改同一份客户端配置。
SwitchPlus 只解析和注册 `switchplus://`，拒绝原版的 `ccswitch://`。
Windows 上检测到 CC Switch 正在运行时拒绝启动；原版目录及指向它的目录链接也被拒绝访问。

## 覆盖安装已有 SwitchPlus

1.0.0 保持原应用标识、程序名、当前用户安装范围和数据目录，安装器识别 4.0.28/4.0.29 并使用
原安装路径。安装前从托盘退出 SwitchPlus，结束当前请求后再运行新版安装包。
安装器的维护页面选择 `Do not uninstall`（保留原安装并覆盖），无需手动卸载或重新导入数据库。
供应商、代理端口、路由/聚合设置、用量和 `live-state.json` 沿用原数据；旧 V9 关闭值自动忽略。
新版安装器禁用覆盖和静默降级。已发出的旧安装器不受这项限制，不要用它覆盖新版。
数据库正常迁移和迁移前备份仍保留。

新版首次启动若没有本机接管记录、旧备份或已启用路由，会保留已有客户端配置；
已有路由安装按保存的模式恢复。不要在请求运行中安装，安装器关闭旧进程会终止进行中的请求。
已有安装继续使用保存的端口；15731 只作为新数据库的默认端口。合并后的源码需要重新构建安装包，尚未进行覆盖安装实测。

旧安装的第三方会话从 `custom` 迁到独立的 `switchplus` 标识前会备份；只有本机旧迁移记录和接管记录都存在时才执行。旧压缩摘要继续可读。

## 本地构建

两台电脑先切换到 `switchplus-personal` 的同一提交，并确认没有额外本地修改。
需要 Node.js 20.19+ 或 22.12+、Corepack、pnpm 10.12.3、`rust-toolchain.toml` 固定的 Rust 1.95。
若 Node 安装包没有附带 Corepack，先运行 `npm install --global corepack`。
Windows 还需要 Visual Studio C++ 桌面构建工具和 Windows SDK；
macOS 需要 Xcode Command Line Tools。

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm build:check
corepack pnpm build
```

统一入口会校验应用标识、版本和图标，使用锁定依赖构建，并在 `bundle/switchplus-build-info.json`
记录源码提交、未提交状态、图标和产物 SHA256。换电脑时对比这些字段，不再靠版本号判断是不是同一份代码。
Windows 默认只生成 EXE 安装包，产物在 `src-tauri/target/<target>/release/bundle/nsis/`，新安装默认路径为
`%LOCALAPPDATA%\SwitchPlus`，已安装时沿用保存的路径。Mac 生成 `bundle/macos/SwitchPlus.app` 和 `bundle/dmg/` 下的 DMG。
Mac 如需指定芯片，可使用 `corepack pnpm build --target aarch64-apple-darwin` 或 `--target x86_64-apple-darwin`。
入口始终明确指定目标架构：优先命令行 `--target`，其次 `CARGO_BUILD_TARGET`，否则使用本机 Rust 工具链的架构。
64 位 Intel/AMD Windows 的 `<target>` 为 `x86_64-pc-windows-msvc`。自定义 Cargo 输出目录时以命令末尾打印的实际路径为准。
旧命令 `pnpm tauri build --config src-tauri/tauri.local-build.conf.json` 保留兼容，配置与默认构建一致。
显式构建 MSI 时新安装路径也保持 `%LOCALAPPDATA%\SwitchPlus`；已有 MSI 仍沿用其注册的路径。
此前采用 `com.switchplus.desktop` / publisher `chow` 打出的包属于另一套安装身份，不能保证自动覆盖识别；本仓库统一沿用最早已安装版本的身份。
无需更新签名密钥，个人版已移除应用更新界面、检查、下载命令和更新插件。
CLI 工具自身的安装更新功能仍然保留。这里提供源码；未完成 Windows/macOS 安装实测。

原版 GitHub 发布流程是上游的基础设施，个人版以本地构建为准；
若以后启用自动发布，需要另配 SwitchPlus 的产物名称与签名。
同步上游参考 [维护说明](upstream-sync.md)，过滤行为参考 [V9 说明](codex-v9-filter.md)。
安装包包含上游 `LICENSE` 和 `THIRD_PARTY_NOTICES.md`；关于页面使用中性的开源许可说明，原作者署名保留在许可文件中。
