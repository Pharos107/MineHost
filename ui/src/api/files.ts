import { ApiError, mutationHeaders, request, serverPath, type Trashed } from "./client";
import type { components } from "./schema";

export type FileEntry = components["schemas"]["FileEntryDto"];
export type TextFile = components["schemas"]["TextFileDto"];
export type FoundEntry = components["schemas"]["FoundEntryDto"];
export type SearchResult = components["schemas"]["FileSearchDto"];
export type DirSize = components["schemas"]["DirSizeDto"];
export type TextPreview = components["schemas"]["TextPreviewDto"];
export type NbtPreview = components["schemas"]["NbtPreviewDto"];
export type TrashBatch = components["schemas"]["TrashBatchDto"];

export interface UploadItem {
  file: File;
  path?: string;
}

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
  copy: (id: string, from: string, to: string) => request<void>("POST", `${serverPath(id)}/files/copy`, { from, to }),
  size: (id: string, path: string) => request<DirSize>("GET", filesPath(id, "/size", path)),
  preview: (id: string, path: string) => request<TextPreview>("GET", filesPath(id, "/preview", path)),
  nbt: (id: string, path: string) => request<NbtPreview>("GET", filesPath(id, "/nbt", path)),
  rawUrl: (id: string, path: string) => `/api/v1${filesPath(id, "/raw", path)}`,
  fetch: (id: string, dir: string, url: string, filename?: string) =>
    request<{ job_id: number }>("POST", `${serverPath(id)}/files/fetch`, { dir, url, filename }),
  trashList: (id: string) => request<TrashBatch[]>("GET", `${serverPath(id)}/files/trash`),
  trashRestore: (id: string, batch: string) => request<string[]>("POST", `${serverPath(id)}/files/trash/restore`, { batch }),
  trashEmpty: (id: string, batch?: string) =>
    request<void>("DELETE", batch ? `${serverPath(id)}/files/trash?batch=${encodeURIComponent(batch)}` : `${serverPath(id)}/files/trash`),
};

const UPLOAD_BATCH = 50;

/**
 * Tải file (hoặc cả thư mục giữ đường dẫn con) lên thư mục `dir`. Dùng `XMLHttpRequest` vì cần
 * tiến độ gửi đi cho file lớn (tới 512 MiB).
 */
export async function uploadFiles(
  id: string,
  dir: string,
  files: Array<File | UploadItem>,
  onProgress: (sent: number, total: number) => void,
): Promise<string[]> {
  const normalized = files.map((item) => (item instanceof File ? { file: item, path: item.webkitRelativePath || item.name } : { file: item.file, path: item.path ?? item.file.name }));
  const relative = normalized.some((item) => item.path.includes("/"));
  const totalBytes = normalized.reduce((sum, item) => sum + item.file.size, 0);
  let sentBefore = 0;
  const uploaded: string[] = [];
  for (let i = 0; i < normalized.length; i += UPLOAD_BATCH) {
    const batch = normalized.slice(i, i + UPLOAD_BATCH);
    const batchBytes = batch.reduce((sum, item) => sum + item.file.size, 0);
    const names = await sendUploadBatch(id, dir, batch, relative, (sent) => onProgress(sentBefore + sent, totalBytes));
    sentBefore += batchBytes;
    uploaded.push(...names);
  }
  return uploaded;
}

function sendUploadBatch(
  id: string,
  dir: string,
  batch: Array<{ file: File; path: string }>,
  relative: boolean,
  onProgress: (sent: number) => void,
): Promise<string[]> {
  const form = new FormData();
  for (const item of batch) {
    if (relative) form.append("path", item.path);
    form.append("file", item.file, item.file.name);
  }
  const url = relative ? `/api/v1${filesPath(id, "/upload", dir)}&relative=true` : `/api/v1${filesPath(id, "/upload", dir)}`;
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", url);
    for (const [name, value] of Object.entries(mutationHeaders())) xhr.setRequestHeader(name, value);
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) onProgress(e.loaded);
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
