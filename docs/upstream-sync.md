# 合并上游更新

本分支现在以 **SwitchPlus** 个人版构建，身份与目录隔离说明见 [SwitchPlus](switchplus.md)。

此 fork 是 `saber570/cc-switch-filter`，直接上游是
[`farion1231/cc-switch`](https://github.com/farion1231/cc-switch)，两者默认分支均为 `main`。
过滤功能可以与上游更新一起保留；同一位置的修改可能需要人工解决冲突。

## 当前维护方向（2026-11-16 用户确认）

**默认完整合并上游，再保留 SwitchPlus 定制层。** 不再把“只挑几项安全修复”当作常规同步策略。
上次遗漏的是数据库 schema 版本及完整应用支持，不是 SQLite 引擎版本；后续必须一起核对：

1. 上游迁移链、`SCHEMA_VERSION`、实际 `PRAGMA user_version`、旧库迁移与未来版本拒绝逻辑。
2. AppType / 前后端数据结构、MCP/Skills 开关、DAO、备份导入导出和新增应用页面。
3. Cargo / pnpm 依赖及锁文件、相关测试；已有移植补丁与测试去重。
4. 请求转换、流响应、重试与 V9 的组合行为，不因本地有过滤逻辑就整块跳过上游修复。

明确保留的定制：SwitchPlus 名称/图标/标识/数据目录、安装路径及覆盖安装兼容、配置所有权保护、
V9 始终启用及压缩兼容、禁用应用自身更新。隐私脱敏是上游独立功能，沿用上游默认设置，不等同于 V9。
许可证、原作者版权、真实来源链接和必要的历史协议标记保留。
具体执行约束见根目录 [AGENTS.md](../AGENTS.md)。

**普通合并不升级版本、不打包。只有用户明确要求升级版本号并打包时才执行。**
本轮保持 1.0.1；上游 4.x 版本仅用于说明来源，不写入本项目的发布版本。

本轮目标上游：`1d68de345884e2484410c427623b386b5b953130`（2026-11-15，4.0.32 源码）。
验证结果与合并状态另记于本轮同步记录，不能把下列历史选择合入记录当成当前策略。

## 历史记录（不再作为默认同步策略）

## 2026-11-13：1.0.0 选择合入

本轮保留 `8656ebd`（上游 4.0.28）为全量基线，选择合入以下修复，提交附有 `cherry picked from` 来源：

- Codex 请求兼容：`72e826e`、`a6200c1`、`08ec82a`、`74f0d41`。
- Windows/WSL 模型目录：`5c46226`；用量扫描与旧会话补记：`4376e5d`、`457f898`。
- 窗口/托盘：`5628bb1`、`479e632`、`063c22d`。
- 编辑页性能：`d0e47b2`；请求状态码筛选：`72fe4ed`。

不等同于完整升级到上游 4.0.31。隐私脱敏、新应用和新语言暂未引入，数据库保持 schema 24。
后续评估先用 `git log --cherry-pick` 或 `git cherry` 检查已移植提交，避免重复引入。
个人版从 1.0.0 独立编号，不同步上游版本号。NSIS 模板基于 Tauri CLI 2.12.4，
后续升级构建工具时需对比模板变化，保留版本迁移、当前用户 MSI 检测和安装路径恢复逻辑；
可用 `powershell -ExecutionPolicy Bypass -File scripts/test-installer-version.ps1` 验证真实 NSIS 版本判断。

## 2026-11-15：1.0.1 数据库备份兼容

补入上游 schema 25（Copilot CLI 开关）和 schema 26（WorkBuddy 开关及早期开发库补列）的数据库迁移。
数据库支持提升到 schema 26，MCP/Skills 保存时保留尚未在本版界面中开放的应用开关。
这是数据库兼容修复，不代表已全量升级到上游 4.0.31，也不代表已引入 Copilot CLI、WorkBuddy 应用管理功能。

## 首次准备

合并更新前，先检查并提交需要保留的本地改动，确保工作区干净。
源码压缩包不含 `.git`，持续维护应使用 Git 克隆的仓库，并将修改纳入提交。

先检查是否为浅克隆。必要时补齐历史后再判断共同祖先，不能使用
`--allow-unrelated-histories` 绕过共同历史检查。

```sh
# 已有同名 upstream 时，先用 git remote -v 核对地址，无需重复添加。
git remote add upstream https://github.com/farion1231/cc-switch.git
# 仅当 git rev-parse --is-shallow-repository 输出 true 时执行：
git fetch --unshallow origin
git fetch upstream
```

## 每次同步

在单独的同步分支上合并和验证，保留 fork 的功能提交及合并历史：

```sh
git switch switchplus-personal
git pull --ff-only origin switchplus-personal
git fetch upstream
git switch -c sync/upstream-YYYYMMDD
git merge upstream/main
```

如有冲突，结合两边行为处理并提交合并；重点检查供应商元数据、供应商编辑表单、
代理处理入口、重试头传递及摘要压缩适配，以及 `identity.rs`、`brand.ts`、Tauri 应用标识、
默认端口与更新禁用设置。不要用强制重置到 upstream 的方式同步，
那样会移除 fork 的功能提交。

过滤逻辑和测试集中在 `src-tauri/src/proxy/codex_v9_*`；压缩复用
`providers/codex_compaction.rs` 的识别、解码和协议转换，避免维护另一套摘要实现。

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm vitest run tests/components/CodexFormFields.test.tsx tests/components/ProviderForm.codexCopilot.test.tsx tests/components/DatabaseCompatibility.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib proxy::
corepack pnpm build:check
corepack pnpm build:renderer
```

验证通过后，将同步分支合并回 fork 的 `switchplus-personal`（也可在自己的 fork 提 PR），本次只完成源码同步。安装包必须等待用户明确要求升级版本号并打包。
GitHub 的 Sync fork 能处理无冲突的代码同步，但不会代替这些兼容测试。
本地构建配置关闭官方更新源，避免已安装的定制版本被不含过滤功能的官方二进制覆盖。
