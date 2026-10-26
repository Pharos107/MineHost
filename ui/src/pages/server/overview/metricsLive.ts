import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { api, type MetricSample, type ServerMetrics } from "../../../api/client";
import { live } from "../../../api/ws";

/** Bằng vòng đệm phía backend: 1 giờ, mỗi 5 giây một mẫu (spec P2, mục 3.5). */
export const MAX_SAMPLES = 720;

/** Dung lượng thư mục và thời điểm bật đổi chậm, nên chỉ hỏi lại REST thưa. */
const REFRESH_MS = 60_000;

export function metricsKey(id: string) {
  return ["metrics", id] as const;
}

/** Thêm một mẫu mới, bỏ mẫu cũ nhất khi quá `MAX_SAMPLES`. */
export function appendSample(metrics: ServerMetrics | undefined, sample: MetricSample): ServerMetrics | undefined {
  if (!metrics) return metrics;
  const samples = [...metrics.samples, sample].slice(-MAX_SAMPLES);
  return { ...metrics, samples };
}

/**
 * Chỉ số của một server: lấy 1 giờ gần nhất qua REST rồi nhận từng mẫu qua topic
 * `server.<id>.metrics`. `state` đổi (bật, tắt) thì hỏi lại để có `running_since` mới.
 */
export function useServerMetrics(id: string, state: string) {
  const queryClient = useQueryClient();
  const query = useQuery({ queryKey: metricsKey(id), queryFn: () => api.metrics(id), refetchInterval: REFRESH_MS });

  useEffect(
    () =>
      live.subscribe(`server.${id}.metrics`, (message) => {
        if (message.error) return;
        queryClient.setQueryData<ServerMetrics>(metricsKey(id), (m) => appendSample(m, message.data as MetricSample));
      }),
    [id, queryClient],
  );

  useEffect(() => {
    void queryClient.invalidateQueries({ queryKey: metricsKey(id) });
  }, [id, state, queryClient]);

  return query;
}
