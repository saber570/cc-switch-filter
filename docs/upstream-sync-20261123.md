# 2026-11-23 完整上游同步记录

## 范围与基线

- 本机同步前个人分支：`d74e7e229f46eefffc9aeb9c9bbe36de143cccb9`，对应上游 4.0.36 的完整合并。
- 先合入远程个人分支 `b557cad266f2afce4de3bb7a018aa394fd93fba9`，保留统一 `chow` 身份、旧安装重装保护及构建约定；合并提交为 `9c226adf32c7b8a237f6979a61b1145b49b9f103`。
- 上次上游基线：`6e06607290b805ff2d0b92fa4e6bc69526f5e740`（4.0.36）。
- 本轮完整上游：`721ca8e08c5233f41e9a79cc367d581ac1ec4bcd`（4.0.40），新增 124 个提交。
- 在 `codex/sync-20261123` 上合并完整上游历史，源码合并提交为 `c1b413496502a493f60f7aa6e9d56003cb98e82c`，
  验证及 Release 编译后已快进合回 `switchplus-personal`。后续提交只补充本文和构建说明，不改变编译源码。
- 发布版本保持 1.0.2，数据库 schema 保持 27。用户本轮要求更新源码并编译 Mac ARM64 Release，不生成 bundle 或安装包，不安装、不推送。

2026-11-18 的 Mac 1.0.2 安装包仍对应上游 4.0.35（`ad22d45`）时期的源码和身份，属于历史产物。
本轮保留 `com.chow.switchplus`，不能用旧包的验证结果代替本轮源码编译和新身份的安装验证。

## 整合与适配

完整纳入上游 4.0.37 至 4.0.40 的代码、依赖、测试和文档，包括数据库 WAL 与读取缓存、
路由请求复制优化、用量同步和会话搜索优化、Grok 官方卡绑定 xAI 账号、Antigravity CLI 插件、
供应商及模型缓存命中率、SQL 导入和深链接检查，以及 macOS 全屏窗口与工具检测修复。
上游新增的应用自身更新入口仍按个人版约定移除，CLI 和插件的安装更新功能保留。

- **V9 与请求重放兼容。** 上游 `ForwardBody` 首次使用已解析请求，重试时从保留字节重建请求。
  V9 入口改为保存过滤后的请求字节，避免内部故障转移或外层重试重新带回已删除的历史推理、消息 ID 和密文。
  首次请求和重放继续保留已选模型、`reasoning.effort`、明文正文与可移植压缩语义。
- **HTTP 回归覆盖。** 新增 `codex_v9_keeps_filtered_history_across_failover_and_outer_retry`，
  使用本机回环服务组合第一个供应商 HTTP 503、第二个供应商 SSE `stream_incomplete`、外层重试成功，
  检查每次上游请求都保持过滤结果和模型选择。已有真实 HTTP、压缩跨协议和搜索历史测试随接口一同适配；运行结果见下方验证状态。
- **测试拆分与分区。** 保留上游 Pi 表单测试拆分，并将个人版自定义字段和标识测试移动到对应新文件；原有 55 项用例无缺失或重复。
  Vitest 的 338 个文件恰好分到一个项目：114 个 `node-unisolated`、8 个 `node-isolated`、216 个 `jsdom`。
  `DatabaseCompatibility`、`SwitchPlusAbout`、`switchplusBuild` 和 `brand` 四个个人版测试文件均保留在 `jsdom`。
- **前端定制与身份隔离。** 保留 SwitchPlus 界面词条和中性供应商链接；移除冲突留下的 Modelflare 旧位置重复预设。
  新 StringAI 预设使用 `switchplus-string-ai`，插件失败重试测试使用 `switchplus-cache` 缓存键。
  保留产品名、图标、`com.chow.switchplus`、`switchplus` 二进制、`~/.switchplus` 数据目录、15731 默认端口和配置所有权保护。
- **格式检查。** 上游扩大 `format:check` 范围至 `tests`，因此一并修正 3 个既存测试文件的 Prettier 排版；仅调整格式，不改变测试行为。
- **后端测试适配。** 插件目录测试改用共享 `TestHome::path()`，保留空目录断言；
  WorkBuddy 深链接测试使用个人版既有的 `switchPlusProviderId` 归属字段，保留完整 JSON 比较和原版数据隔离行为。

## 已完成验证

- `pnpm install --frozen-lockfile`：通过，依赖与锁文件一致。
- `pnpm typecheck`、`pnpm build:check`、`pnpm knip`：通过；`build:check` 仅验证构建配置，不代表发布编译完成。
- 全量 Vitest：338 个测试文件、4,429 项用例全部通过。
- `pnpm format:check`：修正相关格式后全量通过。
- `git diff --check -- src tests`：通过。
- Rust 库全量执行 5,220 项：5,206 项通过、13 项按原设置忽略，唯一失败为新增 WorkBuddy 测试使用原版归属字段；
  修正该测试后定向复测通过，合计 5,207 项有效库测试通过。未改生产行为，因此没有重复执行其余已通过的库测试。
- Rust 集成测试 224 项、应用身份测试 3 项全部通过；测试使用隔离 `SWITCHPLUS_TEST_HOME`、临时数据库和本机回环服务。
  新增 V9 故障转移与外层重试 HTTP 回归已通过，数据库迁移/导入导出、代理/压缩、重试预算和取消测试均包含在库及集成测试中。
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`：通过。
- `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings`：通过；
  新增 V9 回归中删除一个无意义的同类型转换后复查，无警告。

13 项忽略用例需要真实凭据、真实会话或备份语料、真实钥匙串、远程服务或手动性能基准，保留其原有运行条件。
差异空白检查除上游 Antigravity CLI 原始输出 fixture 的末尾空行外通过，该 fixture 按原始输出保留。

## Release 编译与收尾

从干净提交 `c1b413496502a493f60f7aa6e9d56003cb98e82c` 执行：

```sh
pnpm build:check
pnpm exec tauri build --ci --no-sign --no-bundle --target aarch64-apple-darwin -- --locked
```

- 前端生产构建通过（3,602 个模块）；Rust Release 编译通过，耗时 5 分 32 秒。
- 源码 tree：`68cbe2afd01de5455ba70fe7b21c46b50be061d7`；构建开始前 tracked 和 untracked 均干净。
- 产物：`src-tauri/target/aarch64-apple-darwin/release/switchplus`。
- `file` / `lipo -archs`：Mach-O 64-bit executable，`arm64`；大小 46,880,512 字节。
- SHA256：`d17e498d868bc915dfda8864be5b42abfaf1c48c703f78f143f157f5393b6726`。
- Tauri 明确禁用 bundle，未生成 APP/DMG；未启动产物、安装或替换应用。此可执行文件不代表安装流程已经验证。
- 前端构建保留浏览器兼容数据过期、Tailwind `duration-[120ms]` 歧义及大 chunk 提示，均未阻止构建；没有为了消除提示改动锁定依赖。
- `switchplus-personal` 已包含远程个人提交 `b557cad2` 和上游 `721ca8e0` 的完整祖先关系。
- 本机原有 120 个文件名带 ` 2` 的副本在构建完成后从 stash `82eac151` 的未跟踪树恢复，
  逐文件 blob 校验 120/120 一致，共 1,647,399 字节；恢复后 tracked 工作区与暂存区均干净。
  副本仍为未跟踪文件，未纳入提交。再次全量测试或编译前应暂存隔离这些副本，以免旧副本进入文件扫描。

本轮不推送、不生成安装包、不安装或替换正在运行的应用，也未进行 Windows/Linux 原生构建或新身份的 Mac 安装验证。
