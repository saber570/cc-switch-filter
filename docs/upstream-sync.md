# 合并上游更新

本分支现在以 **SwitchPlus** 个人版构建，身份与目录隔离说明见 [SwitchPlus](switchplus.md)。

此 fork 是 `saber570/cc-switch-filter`，直接上游是
[`farion1231/cc-switch`](https://github.com/farion1231/cc-switch)，两者默认分支均为 `main`。
过滤功能可以与上游更新一起保留；同一位置的修改可能需要人工解决冲突。

## 首次准备

合并更新前，先检查并提交需要保留的本地改动，确保工作区干净。
源码压缩包不含 `.git`，持续维护应使用 Git 克隆的仓库，并将修改纳入提交。

当前本机克隆是浅克隆。补齐历史后才适合判断共同祖先，不能使用
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
pnpm install --frozen-lockfile
pnpm typecheck
pnpm vitest run tests/components/CodexV9FilterField.test.tsx tests/components/CodexFormFields.test.tsx tests/components/ProviderForm.codexCopilot.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib proxy::
pnpm tauri build --config src-tauri/tauri.local-build.conf.json
```

验证通过后，将同步分支合并回 fork 的 `switchplus-personal`（也可在自己的 fork 提 PR），再构建新安装包。
GitHub 的 Sync fork 能处理无冲突的代码同步，但不会代替这些兼容测试。
本地构建配置关闭官方更新源，避免已安装的定制版本被不含过滤功能的官方二进制覆盖。
