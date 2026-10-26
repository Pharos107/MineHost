import type { Job, Server, SystemInfo } from "../../api/client";

export interface HomeStats {
  running: number;
  total: number;
  jobsRunning: number;
  /** `null` khi chưa có số (đang tải hoặc lỗi), để thẻ hiện "—" thay vì 0 gây hiểu lầm. */
  cpuPercent: number | null;
  memoryUsed: number | null;
  memoryTotal: number | null;
  diskFree: number | null;
}

const WITH_PROCESS: Server["state"][] = ["starting", "running", "stopping"];

/** Gom số liệu cho các thẻ của Trang chủ từ ba nguồn API. */
export function homeStats(servers: Server[], jobs: Job[], system: SystemInfo | undefined): HomeStats {
  return {
    running: servers.filter((s) => WITH_PROCESS.includes(s.state)).length,
    total: servers.length,
    jobsRunning: jobs.filter((j) => j.state === "running").length,
    cpuPercent: system ? system.cpu_percent : null,
    memoryUsed: system?.memory_used_bytes ?? null,
    memoryTotal: system?.memory_total_bytes ?? null,
    diskFree: system?.disk_free_bytes ?? null,
  };
}
