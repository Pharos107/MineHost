import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { api, type Job } from "../../api/client";
import { live } from "../../api/ws";

export const JOBS_KEY = ["jobs", "list"] as const;

/** Ghép một Job mới hoặc vừa đổi vào danh sách, giữ thứ tự mới nhất trước (id lớn hơn là tạo sau). */
export function upsertJob(list: Job[] | undefined, job: Job): Job[] {
  const rest = (list ?? []).filter((j) => j.id !== job.id);
  return [...rest, job].sort((a, b) => b.id - a.id);
}

/** Danh sách Job: lấy qua REST một lần rồi cập nhật theo topic `jobs` (spec P2, mục 6.3). */
export function useJobsLive() {
  const queryClient = useQueryClient();
  const query = useQuery({ queryKey: JOBS_KEY, queryFn: () => api.jobs() });
  useEffect(
    () =>
      live.subscribe("jobs", (message) => {
        if (message.error) return;
        queryClient.setQueryData<Job[]>(JOBS_KEY, (list) => upsertJob(list, message.data as Job));
      }),
    [queryClient],
  );
  return query;
}
