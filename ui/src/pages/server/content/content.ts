import type { TFunction } from "i18next";

import { ApiError, errorMessage, type Server } from "../../../api/client";
import type { InstalledContent } from "../../../api/content";

export type ContentSourceLabel = "modrinth" | "curseforge" | "modpack" | "manual";

/**
 * Chọn nhãn nguồn hiển thị từ bản ghi: file chép tay hoặc tải lên hiện "Thêm tay", file từ
 * modpack ưu tiên hiện "Modpack", còn lại theo `source`.
 */
export function contentSourceKind(item: Pick<InstalledContent, "manual" | "source" | "from_modpack">): ContentSourceLabel {
  if (item.manual || item.source === "upload" || item.source == null) return "manual";
  if (item.from_modpack || item.source === "modpack") return "modpack";
  if (item.source === "curseforge") return "curseforge";
  return "modrinth";
}

/** Link trang dự án Modrinth khi bản ghi có `source = "modrinth"` và `project_id`. */
export function modrinthProjectUrl(item: Pick<InstalledContent, "source" | "project_id">): string | null {
  if (item.source !== "modrinth" || !item.project_id) return null;
  return `https://modrinth.com/project/${encodeURIComponent(item.project_id)}`;
}

/** Khóa mọi thao tác sửa file mod/plugin khi server không ở trạng thái nghỉ (khớp `ensure_content_idle`). */
export function isContentLocked(server: Pick<Server, "state" | "restart_pending">): boolean {
  return (server.state !== "stopped" && server.state !== "crashed") || server.restart_pending;
}

export function isJarFile(file: Pick<File, "name">): boolean {
  return file.name.toLowerCase().endsWith(".jar");
}

/** Thông báo lỗi tải lên .jar: tách `INVALID_INPUT` thành câu rõ nghĩa cho file hỏng/sai đuôi. */
export function contentUploadError(t: TFunction, error: unknown): string {
  if (error instanceof ApiError && error.code === "INVALID_INPUT") return t("content.uploadInvalid");
  return errorMessage(t, error);
}
