import type { TFunction } from "i18next";

/**
 * Thời gian chạy dễ đọc, hai đơn vị lớn nhất: "3 ngày 2 giờ", "2 giờ 14 phút", "45 giây".
 * Không cần chính xác tới giây khi đã chạy lâu.
 */
export function formatUptime(t: TFunction, seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const days = Math.floor(s / 86_400);
  const hours = Math.floor((s % 86_400) / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  if (days > 0) return join(t, ["days", days], ["hours", hours]);
  if (hours > 0) return join(t, ["hours", hours], ["minutes", minutes]);
  if (minutes > 0) return t("duration.minutes", { count: minutes });
  return t("duration.seconds", { count: s });
}

function join(t: TFunction, big: [string, number], small: [string, number]): string {
  const first = t(`duration.${big[0]}`, { count: big[1] });
  return small[1] > 0 ? `${first} ${t(`duration.${small[0]}`, { count: small[1] })}` : first;
}
