import type { Server } from "../api/client";

/** Nhóm trạng thái cho ô lọc của trang Máy chủ; gộp các trạng thái người dùng coi là một. */
export const STATE_FILTERS = ["all", "running", "stopped", "problem", "installing"] as const;
export type StateFilter = (typeof STATE_FILTERS)[number];

const GROUP: Record<Server["state"], Exclude<StateFilter, "all">> = {
  starting: "running",
  running: "running",
  stopping: "running",
  stopped: "stopped",
  crashed: "problem",
  broken: "problem",
  installing: "installing",
};

export function matchesState(server: Server, filter: StateFilter): boolean {
  return filter === "all" || GROUP[server.state] === filter;
}

/** Địa chỉ để vào chơi từ LAN; chưa biết IP của máy thì chỉ còn cổng. */
export function playAddress(server: Server, lanAddresses: string[] | undefined): string | null {
  if (server.port == null) return null;
  const ip = lanAddresses?.[0];
  return ip ? `${ip}:${server.port}` : `:${server.port}`;
}
