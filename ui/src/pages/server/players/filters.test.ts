import { describe, expect, it } from "vitest";

import type { Player } from "../../../api/players";
import { countPlayers, filterPlayers, relativeTime } from "./filters";

function player(name: string, flags: Partial<Player> = {}): Player {
  return { name, uuid: null, online: false, last_seen: null, whitelisted: false, op: false, banned: false, ban_reason: null, frozen: false, ...flags };
}

const list = [player("Steve", { online: true, op: true, whitelisted: true }), player("Alex", { whitelisted: true }), player("Griefer", { banned: true })];

describe("filterPlayers", () => {
  it("lọc theo nhóm, giữ thứ tự", () => {
    expect(filterPlayers(list, "all", "").map((p) => p.name)).toEqual(["Steve", "Alex", "Griefer"]);
    expect(filterPlayers(list, "op", "").map((p) => p.name)).toEqual(["Steve"]);
    expect(filterPlayers(list, "whitelisted", "").map((p) => p.name)).toEqual(["Steve", "Alex"]);
    expect(filterPlayers(list, "banned", "").map((p) => p.name)).toEqual(["Griefer"]);
  });

  it("tìm theo tên không phân biệt hoa thường, kết hợp với nhóm", () => {
    expect(filterPlayers(list, "all", " STE ").map((p) => p.name)).toEqual(["Steve"]);
    expect(filterPlayers(list, "banned", "ste")).toEqual([]);
  });
});

describe("countPlayers", () => {
  it("đếm từng nhóm", () => {
    expect(countPlayers(list)).toEqual({ online: 1, op: 1, whitelisted: 2, banned: 1 });
  });
});

describe("relativeTime", () => {
  const now = Date.parse("2026-11-17T12:00:00Z");

  it("đổi đơn vị theo khoảng cách", () => {
    expect(relativeTime("2026-11-17T11:59:40Z", "en", now)).toBe("this minute");
    expect(relativeTime("2026-11-17T11:55:00Z", "en", now)).toBe("5 minutes ago");
    expect(relativeTime("2026-11-17T09:00:00Z", "en", now)).toBe("3 hours ago");
    expect(relativeTime("2026-11-16T12:00:00Z", "en", now)).toBe("yesterday");
  });

  it("quá 30 ngày thì ghi ngày, chuỗi hỏng thì null", () => {
    expect(relativeTime("2026-08-01T12:00:00Z", "en", now)).toBe("Aug 1, 2026");
    expect(relativeTime("không phải ngày", "en", now)).toBeNull();
  });
});
