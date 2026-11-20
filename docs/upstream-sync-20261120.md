# 2026-11-20 完整上游同步记录

## 范围

- 同步前个人分支：`efca728130a27d9271622c4fa2c7de491c6af6ee`，已与 origin 对齐。
- 上次上游基线：`ad22d45a8d3cabee5a95ef3eff58ac5cd2ecbeb8`（4.0.35）。
- 本轮上游：`6e06607290b805ff2d0b92fa4e6bc69526f5e740`（4.0.36），新增 26 个提交。
- 使用 `codex/sync-20261120` 合并完整上游历史，验证后合回 `switchplus-personal`。
- 发布版本保持 1.0.2，数据库 schema 保持 27。本轮只更新源码，没有重新打包、安装或推送。
  2026-11-18 生成的 1.0.2 安装包仍对应之前的源码。

## 整合与适配

完整接入上游插件管理、私有 Skills 仓库访问令牌、计价倍率、不可读供应商/MCP 条目提示、
会话章节、Mistral Pi 预设，以及代理请求大小限制、连接复用、熔断健康重置和用量统计修复。

- 私有仓库令牌索引写入 SwitchPlus 设备目录，钥匙串服务使用 `switchplus-skill-repo`；
  阻止通过目录链接访问原版数据，使用内存凭据后端验证隔离。
- 移除自动合并重新引入的应用 updater 引用；CLI/插件的安装更新功能保留。
- 新插件测试改用隔离测试 HOME；MCP/tray 测试使用 SwitchPlus 环境变量和链接协议。
- 新 Mistral 供应商使用 `switchplus-mistral` 标识。插件缓存测试验证原版缓存不显示、不覆盖。
- 保留 8 种语言的 SwitchPlus 定制词条，完整纳入新增界面词条。
- 为上游新引入的 Knip 检查声明本地构建所需系统命令和类型声明文件；路由默认端口复用品牌常量。
- 保留 V9 默认过滤、整轮缓冲、重试预算、客户端取消、`gateway_queue_full` 和可移植压缩兼容。
- 保留身份、图标、独立目录、15731 默认端口、旧库文件名兼容和已有路由恢复。

## 验证

测试使用临时 HOME、临时库、模拟凭据和本机回环服务，不使用正在运行的客户端配置。

- `pnpm install --frozen-lockfile`、`pnpm typecheck`、`pnpm knip`：通过。
- `pnpm build:renderer`、`pnpm build:check`：通过。前者生成前端资源，后者检查身份和构建配置。
- `pnpm format:check`、`cargo fmt --check`：通过。
- `cargo clippy --locked --lib -- -D warnings`：通过。
- `cargo test --locked --lib --test integration --test switchplus_identity -- --test-threads=1`：
  5,107 项库单元测试、223 项集成测试和 3 项身份测试通过；11 项按原配置忽略。
  包括数据库迁移/导入导出、HTTP 429 重试、预算和取消、压缩、路由恢复及凭据隔离。
- `pnpm vitest run --maxWorkers=4 --minWorkers=1`：326 个测试文件、4,278 项测试全部通过。
  与编译并行时曾出现两项界面测试超时；相关 67 项单独复测通过，停止并行编译后全量复测通过。

## 验证边界

本机 Apple Silicon/macOS 完成编译和测试；没有运行 Windows/Linux 原生构建，未生成新安装包，
未替换正在运行的应用。新插件 CLI 行为使用上游固定测试样本，未调用真实账号执行安装。
本地原有 76 个同名 ` 2` 副本在同步前验证与对应文件完全相同，临时保存后原样恢复，未纳入提交。
