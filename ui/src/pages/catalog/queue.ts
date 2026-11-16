import { useCallback, useEffect, useState } from "react";

import type { ContentItem } from "../../api/content";

/** Một mục người dùng chọn cài, kèm tên và bản để hiện mà không phải hỏi lại catalog. */
export interface QueuedContent {
  source: string;
  project_id: string;
  version_id: string | null;
  title: string;
  version_label: string | null;
  icon_url?: string | null;
}

export interface ContentQueue {
  items: QueuedContent[];
  add: (item: QueuedContent) => void;
  remove: (source: string, projectId: string) => void;
  clear: () => void;
  has: (source: string, projectId: string) => boolean;
}

export const MAX_QUEUE = 50;

function isQueued(value: unknown): value is QueuedContent {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.source === "string" && typeof v.project_id === "string" && typeof v.title === "string";
}

function read(key: string | null): QueuedContent[] {
  if (key === null) return [];
  try {
    const parsed: unknown = JSON.parse(window.localStorage.getItem(key) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter(isQueued).slice(0, MAX_QUEUE) : [];
  } catch {
    return [];
  }
}

function write(key: string | null, items: QueuedContent[]) {
  if (key === null) return;
  try {
    if (items.length === 0) window.localStorage.removeItem(key);
    else window.localStorage.setItem(key, JSON.stringify(items));
  } catch {
    // Trình duyệt chặn lưu trữ: danh sách chỉ mất khi tải lại trang, vẫn dùng được.
  }
}

export function queueStorageKey(serverId: string): string {
  return `minehost.contentQueue.${serverId}`;
}

/**
 * Danh sách chờ cài (spec trình duyệt mod, mục 4, 5). `storageKey` có thì nhớ trên trình duyệt này
 * theo từng server; `null` thì chỉ giữ trong bộ nhớ (bước Mod khi tạo server).
 */
export function useContentQueue(storageKey: string | null): ContentQueue {
  const [items, setItems] = useState<QueuedContent[]>(() => read(storageKey));

  useEffect(() => {
    write(storageKey, items);
  }, [storageKey, items]);

  const add = useCallback((item: QueuedContent) => {
    setItems((cur) => {
      const rest = cur.filter((q) => !(q.source === item.source && q.project_id === item.project_id));
      return [...rest, item].slice(-MAX_QUEUE);
    });
  }, []);
  const remove = useCallback((source: string, projectId: string) => {
    setItems((cur) => cur.filter((q) => !(q.source === source && q.project_id === projectId)));
  }, []);
  const clear = useCallback(() => setItems([]), []);
  const has = useCallback(
    (source: string, projectId: string) => items.some((q) => q.source === source && q.project_id === projectId),
    [items],
  );

  return { items, add, remove, clear, has };
}

/** Body gửi lên API cài. */
export function queueItems(items: QueuedContent[]): ContentItem[] {
  return items.map((q) => ({ source: q.source, project_id: q.project_id, version_id: q.version_id }));
}
