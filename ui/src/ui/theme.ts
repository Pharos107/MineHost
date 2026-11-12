import { useCallback, useSyncExternalStore } from "react";

export type Theme = "light" | "dark";

const KEY = "minehost.theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

/** Lựa chọn đã lưu trên trình duyệt này; `null` là theo hệ điều hành. */
function savedTheme(): Theme | null {
  try {
    const value = localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : null;
  } catch {
    return null;
  }
}

/** Đặt `data-theme` trên thẻ html; `tokens.css` đọc thuộc tính này để chọn bảng màu. */
function apply(theme: Theme | null) {
  const root = document.documentElement;
  if (theme) root.dataset.theme = theme;
  else delete root.dataset.theme;
}

/** Gọi một lần trước khi vẽ trang, để không chớp màu sai khi tải. */
export function initTheme() {
  apply(savedTheme());
}

function systemTheme(): Theme {
  return typeof window.matchMedia === "function" && window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

/** Bảng màu đang hiện: lựa chọn của người dùng, không có thì theo hệ điều hành. */
function currentTheme(): Theme {
  const chosen = document.documentElement.dataset.theme;
  return chosen === "light" || chosen === "dark" ? chosen : systemTheme();
}

/** Báo khi bảng màu đổi: người dùng bấm nút, hoặc hệ điều hành đổi sáng/tối. */
export function subscribeTheme(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  const media = typeof window.matchMedia === "function" ? window.matchMedia(DARK_QUERY) : null;
  media?.addEventListener("change", onChange);
  return () => {
    observer.disconnect();
    media?.removeEventListener("change", onChange);
  };
}

/** Bảng màu đang hiện và hàm chuyển sang bảng còn lại (lưu trên trình duyệt này). */
export function useTheme(): [Theme, () => void] {
  const theme = useSyncExternalStore<Theme>(subscribeTheme, currentTheme, () => "light");
  const toggle = useCallback(() => {
    const next: Theme = currentTheme() === "dark" ? "light" : "dark";
    apply(next);
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // Không lưu được thì chỉ đổi cho lần mở này.
    }
  }, []);
  return [theme, toggle];
}
