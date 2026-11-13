import { useTranslation } from "react-i18next";
import { Switch } from "@/components/ui/switch";

export function CodexV9FilterField({
  enabled,
  onChange,
}: {
  enabled: boolean;
  onChange: (enabled: boolean) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-2 border-t border-border pt-3">
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="codex-v9-filter" className="text-sm font-medium">
          {t("codexConfig.v9FilterLabel", {
            defaultValue: "V9 历史过滤与自动重试",
          })}
        </label>
        <Switch
          id="codex-v9-filter"
          checked={enabled}
          onCheckedChange={onChange}
        />
      </div>
      <p className="text-xs leading-relaxed text-fg-2">
        {t("codexConfig.v9FilterHelp", {
          defaultValue:
            "仅在路由、聚合模式生效。从此供应商开始的请求会清理历史推理和可移除的 ID，暂存整轮响应，成功后才显示内容和执行工具；临时限流、断流等错误合计最多额外重试 20 次，总期限 30 分钟。",
        })}
      </p>
      {enabled && (
        <p className="text-xs leading-relaxed text-fg-2" role="status">
          {t("codexConfig.v9FilterNotice", {
            defaultValue:
              "保留正文、工具结果和本轮推理强度，但丢弃历史内部推理。支持文字摘要及 SwitchPlus 摘要压缩；拒绝无法解读的压缩密文、服务端加密压缩和服务器上下文引用。输出会延迟，重试可能增加上游用量；不要同时启用外置 V9 过滤服务。",
          })}
        </p>
      )}
    </div>
  );
}
