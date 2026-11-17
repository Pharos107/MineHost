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
  const [query, setQuery] = useState("");
  const [settled, setSettled] = useState("");
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(query.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query]);
  const active = deep && settled !== "";
  const search = useQuery({
    queryKey: ["files", serverId, "search", dir, settled],
    queryFn: () => filesApi.search(serverId, dir, settled),
    enabled: active,
  });
  return {
    deep,
    toggle: () => setDeep((value) => !value),
    setQuery,
    active,
    rows: active ? (search.data?.items ?? []).map((found) => foundToEntry(dir, found)) : undefined,
    pending: active && search.isPending,
    truncated: active && search.data?.truncated === true,
    error: active ? search.error : null,
  };
}
