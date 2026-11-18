import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { filesApi, type FileEntry, type FoundEntry } from "../../../api/files";

/** Chờ người dùng ngừng gõ rồi mới hỏi server, để không gửi một request mỗi phím. */
const DEBOUNCE_MS = 300;

/** Kết quả tìm đổi thành hàng của bảng: tên là đường dẫn tính từ thư mục đang mở, ví dụ `Essentials/config.yml`. */
export function foundToEntry(dir: string, found: FoundEntry): FileEntry {
  const name = dir !== "" && found.path.startsWith(`${dir}/`) ? found.path.slice(dir.length + 1) : found.path;
  return { name, kind: found.kind, size: found.size, modified: found.modified };
}

/**
 * Tìm cả thư mục con ngay trong ô tìm của bảng (spec trang Tệp, mục 2). Mặc định tắt: khi tắt, ô tìm
 * chỉ lọc thư mục đang mở như mọi bảng khác. Khi bật và có chữ, `rows` là kết quả từ server.
 */
export function useDeepSearch(serverId: string, dir: string) {
  const [deep, setDeep] = useState(false);
  // Gắn từ khóa với thư mục đang mở: đổi thư mục tự xóa từ khóa cũ (spec nâng cấp trang Tệp).
  const [input, setInput] = useState({ dir, query: "" });
  const query = input.dir === dir ? input.query : "";
  const [settled, setSettled] = useState({ dir, value: "" });
  useEffect(() => {
    const trimmed = query.trim();
    const timer = window.setTimeout(() => setSettled({ dir, value: trimmed }), DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [dir, query]);
  const settledValue = settled.dir === dir ? settled.value : "";
  const active = deep && query.trim() !== "" && settledValue !== "";
  const search = useQuery({
    queryKey: ["files", serverId, "search", dir, settledValue],
    queryFn: () => filesApi.search(serverId, dir, settledValue),
    enabled: active,
  });
  const typing = deep && query.trim() !== "" && settledValue !== query.trim();
  return {
    deep,
    toggle: () => setDeep((value) => !value),
    query,
    setQuery: (next: string) => setInput({ dir, query: next }),
    active,
    searching: typing || (active && search.isFetching),
    rows: active && search.data ? search.data.items.map((found) => foundToEntry(dir, found)) : undefined,
    truncated: active && search.data?.truncated === true,
    error: active ? search.error : null,
  };
}
