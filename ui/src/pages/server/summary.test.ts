import { describe, expect, it } from "vitest";

import type { Server } from "../../api/client";
import i18n from "../../i18n";
import { serverSummary } from "./ServerLayout";

function server(patch: Partial<Server>): Server {
  return { id: "A", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false, ...patch };
}

describe("serverSummary", () => {
  const t = i18n.getFixedT("vi");

  it("ghép loader, phiên bản Minecraft và modpack", () => {
    const s = server({ loader: "fabric", loader_version: "0.16.14", minecraft: "1.20.1", modpack: { name: "Prominence II", version_name: "v4.1.8", source: "modrinth" } });
    expect(serverSummary(t, s)).toBe("Fabric 0.16.14 · Minecraft 1.20.1 · Prominence II v4.1.8");
  });

  it("Paper không có bản loader và modpack", () => {
    expect(serverSummary(t, server({ loader: "paper", minecraft: "1.21.4" }))).toBe("Paper · Minecraft 1.21.4");
  });

  it("server chưa có thông tin thì không hiện dòng nào", () => {
    expect(serverSummary(t, server({}))).toBeUndefined();
  });
});
