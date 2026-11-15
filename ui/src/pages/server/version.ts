import type { CatalogVersion } from "../../api/client";

/** Loại server có mod loader: đổi được bản loader ngoài bản Minecraft. */
export const MOD_LOADERS = ["fabric", "quilt", "forge", "neoforge"];

/**
 * Bản `target` cũ hơn bản đang cài theo danh sách của catalog (mới nhất trước). Không tìm thấy một
 * trong hai thì không coi là hạ phiên bản: catalog là nguồn duy nhất biết thứ tự (spec P1, mục 3.5).
 */
export function isDowngrade(versions: Pick<CatalogVersion, "id">[], current: string | null | undefined, target: string): boolean {
  if (!current || current === target) return false;
  const from = versions.findIndex((v) => v.id === current);
  const to = versions.findIndex((v) => v.id === target);
  return from >= 0 && to > from;
}

export interface VersionRequest {
  minecraft?: string;
  loader_version?: string;
  confirm: true;
}

/** Body gửi `POST /servers/{id}/version`. Giữ bản Minecraft cũ thì bỏ trống để backend lấy build mới nhất. */
export function versionRequest(current: string | null | undefined, minecraft: string, loaderVersion: string): VersionRequest {
  const body: VersionRequest = { confirm: true };
  if (minecraft !== current) body.minecraft = minecraft;
  if (loaderVersion) body.loader_version = loaderVersion;
  return body;
}

/** Kết quả Job `change_version`. */
export interface VersionChangeResult {
  changed: boolean;
  minecraft: string;
  build?: number | null;
  loader_version?: string | null;
  backup_id?: string | null;
}
