/** Ô nhập của thẻ "Tự động". Backend lưu một chuỗi `off`, `every_<n>h` hoặc `daily@HH:MM`. */
export interface ScheduleForm {
  mode: "off" | "every" | "daily";
  hours: string;
  time: string;
  keep: string;
}

const EVERY = /^every_(\d+)h$/;
const DAILY = /^daily@(\d{2}:\d{2})$/;
const TIME = /^([01]\d|2[0-3]):[0-5]\d$/;

export function scheduleForm(schedule: string, keep: number): ScheduleForm {
  const every = EVERY.exec(schedule);
  if (every) return { mode: "every", hours: every[1] ?? "6", time: "04:00", keep: String(keep) };
  const daily = DAILY.exec(schedule);
  if (daily) return { mode: "daily", hours: "6", time: daily[1] ?? "04:00", keep: String(keep) };
  return { mode: "off", hours: "6", time: "04:00", keep: String(keep) };
}

export function scheduleString(form: ScheduleForm): string {
  if (form.mode === "every") return `every_${Number(form.hours)}h`;
  if (form.mode === "daily") return `daily@${form.time}`;
  return "off";
}

function inRange(raw: string, min: number, max: number): boolean {
  const value = Number(raw);
  return raw.trim() !== "" && Number.isInteger(value) && value >= min && value <= max;
}

/** Khóa i18n của lỗi từng ô, giống giới hạn của backend (spec P2, mục 4.1). */
export function scheduleErrors(form: ScheduleForm): { hours?: string; time?: string; keep?: string } {
  return {
    hours: form.mode === "every" && !inRange(form.hours, 1, 168) ? "backups.hoursInvalid" : undefined,
    time: form.mode === "daily" && !TIME.test(form.time) ? "backups.timeInvalid" : undefined,
    keep: inRange(form.keep, 1, 100) ? undefined : "backups.keepInvalid",
  };
}
