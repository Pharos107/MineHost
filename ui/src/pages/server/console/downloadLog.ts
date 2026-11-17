// Mã màu ANSI không cần trong file tải về.
// eslint-disable-next-line no-control-regex
const ANSI = /\x1b\[[0-9;]*[A-Za-z]/g;

/** Tên file kiểu `Prominence-II-console-2026-11-16-1052.txt`, giờ theo máy người dùng. */
export function logFileName(serverName: string, now: Date): string {
  const safe = serverName.replace(/[^\p{L}\p{N}_-]+/gu, "-").replace(/^-+|-+$/g, "") || "server";
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}`;
  return `${safe}-console-${stamp}.txt`;
}

export function logText(lines: string[]): string {
  return lines.map((line) => line.replace(ANSI, "")).join("\n") + "\n";
}

/** Lưu các dòng đang có ở trình duyệt thành file văn bản; không hỏi server. */
export function downloadLog(serverName: string, lines: string[]) {
  const url = URL.createObjectURL(new Blob([logText(lines)], { type: "text/plain;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = logFileName(serverName, new Date());
  link.click();
  URL.revokeObjectURL(url);
}
