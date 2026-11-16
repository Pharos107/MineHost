import { describe, expect, it } from "vitest";

import {
  contentKindForLoader,
  DEFAULT_GAME_SETTINGS,
  hasAdvancedChanges,
  hasGameSettingsError,
  isModLoader,
  isPluginLoader,
  maxPlayersError,
  nameError,
  portError,
  toInitialProperties,
  toRequest,
  viewDistanceError,
  type WizardForm,
} from "./form";

describe("form wizard", () => {
  it("tên 1–64 ký tự sau khi bỏ khoảng trắng, đếm ký tự có dấu là một", () => {
    expect(nameError("Server của tôi")).toBeUndefined();
    expect(nameError("ư".repeat(64))).toBeUndefined();
    expect(nameError("ư".repeat(65))).toBe("wizard.nameInvalid");
    expect(nameError("   ")).toBe("wizard.nameInvalid");
  });

  it("cổng để trống hoặc là số từ 1024 đến 65535", () => {
    expect(portError("")).toBeUndefined();
    expect(portError("25565")).toBeUndefined();
    expect(portError("80")).toBe("wizard.portInvalid");
    expect(portError("70000")).toBe("wizard.portInvalid");
    expect(portError("25565.5")).toBe("wizard.portInvalid");
    expect(portError("1e4")).toBe("wizard.portInvalid");
  });

  it("để trống RAM và cổng thì backend tự chọn", () => {
    const form: WizardForm = { loader: "paper", version: "", name: " Survival ", memoryMb: null, port: "" };
    expect(toRequest(form, "1.21.4")).toEqual({ name: "Survival", loader: "paper", minecraft: "1.21.4", memory_mb: null, port: null });
    expect(toRequest({ ...form, memoryMb: 6144, port: "25570" }, "1.21.4")).toMatchObject({ memory_mb: { min: 1024, max: 6144 }, port: 25570 });
  });

  it("mod loader gửi loader_version khi người dùng chọn", () => {
    expect(isModLoader("fabric")).toBe(true);
    expect(isModLoader("paper")).toBe(false);
    expect(isPluginLoader("paper")).toBe(true);
    expect(isPluginLoader("purpur")).toBe(true);
    expect(isPluginLoader("vanilla")).toBe(false);
    expect(contentKindForLoader("paper")).toBe("plugin");
    expect(contentKindForLoader("fabric")).toBe("mod");
    expect(contentKindForLoader("vanilla")).toBeNull();
    const base: WizardForm = { loader: "fabric", version: "", name: "Modded", memoryMb: null, port: "" };
    expect(toRequest(base, "1.21.4")).toEqual({ name: "Modded", loader: "fabric", minecraft: "1.21.4", memory_mb: null, port: null });
    expect(toRequest({ ...base, loaderVersion: "0.16.9" }, "1.21.4")).toMatchObject({ loader: "fabric", loader_version: "0.16.9" });
  });

  it("chỉ gửi properties khác mặc định và kiểm tra giới hạn số", () => {
    expect(toInitialProperties(DEFAULT_GAME_SETTINGS)).toEqual([]);
    const custom = {
      ...DEFAULT_GAME_SETTINGS,
      gamemode: "creative",
      difficulty: "hard",
      onlineMode: false,
      whiteList: true,
      levelSeed: " 12345 ",
    };
    expect(toInitialProperties(custom)).toEqual([
      { key: "gamemode", value: "creative" },
      { key: "difficulty", value: "hard" },
      { key: "online-mode", value: "false" },
      { key: "white-list", value: "true" },
      { key: "level-seed", value: "12345" },
    ]);
    expect(hasGameSettingsError(DEFAULT_GAME_SETTINGS)).toBe(false);
    expect(hasGameSettingsError({ ...DEFAULT_GAME_SETTINGS, viewDistance: "1" })).toBe(true);
    expect(hasGameSettingsError({ ...DEFAULT_GAME_SETTINGS, maxPlayers: "0" })).toBe(true);
    expect(hasGameSettingsError({ ...DEFAULT_GAME_SETTINGS, spawnProtection: "-1" })).toBe(true);
    expect(maxPlayersError("-0")).toBe("wizard.maxPlayersInvalid");
    expect(viewDistanceError("1e1")).toBe("wizard.viewDistanceInvalid");
    expect(toInitialProperties({ ...DEFAULT_GAME_SETTINGS, maxPlayers: "020", viewDistance: "08" })).toEqual([
      { key: "view-distance", value: "8" },
    ]);
    expect(hasAdvancedChanges("", DEFAULT_GAME_SETTINGS)).toBe(false);
    expect(hasAdvancedChanges("25570", DEFAULT_GAME_SETTINGS)).toBe(true);
    expect(hasAdvancedChanges("", { ...DEFAULT_GAME_SETTINGS, allowFlight: true })).toBe(true);
  });
});
