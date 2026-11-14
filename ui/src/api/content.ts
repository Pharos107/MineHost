import { request, serverPath } from "./client";
import type { components } from "./schema";

export type ContentKind = components["schemas"]["ContentKindDto"];
export type ContentSource = components["schemas"]["ContentSourceDto"];
export type InstalledContent = components["schemas"]["InstalledContentDto"];
export type ServerContent = components["schemas"]["ServerContentDto"];
export type SearchHit = components["schemas"]["SearchHitDto"];
export type SearchResult = components["schemas"]["SearchResultDto"];
export type ContentProject = components["schemas"]["ProjectDto"];
export type ContentVersion = components["schemas"]["ContentVersionDto"];
export type ContentPlan = components["schemas"]["ContentPlanDto"];
export type PlannedContent = components["schemas"]["PlannedContentDto"];
export type ContentInstallBody = components["schemas"]["ContentInstallBody"];
export type JobStarted = components["schemas"]["JobStartedDto"];

export interface SearchContentParams {
  source?: string;
  kind?: "mod" | "plugin" | "modpack";
  q?: string;
  serverId?: string;
  page?: number;
}

export function contentKey(serverId: string) {
  return ["content", serverId] as const;
}

function filePath(serverId: string, fileName: string): string {
  return `${serverPath(serverId)}/content/${encodeURIComponent(fileName)}`;
}

/** Route tìm kiếm và quản lý mod/plugin của một server (spec P3, mục 5.8). */
export const contentApi = {
  search: ({ source = "modrinth", kind, q = "", serverId, page = 1 }: SearchContentParams) => {
    const params = new URLSearchParams({ source, q, page: String(page) });
    if (kind) params.set("kind", kind);
    if (serverId) params.set("server_id", serverId);
    return request<SearchResult>("GET", `/content/search?${params.toString()}`);
  },
  project: (source: string, projectId: string) =>
    request<ContentProject>("GET", `/content/${encodeURIComponent(source)}/projects/${encodeURIComponent(projectId)}`),
  versions: (source: string, projectId: string, serverId: string) =>
    request<ContentVersion[]>(
      "GET",
      `/content/${encodeURIComponent(source)}/projects/${encodeURIComponent(projectId)}/versions?server_id=${encodeURIComponent(serverId)}`,
    ),
  plan: (serverId: string, body: ContentInstallBody) =>
    request<ContentPlan>("POST", `${serverPath(serverId)}/content/plan`, body),
  install: (serverId: string, body: ContentInstallBody) =>
    request<JobStarted>("POST", `${serverPath(serverId)}/content`, body),
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
