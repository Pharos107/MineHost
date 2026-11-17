import { ApiError, mutationHeaders, request, serverPath, type Trashed } from "./client";
import type { components } from "./schema";

export type FileEntry = components["schemas"]["FileEntryDto"];
export type TextFile = components["schemas"]["TextFileDto"];
export type FoundEntry = components["schemas"]["FoundEntryDto"];
export type SearchResult = components["schemas"]["FileSearchDto"];

function filesPath(id: string, sub: string, path: string): string {
  return `${serverPath(id)}/files${sub}?path=${encodeURIComponent(path)}`;
}

/** Route tệp (spec P2, mục 5.8). `path` là đường dẫn trong thư mục server, `""` là gốc. */
export const filesApi = {
  list: (id: string, path: string) => request<FileEntry[]>("GET", filesPath(id, "", path)),
  read: (id: string, path: string) => request<TextFile>("GET", filesPath(id, "/content", path)),
  /** Không có `etag` là tạo file mới; có thì chỉ ghi khi file trên đĩa chưa đổi (`FILE_CHANGED`). */
  write: (id: string, path: string, text: string, etag?: string) =>
    request<{ etag: string }>("PUT", filesPath(id, "/content", path), { text }, etag ? { "If-Match": `"${etag}"` } : {}),
  mkdir: (id: string, path: string) => request<void>("POST", `${serverPath(id)}/files/mkdir`, { path }),
  rename: (id: string, from: string, to: string) => request<void>("POST", `${serverPath(id)}/files/rename`, { from, to }),
  remove: (id: string, path: string) => request<Trashed>("DELETE", filesPath(id, "", path)),
  downloadUrl: (id: string, path: string) => `/api/v1${filesPath(id, "/download", path)}`,
  /** Tìm theo tên trong `path` và mọi thư mục con (spec trang Tệp, mục 3). */
  search: (id: string, path: string, q: string) =>
    request<SearchResult>("GET", `${filesPath(id, "/search", path)}&q=${encodeURIComponent(q)}`),
  /** Một zip gồm các mục đã chọn; thư mục cũng tải được theo cách này. */
  archiveUrl: (id: string, paths: string[]) => `/api/v1${serverPath(id)}/files/archive?paths=${encodeURIComponent(JSON.stringify(paths))}`,
  compress: (id: string, paths: string[], dest: string) => request<{ job_id: number }>("POST", `${serverPath(id)}/files/compress`, { paths, dest }),
  extract: (id: string, path: string, dest: string) => request<{ job_id: number }>("POST", `${serverPath(id)}/files/extract`, { path, dest }),
};

/**
 * Tải file lên thư mục `dir`. Dùng `XMLHttpRequest` thay `fetch` vì cần tiến độ gửi đi cho file lớn
 * (tới 512 MiB). Lỗi trả về đúng `ApiError` như các route khác.
 */
export function uploadFiles(id: string, dir: string, files: File[], onProgress: (sent: number, total: number) => void): Promise<string[]> {
  const form = new FormData();
  for (const file of files) form.append("file", file, file.name);
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", `/api/v1${filesPath(id, "/upload", dir)}`);
    for (const [name, value] of Object.entries(mutationHeaders())) xhr.setRequestHeader(name, value);
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) onProgress(e.loaded, e.total);
    };
    xhr.onload = () => {
      const body = parseJson(xhr.responseText);
      if (xhr.status >= 200 && xhr.status < 300) resolve((body as string[] | null) ?? []);
      else {
        const error = body as { code?: string; params?: Record<string, unknown> } | null;
        reject(new ApiError(error?.code ?? "INTERNAL", error?.params ?? {}, xhr.status));
      }
    };
    xhr.onerror = () => reject(new ApiError("NETWORK", {}, 0));
    xhr.send(form);
  });
}

function parseJson(text: string): unknown {
  try {
    return text === "" ? null : JSON.parse(text);
  } catch {
    return null;
  }
}

/** Ghép thư mục cha và tên, không để dấu `/` thừa ở đầu. */
export function joinPath(dir: string, name: string): string {
  return dir === "" ? name : `${dir}/${name}`;
}

/** Các bậc của đường dẫn cho breadcrumb: `"a/b"` → `[["a","a"], ["b","a/b"]]`. */
export function pathSteps(path: string): Array<{ name: string; path: string }> {
  const parts = path.split("/").filter(Boolean);
  return parts.map((name, i) => ({ name, path: parts.slice(0, i + 1).join("/") }));
}

/** Thư mục chứa `path`; `""` là gốc. */
export function parentOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut === -1 ? "" : path.slice(0, cut);
}

/** Giống giới hạn của backend: file lớn hơn thì chỉ tải xuống, không mở trình sửa. */
export const EDITABLE_MAX_BYTES = 1024 * 1024;
