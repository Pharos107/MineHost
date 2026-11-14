import { request, serverPath } from "./client";
import type { components } from "./schema";

export type ContentKind = components["schemas"]["ContentKindDto"];
export type ContentSource = components["schemas"]["ContentSourceDto"];
export type InstalledContent = components["schemas"]["InstalledContentDto"];
export type ServerContent = components["schemas"]["ServerContentDto"];

export function contentKey(serverId: string) {
  return ["content", serverId] as const;
}

function filePath(serverId: string, fileName: string): string {
  return `${serverPath(serverId)}/content/${encodeURIComponent(fileName)}`;
}

/** Route quản lý mod/plugin của một server (spec P3, mục 5.8). */
export const contentApi = {
  list: (serverId: string) => request<ServerContent>("GET", `${serverPath(serverId)}/content`),
  toggle: (serverId: string, fileName: string, enabled: boolean) =>
    request<void>("PATCH", filePath(serverId, fileName), { enabled }),
  remove: (serverId: string, fileName: string) => request<void>("DELETE", filePath(serverId, fileName)),
  upload: (serverId: string, file: File) => {
    const form = new FormData();
    form.append("file", file, file.name);
    return request<InstalledContent>("POST", `${serverPath(serverId)}/content/upload`, form);
  },
};
