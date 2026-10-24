import type { TFunction } from "i18next";

import type { components } from "./schema";

// Kiểu dữ liệu sinh từ OpenAPI của backend (`npm run gen:api`), không tự viết tay.
export type SystemInfo = components["schemas"]["SystemDto"];
export type ServerState = components["schemas"]["StateDto"];
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

async function getJson<T>(path: string): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, { headers: { Accept: "application/json" } });
  } catch {
    throw new ApiError("NETWORK", {}, 0);
  }

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as ErrorBody | null;
    throw new ApiError(body?.code ?? "INTERNAL", body?.params ?? {}, response.status);
  }
  return (await response.json()) as T;
}

export const api = {
  system: () => getJson<SystemInfo>("/system"),
};

/** Câu thông báo đã dịch cho một lỗi bất kỳ. */
export function errorMessage(t: TFunction, error: unknown): string {
  if (error instanceof ApiError) {
    return t(`errors.${error.code}`, { ...error.params, defaultValue: t("errors.INTERNAL") });
  }
  return t("errors.INTERNAL");
}
