import { useEffect, useState } from "react";
import { ExternalLink, Github, Loader2 } from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { settingsApi } from "@/lib/api";
import { APP_NAME, APP_AUTHOR, PROJECT_URL, UPSTREAM_URL } from "@/lib/brand";
import switchplusLogo from "@/assets/icons/switchplus.svg";

export function AboutSection({ isPortable }: { isPortable: boolean }) {
  const { t } = useTranslation();
  const [version, setVersion] = useState<string>();
  useEffect(() => {
    let active = true;
    getVersion().then(
      (value) => {
        if (active) setVersion(value);
      },
      () => {
        if (active) setVersion("—");
      },
    );
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="divide-y divide-border overflow-hidden rounded-panel border border-border bg-surface">
      <div className="flex items-center gap-4 px-5 py-4">
        <img
          src={switchplusLogo}
          alt=""
          className="h-12 w-12"
          draggable={false}
        />
        <div className="space-y-1">
          <div className="text-title text-fg-1">{APP_NAME}</div>
          <div className="text-caption text-fg-2">{APP_AUTHOR}</div>
          <div className="flex items-center gap-2 text-caption text-fg-2">
            {version ? (
              <span>v{version}</span>
            ) : (
              <Loader2 className="h-3 w-3 animate-spin" />
            )}
            <Badge variant="secondary">
              {t("switchplus.personal", "个人自用版")}
            </Badge>
            {isPortable && (
              <Badge variant="secondary">{t("settings.portableMode")}</Badge>
            )}
          </div>
        </div>
      </div>
      <div className="space-y-3 px-5 py-4 text-body text-fg-2">
        <p>
          {t(
            "switchplus.features",
            "保留供应商管理、路由和摘要压缩。第三方 Codex 请求默认启用历史过滤与自动重试。",
          )}
        </p>
        <p>
          {t(
            "switchplus.coexist",
            "使用独立的数据目录 ~/.switchplus，默认代理端口 15731。接管同一个客户端前，请先退出 CC Switch 或其他配置管理软件，避免覆盖彼此的配置。",
          )}
        </p>
        <p>
          {t(
            "switchplus.attribution",
            "基于 CC Switch 与 AgentRouter-Codex-Filter（MIT）修改。原作者版权与许可证保留在安装资源中。",
          )}
        </p>
      </div>
      <div className="flex flex-wrap gap-2 px-5 py-4">
        <Button
          variant="neutral"
          size="compact"
          onClick={() => settingsApi.openExternal(PROJECT_URL)}
        >
          <Github className="h-3.5 w-3.5" />
          {t("switchplus.source", "个人仓库")}
        </Button>
        <Button
          variant="neutral"
          size="compact"
          onClick={() => settingsApi.openExternal(UPSTREAM_URL)}
        >
          <ExternalLink className="h-3.5 w-3.5" />
          {t("switchplus.upstream", "上游 CC Switch")}
        </Button>
      </div>
    </div>
  );
}
