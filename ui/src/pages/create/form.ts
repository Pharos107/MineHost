import type { CreateServer, Loader } from "../../api/client";
import { memoryFor } from "../../memory";

export const STANDARD_LOADERS: readonly Loader[] = ["paper", "purpur", "vanilla"];
/** Thứ tự theo spec P3 mục 6.1 và 11.7. */
export const MOD_LOADERS: readonly Loader[] = ["fabric", "neoforge", "forge", "quilt"];

export function isModLoader(loader: Loader): boolean {
  return (MOD_LOADERS as readonly string[]).includes(loader);
}

export interface WizardForm {
  loader: Loader;
  /** Rỗng: chưa chọn, dùng bản mới nhất. */
  version: string;
  /** Rỗng hoặc undefined: dùng bản đề xuất đầu danh sách. */
  loaderVersion?: string;
  name: string;
  /** `null`: dùng mức đề xuất của backend. */
  memoryMb: number | null;
  /** Rỗng: để backend tự chọn cổng trống. */
  port: string;
}

const NAME_MAX_CHARS = 64;
const PORT_MIN = 1024;
const PORT_MAX = 65535;

/** Khóa i18n của lỗi, hoặc `undefined` khi hợp lệ. Đếm theo ký tự như backend, không theo UTF-16. */
export function nameError(name: string): string | undefined {
  const length = [...name.trim()].length;
  return length === 0 || length > NAME_MAX_CHARS ? "wizard.nameInvalid" : undefined;
}

export function portError(port: string): string | undefined {
  if (port.trim() === "") return undefined;
  const value = Number(port);
  return Number.isInteger(value) && value >= PORT_MIN && value <= PORT_MAX ? undefined : "wizard.portInvalid";
}

export function toRequest(form: WizardForm, version: string): CreateServer {
  const req: CreateServer = {
    name: form.name.trim(),
    loader: form.loader,
    minecraft: version,
    memory_mb: form.memoryMb === null ? null : memoryFor(form.memoryMb),
    port: form.port.trim() === "" ? null : Number(form.port),
  };
  if (isModLoader(form.loader) && form.loaderVersion) {
    req.loader_version = form.loaderVersion;
  }
  return req;
}
