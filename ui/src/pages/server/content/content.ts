import type { TFunction } from "i18next";

import { ApiError, errorMessage, type Server } from "../../../api/client";
import type { ContentKind, ContentVersion, InstalledContent } from "../../../api/content";

export type ContentSourceLabel = "modrinth" | "curseforge" | "modpack" | "manual";

const LOADER_LABELS: Record<string, string> = {
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
  paper: "Paper",
  purpur: "Purpur",
  spigot: "Spigot",
  bukkit: "Bukkit",
};

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

/** Đổi `content_kind` của server sang `kind` của route tìm kiếm. */
export function searchKindFor(kind: ContentKind): "mod" | "plugin" {
  return kind === "plugins" ? "plugin" : "mod";
}

/** Lọc tên loader từ `categories` của Modrinth để hiện nhãn gọn trên `ProjectCard`. */
export function hitLoaders(categories: string[], fallbackLoader?: string | null): string[] {
  const labels = categories
    .map((c) => LOADER_LABELS[c.toLowerCase()])
    .filter((label): label is string => Boolean(label));
  if (labels.length > 0) return labels;
  if (fallbackLoader) return [LOADER_LABELS[fallbackLoader.toLowerCase()] ?? fallbackLoader];
  return [];
}

/** Ưu tiên phiên bản Minecraft của server khi khớp, nếu không lấy bản đầu tiên. */
export function hitVersion(gameVersions: string[], serverMinecraft?: string | null): string | undefined {
  if (serverMinecraft && gameVersions.includes(serverMinecraft)) return serverMinecraft;
  return gameVersions[0];
}

export function versionOptionLabel(
  version: Pick<ContentVersion, "version_number" | "version_type"> & { name?: string },
): string {
  const num = version.version_number ?? version.name ?? "";
  if (version.version_type === "release") return num;
  return `${num} (${version.version_type})`;
}
