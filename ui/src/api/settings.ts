import { request } from "./client";
import type { components } from "./schema";

export type CurseforgeSettings = components["schemas"]["CurseforgeSettingsDto"];
export type CurseforgeKeySource = components["schemas"]["CurseforgeKeySourceDto"];

export const CURSEFORGE_KEY = ["settings", "curseforge"] as const;

/** Trạng thái key CurseForge. Response không bao giờ trả lại key. */
export const settingsApi = {
  curseforge: () => request<CurseforgeSettings>("GET", "/settings/curseforge"),
  setCurseforgeKey: (apiKey: string) =>
    request<CurseforgeSettings>("PUT", "/settings/curseforge", { api_key: apiKey }),
  clearCurseforgeKey: () => request<CurseforgeSettings>("DELETE", "/settings/curseforge"),
};
