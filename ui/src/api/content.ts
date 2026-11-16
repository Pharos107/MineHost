import { request, serverPath, type Created } from "./client";
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
export type PendingContent = components["schemas"]["PendingContentDto"];
export type PendingUploadItem = components["schemas"]["PendingUploadItemDto"];
export type PendingUploadResult = components["schemas"]["PendingUploadResultDto"];
export type ContentInstallBody = components["schemas"]["ContentInstallBody"];
export type ContentItem = components["schemas"]["ContentItemBody"];
export type SearchSort = "relevance" | "downloads" | "updated";
export type CreateModpackBody = components["schemas"]["CreateModpackBody"];
export type JobStarted = components["schemas"]["JobStartedDto"];
export type UpdateReport = components["schemas"]["UpdateReportDto"];
export type UpdateCandidate = components["schemas"]["UpdateCandidateDto"];
export type UpdateItemBody = components["schemas"]["UpdateItemBody"];

export interface ModpackSummary {
  installed_mods: number;
  skipped_client: string[];
  pending_curseforge: string[];
}

export interface SearchContentParams {
  source?: string;
  kind?: "mod" | "plugin" | "modpack";
  q?: string;
  serverId?: string;
  page?: number;
  sort?: SearchSort;
}

export function contentKey(serverId: string) {
  return ["content", serverId] as const;
}

function filePath(serverId: string, fileName: string): string {
  return `${serverPath(serverId)}/content/${encodeURIComponent(fileName)}`;
}

/** Route tìm kiếm và quản lý mod/plugin của một server (spec P3, mục 5.8). */
export const contentApi = {
  search: ({ source = "modrinth", kind, q = "", serverId, page = 1, sort }: SearchContentParams) => {
    const params = new URLSearchParams({ source, q, page: String(page) });
    if (sort) params.set("sort", sort);
    if (kind) params.set("kind", kind);
    if (serverId) params.set("server_id", serverId);
    return request<SearchResult>("GET", `/content/search?${params.toString()}`);
  },
  project: (source: string, projectId: string) =>
    request<ContentProject>("GET", `/content/${encodeURIComponent(source)}/projects/${encodeURIComponent(projectId)}`),
  versions: (source: string, projectId: string, serverId?: string) => {
    const query = serverId ? `?server_id=${encodeURIComponent(serverId)}` : "";
    return request<ContentVersion[]>(
      "GET",
      `/content/${encodeURIComponent(source)}/projects/${encodeURIComponent(projectId)}/versions${query}`,
    );
  },
  /** Kế hoạch cài gộp cho nhiều mục (1–50), dependency dùng chung chỉ một dòng. */
  plan: (serverId: string, items: ContentItem[]) =>
    request<ContentPlan>("POST", `${serverPath(serverId)}/content/plan`, { items } satisfies ContentInstallBody),
  /** Một Job cài mọi mục cùng dependency. */
  install: (serverId: string, items: ContentItem[]) =>
    request<JobStarted>("POST", `${serverPath(serverId)}/content`, { items } satisfies ContentInstallBody),
  list: (serverId: string) => request<ServerContent>("GET", `${serverPath(serverId)}/content`),
  /** Job hỏi nguồn mod xem có bản mới; kết quả (`UpdateReport`) nằm trong `result` của Job. */
  checkUpdates: (serverId: string) => request<JobStarted>("POST", `${serverPath(serverId)}/updates/check`),
  /** Job tắt server, sao lưu `before_update` rồi cài các bản đã chọn. */
  applyUpdates: (serverId: string, items: UpdateItemBody[]) =>
    request<JobStarted>("POST", `${serverPath(serverId)}/updates`, { items }),
  toggle: (serverId: string, fileName: string, enabled: boolean) =>
    request<void>("PATCH", filePath(serverId, fileName), { enabled }),
  remove: (serverId: string, fileName: string) => request<void>("DELETE", filePath(serverId, fileName)),
  upload: (serverId: string, file: File) => {
    const form = new FormData();
    form.append("file", file, file.name);
    return request<InstalledContent>("POST", `${serverPath(serverId)}/content/upload`, form);
  },
  uploadPending: (serverId: string, files: File[]) => {
    const form = new FormData();
    for (const file of files) form.append("file", file, file.name);
    return request<PendingUploadResult>("POST", `${serverPath(serverId)}/content/pending/upload`, form);
  },
};

/** Route tạo server từ modpack (spec P3, mục 5.8). */
export const modpackApi = {
  create: (body: CreateModpackBody) => request<Created>("POST", "/servers/modpack", body),
  upload: (form: FormData) => request<Created>("POST", "/servers/modpack/upload", form),
};
