export const APP_NAME = "SwitchPlus";
export const APP_UPDATES_ENABLED = false;
export const PROJECT_URL = "https://github.com/saber570/cc-switch-filter";
export const UPSTREAM_URL = "https://github.com/farion1231/cc-switch";

// Keep upstream translation files easy to merge; brand only displayed text.
export function brandTranslations<T>(value: T): T {
  if (typeof value === "string") {
    return value
      .replace(/CC Switch/g, APP_NAME)
      .replace(/\.cc-switch/g, ".switchplus")
      .replace(/cc-switch-sync/g, "switchplus-sync") as T;
  }
  if (Array.isArray(value)) return value.map(brandTranslations) as T;
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, text]) => [
        key,
        brandTranslations(text),
      ]),
    ) as T;
  }
  return value;
}
