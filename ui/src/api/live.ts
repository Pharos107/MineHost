import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { api, type Job, type Server } from "./client";
import { live } from "./ws";

export const SERVERS_KEY = ["servers"] as const;

type ServerEvent = Server | { id: string; removed: true };

/** Ghép một tin của topic `servers` vào danh sách đang có; giữ thứ tự cũ, server mới thêm cuối. */
export function applyServerEvent(list: Server[] | undefined, event: ServerEvent): Server[] {
  const current = list ?? [];
  if ("removed" in event) return current.filter((server) => server.id !== event.id);
  const index = current.findIndex((server) => server.id === event.id);
  if (index === -1) return [...current, event];
  return current.map((server, i) => (i === index ? event : server));
}

/**
 * Giữ cache `["servers"]` (và `["servers", id]`) theo sự kiện WebSocket, để danh sách và
 * trang chi tiết đổi trạng thái ngay mà không phải hỏi lại REST. Gọi một lần ở gốc app.
 */
export function useServersLive() {
  const queryClient = useQueryClient();
  useEffect(
    () =>
      live.subscribe("servers", (message) => {
        if (message.error) return;
        const event = message.data as ServerEvent;
        queryClient.setQueryData<Server[]>(SERVERS_KEY, (list) => applyServerEvent(list, event));
        if ("removed" in event) queryClient.removeQueries({ queryKey: [...SERVERS_KEY, event.id] });
        else {
          queryClient.setQueryData([...SERVERS_KEY, event.id], event);
          void queryClient.invalidateQueries({ queryKey: [...SERVERS_KEY, event.id, "preflight"] });
        }
      }),
    [queryClient],
  );
}

/** Job theo id: lấy qua REST một lần rồi cập nhật theo topic `job.<id>` tới khi xong. */
export function useJob(id: number) {
  const queryClient = useQueryClient();
  const key = ["jobs", id];
  const query = useQuery({
    queryKey: key,
    // REST về sau tin WebSocket thì giữ tin WebSocket, vì nó mới hơn.
    queryFn: async () => {
      const job = await api.job(id);
      return queryClient.getQueryData<Job>(key) ?? job;
    },
    staleTime: Infinity,
  });
  useEffect(
    () =>
      live.subscribe(`job.${id}`, (message) => {
        if (!message.error) queryClient.setQueryData(["jobs", id], message.data as Job);
      }),
    [id, queryClient],
  );
  return query;
}
