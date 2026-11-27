import { useTranslation } from "react-i18next";
import { HoverTip } from "@/components/ui/hover-tip";
import { cn } from "@/lib/utils";
import type { WebsiteCheckStatus } from "@/lib/api/websiteCheck";

export function WebsiteStatusDot({
  status,
  error,
  refreshing,
  onRefresh,
}: {
  status: WebsiteCheckStatus | null | undefined;
  error: boolean;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  const { t, i18n } = useTranslation();
  const checking = !error && (refreshing || !status || status.checking);
  const stale =
    !!status?.checkedAt && Date.now() - status.checkedAt > 11 * 60_000;
  const available = error || stale || checking ? null : status?.available;
  const label = checking
    ? t("websiteCheck.checking", { defaultValue: "网站检测中" })
    : available === true
      ? t("websiteCheck.available", { defaultValue: "网站可访问" })
      : available === false
        ? t("websiteCheck.unavailable", { defaultValue: "网站不可用" })
        : t("websiteCheck.unknown", { defaultValue: "网站状态待更新" });
  const hint = (
    <div className="max-w-[320px] space-y-1 break-words text-start">
      <div>{label}</div>
      {status?.url && <div>{status.url}</div>}
      {status?.checkedAt && (
        <div>
          {t("websiteCheck.lastChecked", {
            defaultValue: "最近检测：{{time}}",
            time: new Date(status.checkedAt).toLocaleString(i18n.language),
          })}
        </div>
      )}
      {status?.error && <div>{status.error}</div>}
      <div>
        {t("websiteCheck.hint", {
          defaultValue:
            "每 10 分钟检测网站；不代表模型接口可用。点击立即重测。",
        })}
      </div>
    </div>
  );
  return (
    <HoverTip content={hint}>
      <button
        type="button"
        aria-label={label}
        aria-busy={checking}
        disabled={checking}
        onClick={onRefresh}
        className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <span
          aria-hidden="true"
          className={cn(
            "h-2 w-2 rounded-full",
            available === true
              ? "bg-success"
              : available === false
                ? "bg-danger"
                : "bg-fg-3",
            checking && "animate-pulse",
          )}
        />
      </button>
    </HoverTip>
  );
}
