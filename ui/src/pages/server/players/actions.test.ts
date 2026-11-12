import { describe, expect, it } from "vitest";

import type { Player } from "../../../api/players";
import { playerActions, teleportTargets } from "./actions";

function player(over: Partial<Player> = {}): Player {
  return { name: "Steve", uuid: "u", online: true, last_seen: null, whitelisted: false, op: false, banned: false, ban_reason: null, frozen: false, ...over };
}

describe("playerActions", () => {
  it("server tắt: chỉ quyền và cấm bấm được", () => {
    const groups = playerActions(player(), false, ["Alex"]);
    const all = [...groups.permissions, ...groups.discipline, ...groups.interact, ...groups.message];
    const usable = all.filter((a) => a.blocked === null).map((a) => a.kind);
    expect(usable).toEqual(["whitelist_add", "op", "ban"]);
    expect(all.filter((a) => a.blocked !== null).every((a) => a.blocked === "stopped")).toBe(true);
  });

  it("đổi nút theo trạng thái hiện tại của người chơi", () => {
    const groups = playerActions(player({ whitelisted: true, op: true, banned: true, frozen: true }), true, ["Alex"]);
    expect(groups.permissions.map((a) => a.kind)).toEqual(["whitelist_remove", "deop"]);
    expect(groups.discipline.map((a) => a.kind)).toEqual(["kick", "pardon"]);
    expect(groups.interact.map((a) => a.kind)).toEqual(["unfreeze", "teleport_to_spawn", "teleport_to_player"]);
    expect([...groups.interact, ...groups.message].every((a) => a.blocked === null)).toBe(true);
  });

  it("người không online, hoặc không có ai để dịch chuyển tới", () => {
    expect(playerActions(player({ online: false }), true, ["Alex"]).interact.every((a) => a.blocked === "offline")).toBe(true);
    const alone = playerActions(player(), true, []);
    expect(alone.interact.map((a) => a.blocked)).toEqual([null, null, "noTarget"]);
  });

  it("đích dịch chuyển là người khác đang online", () => {
    const list = [player(), player({ name: "Alex" }), player({ name: "Bob", online: false })];
    expect(teleportTargets(list, "steve")).toEqual(["Alex"]);
  });
});

/**
 * Mã nguồn mọi file `.ts`, `.tsx` (trừ test) trong `src/`, khóa là đường dẫn tính từ `src/`.
 * Bỏ file test vì chúng nhắc tới tên module trong chuỗi, không phải import thật.
 */
function sources(): [string, string][] {
  const raw = import.meta.glob<string>(["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}"], { query: "?raw", import: "default", eager: true });
  return Object.entries(raw).map(([path, text]) => [path.replace(/^\/src\//, ""), text]);
}

// Kế hoạch PR-19: skinview3d (kèm three.js) không được nằm trong bundle chính. Vite tách chunk theo
// `import()`, nên chỉ cần mọi chỗ khác không import tĩnh thư viện hay file bọc nó.
describe("skinview3d tải lười", () => {
  const files = sources();

  it("chỉ SkinViewer.tsx import thư viện", () => {
    const importers = files.filter(([, text]) => /from\s+["']skinview3d["']/.test(text)).map(([path]) => path);
    expect(importers).toEqual(["pages/server/players/SkinViewer.tsx"]);
  });

  it("SkinViewer chỉ được nạp bằng import()", () => {
    const staticImports = files.filter(([, text]) => /from\s+["']\.\/SkinViewer["']/.test(text)).map(([path]) => path);
    expect(staticImports).toEqual([]);
    const lazy = files.filter(([, text]) => text.includes('import("./SkinViewer")')).map(([path]) => path);
    expect(lazy).toEqual(["pages/server/players/PlayerDialog.tsx"]);
  });
});
