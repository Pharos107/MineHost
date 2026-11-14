import { describe, expect, it } from "vitest";

import type { Server } from "./client";
import { applyServerEvent } from "./live";

function server(id: string, state: Server["state"] = "stopped"): Server {
  return { id, state, eula_accepted: false, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false };
}

describe("applyServerEvent", () => {
  it("thêm server mới vào cuối", () => {
    expect(applyServerEvent([server("a")], server("b")).map((s) => s.id)).toEqual(["a", "b"]);
    expect(applyServerEvent(undefined, server("a"))).toHaveLength(1);
  });

  it("thay server đã có tại chỗ", () => {
    const list = applyServerEvent([server("a"), server("b")], server("a", "running"));
    expect(list.map((s) => `${s.id}:${s.state}`)).toEqual(["a:running", "b:stopped"]);
  });

  it("bỏ server đã xóa", () => {
    expect(applyServerEvent([server("a"), server("b")], { id: "a", removed: true }).map((s) => s.id)).toEqual(["b"]);
  });
});
