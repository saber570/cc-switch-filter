# Codex V9 内置过滤

本分支以 SwitchPlus 个人版构建；下文 CC Switch 指复用的上游代理与摘要实现。

在 **Codex → 编辑供应商 → 高级设置 → V9 历史过滤与自动重试** 开启。
默认关闭，仅对经 CC Switch 本地代理的 Codex 请求生效；直连不会经过过滤。
配置保存在供应商元数据 `meta.codexV9Filter`，不写入 Codex 配置文件。

该供应商作为请求的起始目标时启用 V9 过滤与重试，整次请求共用重试预算，
包括 CC Switch 内部故障转移后的响应。聚合模式按请求选中的供应商决定。
每个新请求重新读取设置；当前已经开始的请求继续使用开始时的设置。
官方登录供应商不启用此模式。

## 行为

- 清理历史 `reasoning` 条目，以及普通消息、函数/自定义工具调用和结果的顶层 `id`。
  保留正文、图片、文件引用、`call_id`、工具参数和结果，以及当前的 `reasoning.effort`。
- 设置 `store: false`，移除 `include` 中的 `reasoning.encrypted_content`。
- 整轮 SSE 暂存到 `response.completed` 再交付，保留成功响应里的推理。
  失败尝试中的文本和工具调用不会交给客户端；不可重试的失败只交付错误事件。
  `response.incomplete` 原样返回且不重放。
- 临时限流最多额外 20 次；断流/超时/已识别 ALB HTML 400 共用 10 次；
  无可用渠道 10 次；Azure 资源归属错误 5 次。各类总计最多额外 20 次，
  总期限 30 分钟，读取静默超时 120 秒。服务端 Retry-After 是等待下限。
- 等待时每 15 秒发送 V9 的空 `response.in_progress` 保活，输出数组为空，
  不发送模型文字或工具调用，不伪造完成事件。
  客户端关闭响应连接后，当前转发及重试等待随响应体一起取消。
- 请求和响应暂存限额 64 MiB，支持 gzip、deflate、br、zstd 响应解压。
  请求解压沿用 CC Switch 现有逻辑。
- 支持普通文字摘要，以及 CC Switch 的摘要压缩：保留 `compaction_trigger`，
  交给已有 Responses / Chat / Anthropic 适配器生成摘要；成功响应仍经 V9 缓冲交付。
  后续请求中的 CC Switch 摘要条目解回普通文字消息，切换供应商时完整携带摘要。
  支持 `compaction`、`compaction_summary`、`context_compaction` 三种摘要条目名称。
- 拒绝未知、损坏或空白的压缩载荷，以及 `item_reference`、`previous_response_id`
  和 `conversation`。整份历史验证通过后才转换，不把无法解读的上下文换成占位文字。
- `/responses/compact` 和 `context_management: [{type: "compaction"}]` 仍被拒绝：
  这两条路径未接入 CC Switch 的可移植文字摘要转换，可能生成供应商专属密文。
  是否可用取决于压缩格式，不取决于用户手动触发还是客户端自动触发。

这是内置版相对于独立 V9 的兼容扩展：允许可解码的 CC Switch 摘要，而非拒绝所有压缩条目。

CC Switch 原有供应商选择、故障转移、鉴权和协议转换仍然运行。
20 次指外层额外尝试，内部故障转移可能导致更多上游调用。
用量与请求日志保留各次尝试，丢弃输出不代表未产生上游用量。
过滤日志使用 `[CodexV9]` 前缀，仅记录数量、重试类别和等待时间，不记录正文和密钥。

## 从外置 V9 迁移

1. 在没有进行中任务时，正常停止外置 V9 服务，让它恢复 Codex 地址。
2. 保持 CC Switch 的本地代理/路由模式开启，启用目标供应商的 V9 开关。
3. 重开之前缓存了 `18181` 地址的 Codex 客户端。

不再需要 Node.js 过滤进程、18181 端口或配置文件轮询。
不要同时串联外置 V9，避免两层暂存和重试。
遇到无法解读的旧压缩密文时，需要在能读取它的原环境先生成文字摘要，再接续会话。

## 开发验证

```sh
pnpm install --frozen-lockfile
pnpm typecheck
pnpm vitest run tests/components/CodexV9FilterField.test.tsx tests/components/CodexFormFields.test.tsx tests/components/ProviderForm.codexCopilot.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib codex_v9
```

测试使用模拟响应和本机回环 HTTP 服务，不调用真实模型。构建沿用仓库的 Tauri 流程。
本地未配置更新签名密钥时，可使用 `pnpm tauri build --config src-tauri/tauri.local-build.conf.json`。
macOS 需要 Xcode Command Line Tools 和对应 Rust 工具链，在 Mac 上运行。

V9 逻辑来源及 MIT 许可证见根目录 `THIRD_PARTY_NOTICES.md`。

后续合并原版更新的流程见 [fork 维护说明](upstream-sync.md)。
