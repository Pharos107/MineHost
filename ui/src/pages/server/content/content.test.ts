import { describe, expect, it } from "vitest";

import { ApiError } from "../../../api/client";
import i18n from "../../../i18n";
import {
  contentSourceKind,
  contentUploadError,
  hitLoaders,
  hitVersion,
  isContentLocked,
  isJarFile,
  modrinthProjectUrl,
  searchKindFor,
  versionOptionLabel,
} from "./content";

describe("content helpers", () => {
  it("chọn đúng nhãn nguồn từ bản ghi", () => {
    expect(contentSourceKind({ manual: true, source: null, from_modpack: false })).toBe("manual");
    expect(contentSourceKind({ manual: false, source: "upload", from_modpack: false })).toBe("manual");
    expect(contentSourceKind({ manual: false, source: null, from_modpack: false })).toBe("manual");
    expect(contentSourceKind({ manual: false, source: "modpack", from_modpack: true })).toBe("modpack");
    expect(contentSourceKind({ manual: false, source: "modrinth", from_modpack: true })).toBe("modpack");
    expect(contentSourceKind({ manual: false, source: "curseforge", from_modpack: false })).toBe("curseforge");
    expect(contentSourceKind({ manual: false, source: "modrinth", from_modpack: false })).toBe("modrinth");
  });

  it("tạo link Modrinth khi có project_id thuộc nguồn modrinth", () => {
    expect(modrinthProjectUrl({ source: "modrinth", project_id: "AANobbMI" })).toBe("https://modrinth.com/project/AANobbMI");
    expect(modrinthProjectUrl({ source: "modrinth", project_id: null })).toBeNull();
    expect(modrinthProjectUrl({ source: "curseforge", project_id: "238222" })).toBeNull();
    expect(modrinthProjectUrl({ source: "upload", project_id: "AANobbMI" })).toBeNull();
  });

  it("khóa thao tác khi server đang chạy, đang đổi trạng thái hoặc chờ khởi động lại", () => {
    expect(isContentLocked({ state: "stopped", restart_pending: false })).toBe(false);
    expect(isContentLocked({ state: "crashed", restart_pending: false })).toBe(false);
    expect(isContentLocked({ state: "stopped", restart_pending: true })).toBe(true);
    expect(isContentLocked({ state: "starting", restart_pending: false })).toBe(true);
    expect(isContentLocked({ state: "running", restart_pending: false })).toBe(true);
    expect(isContentLocked({ state: "stopping", restart_pending: false })).toBe(true);
    expect(isContentLocked({ state: "installing", restart_pending: false })).toBe(true);
  });

  it("kiểm tra đuôi .jar không phân biệt hoa thường", () => {
    expect(isJarFile({ name: "sodium.jar" })).toBe(true);
    expect(isJarFile({ name: "Lithium.JAR" })).toBe(true);
    expect(isJarFile({ name: "pack.mrpack" })).toBe(false);
    expect(isJarFile({ name: "sodium.jar.disabled" })).toBe(false);
  });

  it("dịch lỗi tải lên khi file hỏng hoặc trùng", () => {
    const t = i18n.t.bind(i18n);
    expect(contentUploadError(t, new ApiError("INVALID_INPUT", {}, 400))).toBe("File không phải .jar hợp lệ hoặc bị hỏng.");
    expect(contentUploadError(t, new ApiError("FILE_EXISTS", {}, 409))).toBe("Đã có file hoặc thư mục trùng tên.");
  });

  it("trích loader, phiên bản và kind cho tìm kiếm", () => {
    expect(searchKindFor("mods")).toBe("mod");
    expect(searchKindFor("plugins")).toBe("plugin");
    expect(hitLoaders(["optimization", "fabric", "quilt"], "fabric")).toEqual(["Fabric", "Quilt"]);
    expect(hitLoaders(["optimization"], "neoforge")).toEqual(["NeoForge"]);
    expect(hitVersion(["1.20.4", "1.21.4"], "1.21.4")).toBe("1.21.4");
    expect(hitVersion(["1.21.1", "1.20.4"], "1.21.4")).toBe("1.21.1");
    expect(versionOptionLabel({ version_number: "0.14.2", version_type: "release" })).toBe("0.14.2");
    expect(versionOptionLabel({ version_number: "0.15.0-beta.1", version_type: "beta" })).toBe("0.15.0-beta.1 (beta)");
  });
});
