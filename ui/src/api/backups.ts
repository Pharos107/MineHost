import { request, serverPath } from "./client";
import type { components } from "./schema";

export type Backup = components["schemas"]["BackupDto"];
export type BackupSettings = components["schemas"]["BackupSettingsDto"];
type JobStarted = components["schemas"]["JobStartedDto"];

export function backupsKey(serverId: string) {
  return ["backups", serverId] as const;
}

function backupPath(serverId: string, backupId: string): string {
  return `${serverPath(serverId)}/backups/${encodeURIComponent(backupId)}`;
}

export const backupsApi = {
  list: (serverId: string) => request<Backup[]>("GET", `${serverPath(serverId)}/backups`),
  create: (serverId: string, note: string, isProtected: boolean) =>
    request<JobStarted>("POST", `${serverPath(serverId)}/backups`, { note: note.trim() || null, protected: isProtected }),
  /** `note` là chuỗi rỗng thì xóa ghi chú. */
  update: (serverId: string, backupId: string, patch: { note?: string; protected?: boolean }) =>
    request<Backup>("PATCH", backupPath(serverId, backupId), patch),
  remove: (serverId: string, backupId: string) => request<void>("DELETE", backupPath(serverId, backupId)),
  restore: (serverId: string, backupId: string) => request<JobStarted>("POST", `${backupPath(serverId, backupId)}/restore`, { confirm: true }),
  downloadUrl: (serverId: string, backupId: string) => `/api/v1${backupPath(serverId, backupId)}/download`,
};
