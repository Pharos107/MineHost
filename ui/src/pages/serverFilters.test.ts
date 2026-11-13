import { describe, expect, it } from "vitest";

import type { Server } from "../api/client";
import { matchesState, playAddress } from "./serverFilters";

function server(patch: Partial<Server>): Server {
  return { id: "01JB", name: "Survival", state: "stopped", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, ...patch };
}

describe("lọc server theo trạng thái", () => {
  it("gộp đang bật, đang chạy, đang tắt thành Đang chạy", () => {
    for (const state of ["starting", "running", "stopping"] as const) {
      expect(matchesState(server({ state }), "running")).toBe(true);
    }
    expect(matchesState(server({ state: "stopped" }), "running")).toBe(false);
  });

  it("crash và hỏng đều là Có lỗi; Mọi trạng thái nhận hết", () => {
    expect(matchesState(server({ state: "crashed" }), "problem")).toBe(true);
    expect(matchesState(server({ state: "broken" }), "problem")).toBe(true);
    expect(matchesState(server({ state: "installing" }), "all")).toBe(true);
  });
});

describe("địa chỉ vào chơi", () => {
  it("ghép IP LAN đầu tiên với cổng", () => {
    expect(playAddress(server({ port: 25565 }), ["192.168.1.5", "10.0.0.2"])).toBe("192.168.1.5:25565");
  });

  it("chưa biết IP thì chỉ còn cổng, chưa có cổng thì không có địa chỉ", () => {
    expect(playAddress(server({ port: 25566 }), [])).toBe(":25566");
    expect(playAddress(server({ port: null }), ["192.168.1.5"])).toBeNull();
  });
});
