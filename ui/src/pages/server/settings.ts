import type { Server, ServerPatch } from "../../api/client";
import { memoryFor } from "../../memory";
import { nameError, portError } from "../create/form";

/** Giá trị đang sửa trong tab Cài đặt server; `port` là chữ để gõ dở vẫn giữ được. */
export interface SettingsForm {
  name: string;
  memoryMb: number;
  port: string;
  autostart: boolean;
  restartOnCrash: boolean;
}

export function settingsFrom(server: Server): SettingsForm {
  return {
    name: server.name ?? "",
    memoryMb: server.memory_mb?.max ?? 1024,
    port: server.port == null ? "" : String(server.port),
    autostart: server.autostart ?? false,
    restartOnCrash: server.restart_on_crash ?? true,
  };
}

/** Khóa i18n của lỗi theo từng ô; ở đây cổng bắt buộc phải có. */
export function settingsErrors(form: SettingsForm): { name?: string; port?: string } {
  return { name: nameError(form.name), port: form.port.trim() === "" ? "wizard.portInvalid" : portError(form.port) };
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
  return patch;
}
