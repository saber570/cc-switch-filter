# 2026-11-16 完整上游同步记录

## 范围与维护方向

用户确认改为“完整同步上游、保留 SwitchPlus 定制”。规则已写入根目录
`AGENTS.md` 和 `docs/upstream-sync.md`，替代以前以选择移植为主的常规策略。

- 同步前本地：`36970da009d322c772893ac8f76b57c3d7072ada`。
- 原全量共同基线：`8656ebd91f3eba631837126e29df71e8353b7e43`。
- 本轮上游：`86e23b6331b9ce2cee314d1ebb5b186da0de645a`，4.0.33，距原基线 135 个提交。
- 先合入已获取的 4.0.32，再在网络恢复后获取并合入 4.0.33 的三个提交，保留真实 Git merge 祖先关系。
- 本地发布版本保持 **1.0.1**。本轮没有打包、安装或发布，也未推送远端。

## 整合结果

- 完整接入上游新应用、应用能力表、隐私过滤、新语言、同步及请求协议修复。
- 数据库保持上游 schema 26；迁移、AppType、MCP/Skills 字段、DAO、界面和导入导出同步整合。
  `rusqlite` 为 0.31.0，`libsqlite3-sys` 为 0.28.0，与上游一致。
  以前遗漏的是 schema/应用字段支持，不是 SQLite 引擎升级。
- 保留数据库中毒锁恢复、配置锁、共享自动同步实现，以及最新 WebDAV/S3 网络错误原因提示。
- 保留 V9 过滤和缓冲重试、可移植摘要压缩、模型映射、重试时间头传递。
  上游隐私过滤沿用默认关闭设置，独立于第三方 Codex 的默认 V9 行为。
- 保留 SwitchPlus 身份、图标、独立数据目录、15731 默认端口、旧库文件兼容和首次启动配置所有权保护。
- 保留安装模板及旧安装路径迁移；Windows/macOS 使用同一组受版本管理的图标。
- 移除上游重新引入的应用更新模块、后台任务、托盘入口、前端提示和相关接口；工具更新照常保留。
- WorkBuddy 新配置使用 `switchPlusProviderId` 归属标记；测试验证相同供应商 ID 的原版条目不会被改写或删除。
- 修正新增 Copilot 预设的 SwitchPlus 前缀适配、新测试中的链接协议/命令变量，去除以前移植造成的重复测试。
- CI 使用 `SWITCHPLUS_TEST_HOME`；语言完整性检查和 README 对照测试适配 fork 定制说明。

## 验证

所有数据库、客户端配置和账号模拟测试均使用临时目录或隔离测试 HOME。

- `pnpm install --frozen-lockfile`：通过。
- `pnpm typecheck`：通过。
- `pnpm vitest run`：319 个文件、4,113 项通过。
- `pnpm build:renderer`：通过；没有生成安装包。
- `pnpm format:check`、`cargo fmt --check`：通过。
- WSL Linux `cargo test --locked --offline --lib -- --test-threads=1`：4,896 项通过，15 项忽略，0 失败。
  包含 V9 集成/单元测试、摘要压缩、模型映射、隐私过滤、schema 24→26、schema 26 导入编辑导出及未来版本拒绝。
- Windows x86_64 MSVC `cargo xwin check --locked --offline --target x86_64-pc-windows-msvc --lib`：通过。
- Windows/macOS 构建配置和图标一致性测试通过；`build:check` 仅表示配置检查，不代表生成安装包。
- 独立集成测试：70 项通过（身份 3、导入导出 19、MCP 42、WorkBuddy 6），0 失败。

## 验证边界

未执行 Windows 覆盖安装、macOS 原生编译或运行，也未调用真实付费供应商。
15 项忽略测试需要特定系统/真实服务等环境，未强行运行。
编译存在上游未使用函数警告，界面构建存在资源块大小提示，不影响上述检查通过。
