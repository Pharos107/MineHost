import type { TFunction } from "i18next";

import type { components } from "./schema";

// Kiểu dữ liệu sinh từ OpenAPI của backend (`npm run gen:api`), không tự viết tay.
export type SystemInfo = components["schemas"]["SystemDto"];
export type ServerState = components["schemas"]["StateDto"];
export type Server = components["schemas"]["ServerDto"];
export type Job = components["schemas"]["JobDto"];
export type MetricSample = components["schemas"]["MetricSampleDto"];
export type ServerMetrics = components["schemas"]["ServerMetricsDto"];
export type JobProgress = components["schemas"]["ProgressDto"];
export type CatalogVersion = components["schemas"]["CatalogVersionDto"];
export type CreateServer = components["schemas"]["CreateServerBody"];
export type Created = components["schemas"]["CreatedDto"];
export type ConsoleLine = components["schemas"]["ConsoleLineDto"];
export type ServerAction = "start" | "stop" | "restart" | "kill";
export type Property = components["schemas"]["PropertyDto"];
export type ServerPatch = components["schemas"]["PatchServerBody"];
export type Trashed = components["schemas"]["TrashedDto"];
type ErrorBody = components["schemas"]["ErrorBody"];

/** Lỗi từ API. Thông báo được dịch theo `code`, không lấy câu chữ từ backend. */
export class ApiError extends Error {
  readonly code: string;
  readonly params: Record<string, unknown>;
  readonly status: number;

  constructor(code: string, params: Record<string, unknown>, status: number) {
    super(code);
    this.code = code;
    this.params = params;
    this.status = status;
  }
}

async function request<T>(method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE", path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (body !== undefined) headers["Content-Type"] = "application/json";
  // Controller bắt request thay đổi dữ liệu phải có Origin của nó hoặc header này.
  if (method !== "GET") headers["X-MineHost"] = "1";

  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError("NETWORK", {}, 0);
  }

  if (!response.ok) {
    const error = (await response.json().catch(() => null)) as ErrorBody | null;
    throw new ApiError(error?.code ?? "INTERNAL", error?.params ?? {}, response.status);
  }
  // 202/204 của thao tác không trả dữ liệu thì thân rỗng.
  const text = await response.text();
  return (text === "" ? undefined : JSON.parse(text)) as T;
}

function serverPath(id: string): string {
  return `/servers/${encodeURIComponent(id)}`;
}

export const api = {
  health: () => request<components["schemas"]["Health"]>("GET", "/health"),
  system: () => request<SystemInfo>("GET", "/system"),
  /** Tắt an toàn mọi server rồi thoát MineHost. */
  shutdown: () => request<void>("POST", "/app/shutdown"),
  servers: () => request<Server[]>("GET", "/servers"),
  server: (id: string) => request<Server>("GET", serverPath(id)),
  createServer: (body: CreateServer) => request<Created>("POST", "/servers", body),
  action: (id: string, action: ServerAction) => request<void>("POST", `${serverPath(id)}/actions/${action}`),
  acceptEula: (id: string) => request<Server>("POST", `${serverPath(id)}/eula`, { accept: true }),
  sendCommand: (id: string, command: string) => request<void>("POST", `${serverPath(id)}/console`, { command }),
  updateServer: (id: string, patch: ServerPatch) => request<Server>("PATCH", serverPath(id), patch),
  trashServer: (id: string, confirmName: string) =>
    request<Trashed>("DELETE", `${serverPath(id)}?confirm_name=${encodeURIComponent(confirmName)}`),
  /** Chỉ số 1 giờ gần nhất; `since` (RFC 3339) có thì chỉ lấy mẫu sau thời điểm đó. */
  metrics: (id: string, since?: string) =>
    request<ServerMetrics>("GET", `${serverPath(id)}/metrics${since ? `?since=${encodeURIComponent(since)}` : ""}`),
  properties: (id: string) => request<{ properties: Property[] }>("GET", `${serverPath(id)}/properties`),
  saveProperties: (id: string, properties: Property[]) => request<void>("PUT", `${serverPath(id)}/properties`, { properties }),
  paperVersions: () => request<CatalogVersion[]>("GET", "/catalog/paper/versions"),
  /** Job đang giữ, mới nhất trước; `serverId` có thì chỉ Job của server đó. */
  jobs: (serverId?: string) =>
    request<Job[]>("GET", serverId ? `/jobs?server_id=${encodeURIComponent(serverId)}` : "/jobs"),
  job: (id: number) => request<Job>("GET", `/jobs/${id}`),
  cancelJob: (id: number) => request<void>("POST", `/jobs/${id}/cancel`),
};

/** Câu thông báo đã dịch cho một lỗi bất kỳ. */
export function errorMessage(t: TFunction, error: unknown): string {
  if (error instanceof ApiError) {
    return t(`errors.${error.code}`, { ...error.params, defaultValue: t("errors.INTERNAL") });
  }
  return t("errors.INTERNAL");
}
