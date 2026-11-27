import { invoke } from "@tauri-apps/api/core";
import type { AppId } from "./types";

export interface WebsiteCheckStatus {
  url: string;
  checking: boolean;
  available: boolean | null;
  checkedAt: number | null;
  httpStatus: number | null;
  error: string | null;
}

export const websiteCheckApi = {
  status: (app: AppId, providerId: string) =>
    invoke<WebsiteCheckStatus | null>("get_website_check_status", {
      app,
      providerId,
    }),
  setEnabled: (app: AppId, providerId: string, enabled: boolean) =>
    invoke<void>("set_website_check_enabled", { app, providerId, enabled }),
  refresh: (app: AppId, providerId: string) =>
    invoke<void>("refresh_website_check", { app, providerId }),
};
