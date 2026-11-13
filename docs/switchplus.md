# SwitchPlus 个人版

基于 CC Switch 4.0.28 的个人分支，保留现有供应商管理、路由、聚合、账号与工具功能，
集成 V9 历史过滤和重试，并支持可移植文字摘要压缩。

## 独立身份

| 项目             | SwitchPlus                      |
| ---------------- | ------------------------------- |
| 软件名           | SwitchPlus                      |
| 应用标识         | `com.switchplus.desktop` |
| Windows 程序名   | `SwitchPlus.exe`                |
| macOS 应用       | `SwitchPlus.app`                |
| 默认数据目录     | `~/.switchplus`                 |
| 默认代理端口     | `15731`                         |
| 系统注册链接协议 | `switchplus://`                 |
| 应用更新         | 手动合并源码、重新构建安装      |

Tauri 的窗口状态、WebView 存储、路径设置及单实例标识随新的应用标识隔离。
数据库、设备设置、登录副本、日志、Skills 和恢复备份写在新的应用目录；
数据库文件是 `~/.switchplus/switchplus.db`，不读取原版的数据库或设置。
Rust 内部 crate 名及摘要协议标记也保留，以减少后续合并上游时的冲突。
版本号为 4.0.29，作者和安装包发布者为 chow，关于页面显示“个人自用版”。

## 与原版共存与配置导入

安装和应用数据相互独立，首次启动不自动搬走或修改 `~/.cc-switch` 数据。
需要原有供应商时，在原版导出配置，再在 SwitchPlus 的导入功能中导入，或手动添加。
完整数据库导入会带入其中的代理端口等设置，导入后请检查端口。
不要把 SwitchPlus 自定义数据目录指向 `~/.cc-switch`。

两款软件管理的外部工具仍是同一套：Codex、Claude 等客户端配置和系统登录状态
不会因为改名而另建一份。接管同一个客户端前先退出原版和外置过滤进程，
让它们恢复配置，再由 SwitchPlus 接管。不要让两款软件同时修改同一份客户端配置。
SwitchPlus 只解析和注册 `switchplus://`，拒绝原版的 `ccswitch://`。
检测到 CC Switch 正在运行时拒绝启动；原版目录及指向它的目录链接也被拒绝访问。

## 本地构建

需要 Node.js 20.19+ 或 22.12+、pnpm 10.12.3、Rust 稳定版。
Windows 还需要 Visual Studio C++ 桌面构建工具和 Windows SDK；
macOS 需要 Xcode Command Line Tools。

```sh
pnpm install --frozen-lockfile
pnpm tauri build --config src-tauri/tauri.local-build.conf.json
```

Windows 只生成 EXE 安装包可添加 `--bundles nsis`，产物在
`src-tauri/target/release/bundle/nsis/`；Mac 产物在 `bundle/macos/` 和 `bundle/dmg/`。
无需更新签名密钥，个人版默认关闭官方更新源，前后端也不请求应用更新。
CLI 工具自身的安装更新功能仍然保留。Windows 已通过 Rust 发布编译检查、前端生产构建、145 项相关前端测试和 30 项相关 Rust 回归（24 项 V9/V12、3 项隔离、3 项路由切换与默认端口）。
此前 4.0.29 的 MSI/便携包已通过身份校验；本次合并后的源码需要重新构建安装包，尚未执行安装实测。

原版 GitHub 发布流程是上游的基础设施，个人版以本地构建为准；
若以后启用自动发布，需要另配 SwitchPlus 的产物名称与签名。
同步上游参考 [维护说明](upstream-sync.md)，过滤行为参考 [V9 说明](codex-v9-filter.md)。
安装包包含上游 `LICENSE` 和 `THIRD_PARTY_NOTICES.md`；关于页面保留来源署名。
