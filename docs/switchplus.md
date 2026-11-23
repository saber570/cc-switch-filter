# SwitchPlus 个人版

完整同步 CC Switch 上游并保留定制的个人分支，保留现有供应商管理、路由、聚合、账号与工具功能，
集成 V9 历史过滤和重试，并支持可移植文字摘要压缩。

源码分支于 2026-11-23 完整整合上游 4.0.40（`721ca8e0`），发布版本保持 1.0.2，数据库 schema 保持 27。
本轮已完成 Mac ARM64 Release 编译，源码提交 `c1b41349`，未生成 bundle 或安装包，
未安装、未推送；验证结果与产物校验值见 [2026-11-23 同步记录](upstream-sync-20261123.md)。
2026-11-18 生成的 Mac 1.0.2 安装包基于上游 4.0.35（`ad22d45`），属于历史产物，
不能据此判断本轮源码的编译或安装验证结果。
Codex 路由遇到 `gateway_queue_full` 时继续按限流策略重试。
后续遵循 [完整合并维护规则](upstream-sync.md) 和根目录 [AGENTS.md](../AGENTS.md)。
只有明确要求升级版本号并打包时才变更发布版本。

## 独立身份

| 项目             | SwitchPlus                 |
| ---------------- | -------------------------- |
| 软件名           | SwitchPlus                 |
| 应用标识         | `com.chow.switchplus`      |
| Windows 程序名   | `switchplus.exe`           |
| macOS 应用       | `SwitchPlus.app`           |
| 默认数据目录     | `~/.switchplus`            |
| 默认代理端口     | `15731`                    |
| 系统注册链接协议 | `switchplus://`            |
| 应用更新         | 手动合并源码、重新构建安装 |

Tauri 的窗口状态、WebView 存储、路径设置及单实例标识随新的应用标识隔离。
数据库、设备设置、登录副本、日志、Skills 和恢复备份写在新的应用目录；
数据库默认位于 `~/.switchplus/cc-switch.db`，沿用已安装版本的数据；若仅存在远程分支版本创建的 `switchplus.db`，则继续使用该文件。不会读取原版 `~/.cc-switch` 的数据库或设置。
Rust 库名仍为 `cc_switch_lib`，客户端路由标识使用独立的 `switchplus`。
版本号从 1.0.0 开始独立递增，当前版本为 1.0.2。作者和安装包 publisher 统一为 `chow`，关于页面显示“个人自用版”。2026-11-20 按用户确认的卸载重装方案，将应用标识改为 `com.chow.switchplus`，NSIS/MSI 安装注册表改为 `Software\chow\SwitchPlus`；MSI UpgradeCode 固定为 `e2d7a916-1ba3-4b5c-99c7-4a0606ba07b0`。仓库链接保持原值，`.switchplus` 主数据目录继续沿用。2026-11-20 的重新打包没有升级版本，同为 1.0.2 的旧包应通过构建时间、应用标识和 SHA256 区分。

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

## 从旧身份切换到 chow（2026-11-20）

用户选择先取得新包，再自行关闭并卸载旧版；不开发旧系统存储的自动迁移。
`io.github.saber570.switchplus` 和 `com.switchplus.desktop` 属于旧身份，不能直接套用后面的历史覆盖安装步骤。
新 NSIS 安装器遇到旧安装记录且缺少 `com.chow.switchplus` 标记时，会停止并提示先卸载，静默模式返回退出码 2。

1. 结束当前请求，在旧版关闭路由接管、切回直连，导出 SQL。
2. 完全退出旧版，备份 `~/.switchplus`；使用自定义数据目录时也备份该目录并记录路径。
3. 卸载旧版，保留数据，再安装新包。Windows 默认位置仍为 `%LOCALAPPDATA%\SwitchPlus`。
4. 若新版已经读取原数据，无需再导入；若为空，则先设置自定义数据目录或导入 SQL，再按需恢复本机设置。
5. 核对供应商、模型、端口、聚合选项和登录状态，之后重新开启路由。

SQL 包含数据库中的配置和记录；不包含 `settings.json`、Skills 文件、独立账号文件和 `live-state.json` 等本机文件。
Tauri 旧身份下的窗口/WebView 状态和 `app_paths.json` 自定义目录指针不自动复制，因此主数据仍在也可能需要重新选择目录。
Mac 同样改用新标识，先退出旧应用再替换，主数据目录保持不变；2026-11-20 只生成 Windows EXE，没有生成 Mac 包。
后续 `com.chow.switchplus` 安装会写入身份标记并保留安装路径，可在同一身份下继续覆盖升级。

## 历史：旧身份版本之间的覆盖安装

Windows：1.0.1 保持原应用标识、程序名、当前用户安装范围和数据目录，安装器识别 4.0.28/4.0.29 并使用
原安装路径。安装前从托盘退出 SwitchPlus，结束当前请求后再运行新版安装包。
安装器的维护页面选择 `Do not uninstall`（保留原安装并覆盖），无需手动卸载或重新导入数据库。
供应商、代理端口、路由/聚合设置、用量和 `live-state.json` 沿用原数据；旧 V9 关闭值自动忽略。
新版安装器禁用覆盖和静默降级。已发出的旧安装器不受这项限制，不要用它覆盖新版。
数据库正常迁移和迁移前备份仍保留。

macOS：结束当前请求并完全退出 SwitchPlus，打开新版 DMG，将 `SwitchPlus.app` 拖到原来的安装位置并选择替换
（通常为 `/Applications/SwitchPlus.app`）。1.0.2 沿用原应用标识和 `~/.switchplus` 数据目录，
替换应用不会删除该目录中的供应商、路由和用量数据，无需先卸载或重新导入。已有自定义数据目录也继续沿用。
Finder 可能因旧版编号为 4.x 而提示版本较旧，核对应用名和源码提交后仍可替换；Windows 安装器的维护选项不适用于 Mac。

新版首次启动若没有本机接管记录、旧备份或已启用路由，会保留已有客户端配置；
已有路由安装按保存的模式恢复。不要在请求运行中安装，安装器关闭旧进程会终止进行中的请求。
已有安装继续使用保存的端口；15731 只作为新数据库的默认端口。已在 Apple 芯片 Mac 上验证从 DMG 覆盖临时目录中的 1.0.1 应用副本，并检查 1.0.2 的签名、架构和应用标识。旧数据库迁移和保存的路由恢复使用隔离数据测试。

旧安装的第三方会话从 `custom` 迁到独立的 `switchplus` 标识前会备份；只有本机旧迁移记录和接管记录都存在时才执行。旧压缩摘要继续可读。

## 本地构建

两台电脑先切换到 `switchplus-personal` 的同一提交，并确认没有额外本地修改。
需要 Node.js 20.19+ 或 22.12+、Corepack、pnpm 10.12.3、`rust-toolchain.toml` 固定的 Rust 1.95。
若 Node 安装包没有附带 Corepack，先运行 `npm install --global corepack`。
Windows 还需要 Visual Studio C++ 桌面构建工具和 Windows SDK；
macOS 需要 Xcode Command Line Tools（首次使用运行 `xcode-select --install`），应用最低支持 macOS 12.0。

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm build:check
corepack pnpm build
```

统一入口会校验应用标识、版本和图标，使用锁定依赖构建，并在 `bundle/switchplus-build-info.json`
记录源码提交、未提交状态、图标和产物 SHA256。换电脑时对比这些字段，不再靠版本号判断是不是同一份代码。
Windows 默认只生成 EXE 安装包，产物在 `src-tauri/target/<target>/release/bundle/nsis/`，新安装默认路径为
`%LOCALAPPDATA%\SwitchPlus`，已安装时沿用保存的路径。Mac 生成 `bundle/macos/SwitchPlus.app` 和 `bundle/dmg/` 下的 DMG。
Mac 默认生成当前 Rust 工具链架构的包，不会自动生成 Universal 通用包。
Apple 芯片使用 `corepack pnpm build --target aarch64-apple-darwin`，Intel 使用 `corepack pnpm build --target x86_64-apple-darwin`。
指定另一种架构前，先用 `rustup target add aarch64-apple-darwin` 或 `rustup target add x86_64-apple-darwin` 安装对应目标。
两种架构均需在 Mac 上构建。APP、Dock 和 DMG 使用同套 SwitchPlus 图标，菜单栏使用单色图标适配深浅色。
本地入口使用 ad hoc 签名，在生成 DMG 前验证应用签名。个人构建未经 Apple 公证，无需开发者证书。README 中保留的上游签名/公证说明不适用于这些个人构建产物。
`corepack pnpm build:check` 只校验配置，不执行编译或打包。
入口始终明确指定目标架构：优先命令行 `--target`，其次 `CARGO_BUILD_TARGET`，否则使用本机 Rust 工具链的架构。
64 位 Intel/AMD Windows 的 `<target>` 为 `x86_64-pc-windows-msvc`。自定义 Cargo 输出目录时以命令末尾打印的实际路径为准。
旧命令 `pnpm tauri build --config src-tauri/tauri.local-build.conf.json` 保留兼容，配置与默认构建一致。
显式构建 MSI 时新安装路径也保持 `%LOCALAPPDATA%\SwitchPlus`；同一新身份的 MSI 沿用已注册路径。
此前采用 `com.switchplus.desktop` 或 `io.github.saber570.switchplus` 打出的包需按前述卸载重装流程切换，不能仅依据版本号判断是否可覆盖。
无需更新签名密钥，个人版已移除应用更新界面、检查、下载命令和更新插件。
CLI 工具自身的安装更新功能仍然保留。本轮未验证 Windows 安装。

### Windows / WSL 构建位置约定（2026-11-20）

Windows 包优先使用 Windows 原生构建：源码、Node/pnpm、Rust/MSVC 和构建输出都留在 Windows，
使用上面的统一构建入口。需要安装 Microsoft C++ Build Tools 的“使用 C++ 的桌面开发”工作负载，
包含 MSVC x64/x86 工具和 Windows SDK；无需为了构建安装完整 Visual Studio IDE。
本机已于 2026-11-20 安装 Visual Studio 2022 Build Tools 17.14.45、MSVC v143（14.44）、
Windows SDK 10.0.26100.0，以及工作负载配套的 CMake 3.31.6-msvc6 和 Ninja 1.12.1。
安装路径为 `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools`，安装器成功退出，
无需重启系统。沿用 Windows Rust 1.95.0，已将 `%USERPROFILE%\.cargo\bin` 加入用户 PATH；
安装前打开的终端或编辑器需要重新打开才能继承新的 PATH。

已在 Windows 原生环境验证 C 程序的 SDK 头文件、编译、链接和运行，以及 Rust x64 程序的编译、链接和运行。
SwitchPlus 的 `cargo check --manifest-path src-tauri/Cargo.toml --locked --offline --target x86_64-pc-windows-msvc --lib --bins`
和 `corepack pnpm build:check` 均通过。验证使用 Visual Studio 的 x64 Developer PowerShell 环境。
随后按用户要求，已通过统一入口 `corepack pnpm build --target x86_64-pc-windows-msvc`
完成 Windows 原生发布编译、链接和 NSIS 打包，版本保持 1.0.2；整个构建过程没有调用 WSL。
前端类型检查、5 项构建/身份测试、11 项 NSIS 版本迁移/降级判断测试通过。
解包校验确认 x64 架构、版本、图标、许可证及旧安装身份/路径恢复规则；
安装包内的程序与原生编译产物仅有 Tauri 写入的 NSIS 包类型标记差异（3 字节），其余字节一致。
安装包已放入 `release/windows/SwitchPlus_1.0.2_x64-setup.exe`，对应构建记录及 SHA256 在同目录。
尚未在本机实际执行覆盖安装或启动新程序验证。

此前同日的 1.0.2 安装包采用 Windows 构建前端、WSL cargo-xwin 编译 Windows 可执行文件、
Windows Tauri/NSIS 封装的方式。Cargo 缓存位于 `/home/chow/switchplus-win-target`，
但源码仍从 `/mnt/c/work/workspace/cc-switch-filter` 读取，因此触发 WSL 跨文件系统性能提示。
这条历史流程已经完成发布编译和安装包静态校验，但后续不再沿用从 `/mnt/c` 编译的目录安排。

需要沿用 WSL 时，先把准确版本的源码检出或复制到 `/home/chow/...` 的独立构建目录，
前端依赖、前端产物、Cargo 缓存和编译输出全部使用 Linux 文件系统。保留并核对源码提交，
如果包含未提交改动，要同步这些改动并记录差异，不能只复制 HEAD 后仍声称构建了当前工作区。
Windows 和 Linux 的 `node_modules` 分别安装；跨系统访问仅用于传输源码快照和最终产物。
必要时可将完成的 Windows 可执行文件及同版本打包资源复制回 Windows，再用 NSIS 封装。

Tauri 支持通过 cargo-xwin 在 Linux 交叉构建 Windows NSIS 安装包，但本项目尚未验证全程 WSL 打包。
现有 `scripts/build-switchplus.mjs` 按构建主机选择平台配置，并拒绝目标系统与主机不同，
所以不能直接在 WSL 运行普通 `pnpm build` 来生成 Windows 包。
若后续实现全程 WSL 构建，需要单独适配交叉编译入口、Windows 平台配置和 NSIS 模板路径，
继续校验图标、安装身份、覆盖安装规则和构建记录。

原版 GitHub 发布流程是上游的基础设施，个人版以本地构建为准；
若以后启用自动发布，需要另配 SwitchPlus 的产物名称与签名。
同步上游参考 [维护说明](upstream-sync.md)，过滤行为参考 [V9 说明](codex-v9-filter.md)。
安装包包含上游 `LICENSE` 和 `THIRD_PARTY_NOTICES.md`；关于页面使用中性的开源许可说明，原作者署名保留在许可文件中。
