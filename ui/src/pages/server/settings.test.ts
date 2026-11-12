import { describe, expect, it } from "vitest";

import type { Server } from "../../api/client";
import { getProperty, sameProperties, setProperty } from "./properties";
import { settingsErrors, settingsFrom, settingsPatch } from "./settings";

const server: Server = {
  id: "01JB",
  name: "Survival",
  state: "stopped",
  memory_mb: { min: 1024, max: 4096 },
  port: 25565,
  autostart: false,
  restart_on_crash: true,
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
};

describe("cài đặt server", () => {
  it("không đổi gì thì không gửi gì", () => {
    expect(settingsPatch(server, settingsFrom(server))).toEqual({});
  });

  it("chỉ gửi những ô đã đổi", () => {
    const form = { ...settingsFrom(server), name: " Creative ", memoryMb: 6144, autostart: true };
    expect(settingsPatch(server, form)).toEqual({ name: "Creative", memory_mb: { min: 1024, max: 6144 }, autostart: true });
  });

  it("lịch khởi động lại: chỉ gửi khi đổi, bắt giờ sai và giờ trùng", () => {
    const withTimes: Server = { ...server, restart_at: ["04:00", "16:00"] };
    // Đổi thứ tự không phải là đổi lịch: backend luôn sắp xếp.
    expect(settingsPatch(withTimes, { ...settingsFrom(withTimes), restartAt: ["16:00", "04:00"] })).toEqual({});
    expect(settingsPatch(withTimes, { ...settingsFrom(withTimes), restartAt: [] })).toEqual({ restart_at: [] });
    expect(settingsPatch(server, { ...settingsFrom(server), restartAt: ["05:30"] })).toEqual({ restart_at: ["05:30"] });
    expect(settingsErrors({ ...settingsFrom(server), restartAt: [""] }).restartAt).toBe("settings.restartAtInvalid");
    expect(settingsErrors({ ...settingsFrom(server), restartAt: ["04:00", "04:00"] }).restartAt).toBe("settings.restartAtDuplicate");
    expect(settingsErrors({ ...settingsFrom(server), restartAt: ["23:59"] }).restartAt).toBeUndefined();
  });

  it("cổng bắt buộc và phải hợp lệ", () => {
    expect(settingsErrors({ ...settingsFrom(server), port: "" }).port).toBe("wizard.portInvalid");
    expect(settingsErrors({ ...settingsFrom(server), port: "25570" }).port).toBeUndefined();
    expect(settingsPatch(server, { ...settingsFrom(server), port: "25570" })).toEqual({ port: 25570 });
  });
});

describe("server.properties", () => {
  const list = [
    { key: "motd", value: "A" },
    { key: "pvp", value: "true" },
  ];

  it("đổi giá trị giữ nguyên thứ tự, khóa mới thêm cuối", () => {
    expect(setProperty(list, "motd", "B").map((p) => p.key)).toEqual(["motd", "pvp"]);
    expect(getProperty(setProperty(list, "motd", "B"), "motd")).toBe("B");
    expect(setProperty(list, "difficulty", "hard").map((p) => p.key)).toEqual(["motd", "pvp", "difficulty"]);
  });

  it("so sánh theo cả khóa, giá trị và thứ tự", () => {
    expect(sameProperties(list, [...list])).toBe(true);
    expect(sameProperties(list, setProperty(list, "pvp", "false"))).toBe(false);
    expect(sameProperties(list, [...list].reverse())).toBe(false);
  });
});
