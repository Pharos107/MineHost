import { describe, expect, it } from "vitest";

import { isModLoader, nameError, portError, toRequest, type WizardForm } from "./form";

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
  });

  it("để trống RAM và cổng thì backend tự chọn", () => {
    const form: WizardForm = { loader: "paper", version: "", name: " Survival ", memoryMb: null, port: "" };
    expect(toRequest(form, "1.21.4")).toEqual({ name: "Survival", loader: "paper", minecraft: "1.21.4", memory_mb: null, port: null });
    expect(toRequest({ ...form, memoryMb: 6144, port: "25570" }, "1.21.4")).toMatchObject({ memory_mb: { min: 1024, max: 6144 }, port: 25570 });
  });

  it("mod loader gửi loader_version khi người dùng chọn", () => {
    expect(isModLoader("fabric")).toBe(true);
    expect(isModLoader("paper")).toBe(false);
    const base: WizardForm = { loader: "fabric", version: "", name: "Modded", memoryMb: null, port: "" };
    expect(toRequest(base, "1.21.4")).toEqual({ name: "Modded", loader: "fabric", minecraft: "1.21.4", memory_mb: null, port: null });
    expect(toRequest({ ...base, loaderVersion: "0.16.9" }, "1.21.4")).toMatchObject({ loader: "fabric", loader_version: "0.16.9" });
  });
});
