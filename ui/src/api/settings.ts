import { request } from "./client";
import type { components } from "./schema";

export type CurseforgeSettings = components["schemas"]["CurseforgeSettingsDto"];
export type CurseforgeKeySource = components["schemas"]["CurseforgeKeySourceDto"];

export type DownloadCache = components["schemas"]["DownloadCacheDto"];
export type ClearedCache = components["schemas"]["ClearedCacheDto"];

export const CURSEFORGE_KEY = ["settings", "curseforge"] as const;
export const DOWNLOAD_CACHE_KEY = ["settings", "download-cache"] as const;

/** Trạng thái key CurseForge. Response không bao giờ trả lại key. */
export const settingsApi = {
  curseforge: () => request<CurseforgeSettings>("GET", "/settings/curseforge"),
  setCurseforgeKey: (apiKey: string) =>
    request<CurseforgeSettings>("PUT", "/settings/curseforge", { api_key: apiKey }),
  clearCurseforgeKey: () => request<CurseforgeSettings>("DELETE", "/settings/curseforge"),
  downloadCache: () => request<DownloadCache>("GET", "/settings/download-cache"),
  clearDownloadCache: () => request<ClearedCache>("DELETE", "/settings/download-cache"),
};
