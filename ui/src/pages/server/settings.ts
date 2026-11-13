import type { Server, ServerPatch } from "../../api/client";
import { memoryFor } from "../../memory";
import { nameError, portError } from "../create/form";

/** Các phiên bản Java MineHost hỗ trợ chọn cho từng server. */
export const JAVA_MAJORS = [8, 11, 17, 21, 25] as const;

/** Java đề xuất theo quy ước phiên bản của Minecraft. */
export function recommendedJavaMajor(minecraft: string | null | undefined): number {
  if (!minecraft) return 21;
  const parts = (minecraft.split("-")[0] ?? "").split(".").map(Number);
  const first = parts[0] ?? 0;
  if (first >= 26) return 25;
  if (first === 1) {
    const minor = parts[1] ?? 0;
    const patch = parts[2] ?? 0;
    if (minor > 20 || (minor === 20 && patch >= 5)) return 21;
    if (minor >= 17) return 17;
    return 8;
  }
  return 21;
}

/** Giá trị đang sửa trong tab Cài đặt server; `port` là chữ để gõ dở vẫn giữ được. */
export interface SettingsForm {
  name: string;
  memoryMb: number;
  port: string;
  autostart: boolean;
  restartOnCrash: boolean;
  /** Giờ khởi động lại hằng ngày `HH:MM`; ô vừa thêm còn trống thì là chuỗi rỗng. */
  restartAt: string[];
  javaMajor: number;
}

/** Tối đa số mốc mỗi ngày, giống backend (spec P2, mục 4.1). */
export const MAX_RESTART_TIMES = 4;
const TIME = /^([01]\d|2[0-3]):[0-5]\d$/;

export function settingsFrom(server: Server): SettingsForm {
  return {
    name: server.name ?? "",
    memoryMb: server.memory_mb?.max ?? 1024,
    port: server.port == null ? "" : String(server.port),
    autostart: server.autostart ?? false,
    restartOnCrash: server.restart_on_crash ?? true,
    restartAt: server.restart_at ?? [],
    javaMajor: server.java_major ?? recommendedJavaMajor(server.minecraft),
  };
}

/** Khóa i18n của lỗi lịch khởi động lại, hoặc `undefined` khi hợp lệ. */
export function restartError(times: string[]): string | undefined {
  if (times.some((time) => !TIME.test(time))) return "settings.restartAtInvalid";
  if (new Set(times).size !== times.length) return "settings.restartAtDuplicate";
  return undefined;
}

function sameTimes(a: string[], b: string[]): boolean {
  const left = [...a].sort();
  const right = [...b].sort();
  return left.length === right.length && left.every((time, i) => time === right[i]);
}

/** Khóa i18n của lỗi theo từng ô; ở đây cổng bắt buộc phải có. */
export function settingsErrors(form: SettingsForm): { name?: string; port?: string; restartAt?: string } {
  return {
    name: nameError(form.name),
    port: form.port.trim() === "" ? "wizard.portInvalid" : portError(form.port),
    restartAt: restartError(form.restartAt),
  };
}

/** Chỉ gửi những gì đã đổi, để PATCH không ghi đè cài đặt mà người dùng không động tới. */
export function settingsPatch(server: Server, form: SettingsForm): ServerPatch {
  const before = settingsFrom(server);
  const patch: ServerPatch = {};
  if (form.name.trim() !== before.name) patch.name = form.name.trim();
  if (form.memoryMb !== before.memoryMb) patch.memory_mb = memoryFor(form.memoryMb);
  if (form.port.trim() !== before.port) patch.port = Number(form.port);
  if (form.autostart !== before.autostart) patch.autostart = form.autostart;
  if (form.restartOnCrash !== before.restartOnCrash) patch.restart_on_crash = form.restartOnCrash;
  if (!sameTimes(form.restartAt, before.restartAt)) patch.restart_at = form.restartAt;
  if (form.javaMajor !== before.javaMajor) patch.java_major = form.javaMajor;
  return patch;
}
