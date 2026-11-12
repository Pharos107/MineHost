import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { playersApi, playersKey, type Players } from "../../../api/players";
import { live } from "../../../api/ws";

/**
 * Người chơi của một server: tải qua REST, rồi topic `server.<id>.players` gửi lại cả danh sách
 * mỗi lần có người vào, ra, hoặc file whitelist / op / cấm đổi.
 */
export function usePlayers(serverId: string, state: string) {
  const queryClient = useQueryClient();
  const query = useQuery({ queryKey: playersKey(serverId), queryFn: () => playersApi.list(serverId) });

  useEffect(
    () =>
      live.subscribe(`server.${serverId}.players`, (message) => {
        if (message.error) return;
        queryClient.setQueryData<Players>(playersKey(serverId), message.data as Players);
      }),
    [serverId, queryClient],
  );

  // Bật, tắt server đổi `running` và những thao tác làm được.
  useEffect(() => {
    void queryClient.invalidateQueries({ queryKey: playersKey(serverId) });
  }, [serverId, state, queryClient]);

  return query;
}
