import { request, serverPath } from "./client";
import type { components } from "./schema";

export type Player = components["schemas"]["PlayerDto"];
export type Players = components["schemas"]["PlayersDto"];
export type PlayerActionKind = components["schemas"]["PlayerActionKind"];
export type PlayerActionBody = components["schemas"]["PlayerActionBody"];

export function playersKey(serverId: string) {
  return ["players", serverId] as const;
}

export const playersApi = {
  list: (serverId: string) => request<Players>("GET", `${serverPath(serverId)}/players`),
  /** `sent`: đã gửi lệnh console; `saved`: server tắt, MineHost đã sửa file. */
  act: (serverId: string, name: string, body: PlayerActionBody) =>
    request<{ done: string }>("POST", `${serverPath(serverId)}/players/${encodeURIComponent(name)}/actions`, body),
  /** Skin qua MineHost (spec P2, mục 3.8). Người chưa có UUID thì không có ảnh. */
  skinUrl: (uuid: string | null | undefined) => (uuid ? `/api/v1/players/${encodeURIComponent(uuid)}/skin` : null),
};

/** Tên hợp lệ của Minecraft, giống kiểm tra ở backend. */
export const PLAYER_NAME = /^[A-Za-z0-9_]{3,16}$/;
