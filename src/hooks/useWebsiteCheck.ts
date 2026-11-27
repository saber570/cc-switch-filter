import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { websiteCheckApi } from "@/lib/api/websiteCheck";
import type { AppId } from "@/lib/api";
import type { Provider } from "@/types";
import type { ProvidersQueryData } from "@/lib/query/queries";
import { toast } from "@/lib/toast";
import { extractErrorMessage } from "@/utils/errorUtils";

export function validWebsiteUrl(raw: string | undefined): boolean {
  try {
    const url = new URL(raw?.trim() ?? "");
    return (
      ["http:", "https:"].includes(url.protocol) &&
      !!url.hostname &&
      !url.username &&
      !url.password
    );
  } catch {
    return false;
  }
}

export function useWebsiteCheck(provider: Provider, app: AppId) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const enabled = provider.meta?.websiteCheckEnabled === true;
  const key = ["website-check", app, provider.id, provider.websiteUrl ?? ""];
  const query = useQuery({
    queryKey: key,
    queryFn: () => websiteCheckApi.status(app, provider.id),
    enabled,
    // Reads only in-memory status. Network probing runs in Rust every 10 minutes,
    // even when this card is unmounted or the main window is hidden.
    refetchInterval: 2_000,
    staleTime: 0,
    retry: false,
  });
  const onError = (error: unknown) =>
    toast.error(
      t("websiteCheck.actionFailed", { defaultValue: "网站检测操作失败" }),
      { description: extractErrorMessage(error) },
    );
  const toggle = useMutation({
    mutationFn: (next: boolean) =>
      websiteCheckApi.setEnabled(app, provider.id, next),
    onSuccess: async (_, next) => {
      await client.cancelQueries({ queryKey: key });
      client.setQueryData(key, null);
      client.setQueryData<ProvidersQueryData>(["providers", app], (old) => {
        const current = old?.providers[provider.id];
        if (!old || !current) return old;
        return {
          ...old,
          providers: {
            ...old.providers,
            [provider.id]: {
              ...current,
              meta: { ...current.meta, websiteCheckEnabled: next },
            },
          },
        };
      });
      void client.invalidateQueries({ queryKey: ["providers", app] });
      void client.invalidateQueries({ queryKey: key });
    },
    onError,
  });
  const refresh = useMutation({
    mutationFn: () => websiteCheckApi.refresh(app, provider.id),
    onSuccess: async () => {
      await client.cancelQueries({ queryKey: key });
      client.setQueryData(key, null);
      void client.invalidateQueries({ queryKey: key });
    },
    onError,
  });
  return { enabled, query, toggle, refresh };
}
