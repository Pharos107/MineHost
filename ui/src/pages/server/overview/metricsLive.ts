import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
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

/** Ghép mẫu realtime mới nhất vào dữ liệu khung dài (> 1h) để điểm cuối không bị trễ 60 giây. */
export function mergeLiveSample(base: ServerMetrics | undefined, liveData: ServerMetrics | undefined): ServerMetrics | undefined {
  if (!base) return liveData;
  const latest = liveData?.samples.at(-1);
  if (!latest) return base;
  const baseLast = base.samples.at(-1);
  if (baseLast && Date.parse(latest.at) <= Date.parse(baseLast.at)) return base;
  return { ...base, samples: [...base.samples, latest] };
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

/** Dữ liệu tổng hợp cho khung > 1 giờ trên thẻ Tổng quan. */
export function useWindowMetrics(id: string, windowMinutes: number, enabled: boolean) {
  return useQuery({
    queryKey: ["metrics", id, "window", windowMinutes],
    enabled,
    refetchInterval: REFRESH_MS,
    placeholderData: keepPreviousData,
    queryFn: () =>
      api.metrics(id, {
        since: new Date(Date.now() - windowMinutes * 60_000).toISOString(),
        maxPoints: 360,
      }),
  });
}

/** Dữ liệu cho hộp thoại Xem chi tiết theo khoảng `since..until` và số điểm tối đa. */
export function useDetailMetrics(
  id: string,
  sinceIso: string | undefined,
  untilIso: string | undefined,
  maxPoints: number,
  enabled: boolean,
  liveRefresh: boolean,
  keepPrev = true,
) {
  return useQuery({
    queryKey: ["metrics", id, "detail", sinceIso ?? "", untilIso ?? "", maxPoints],
    enabled,
    refetchInterval: liveRefresh ? REFRESH_MS : false,
    placeholderData: keepPrev ? keepPreviousData : undefined,
    queryFn: () => api.metrics(id, { since: sinceIso, until: untilIso, maxPoints }),
  });
}
