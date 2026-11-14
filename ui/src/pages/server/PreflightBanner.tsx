import { useMutation, useQuery } from "@tanstack/react-query";
import { Play } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Preflight, type Server } from "../../api/client";
import { Button, Notice, Stack } from "../../ui";

function hasAnyIssues(data: Preflight): boolean {
  return (
    data.missing_deps.length > 0 ||
    data.pending.length > 0 ||
    data.version_mismatches.length > 0 ||
    data.wrong_loader.length > 0 ||
    data.client_only.length > 0
  );
}

/** Băng cảnh báo ở đầu trang server khi thiếu dependency, chờ tải CurseForge hoặc có lưu ý mod. */
export function PreflightBanner({ server }: { server: Server }) {
  const { t } = useTranslation();
  const canStart = server.state === "stopped" || server.state === "crashed";
  const enabled = canStart && server.content_kind != null;

  const query = useQuery({
    queryKey: ["servers", server.id, "preflight"],
    queryFn: () => api.preflight(server.id),
    enabled,
  });

  const startAnyway = useMutation({
    mutationFn: () => api.action(server.id, "start", { ignore_preflight: true }),
  });

  if (!enabled || !query.data || !hasAnyIssues(query.data)) return null;

  const data = query.data;
  const blocking = data.missing_deps.length > 0 || data.pending.length > 0;

  return (
    <Stack gap={2}>
      <Notice
        tone={blocking ? "danger" : "warning"}
        title={blocking ? t("preflight.blockingTitle") : t("preflight.warningTitle")}
        action={
          blocking ? (
            <Button
              variant="secondary"
              icon={Play}
              loading={startAnyway.isPending}
              onClick={() => startAnyway.mutate()}
            >
              {t("preflight.startAnyway")}
            </Button>
          ) : undefined
        }
      >
        <ul>
          {data.missing_deps.map((d) => (
            <li key={`miss:${d.required_by}:${d.id}`}>
              {t(d.version_req && d.version_req !== "*" ? "preflight.missingItem" : "preflight.missingItemAny", {
                id: d.id,
                version: d.version_req,
                by: d.required_by,
              })}
            </li>
          ))}
          {data.pending.map((p) => (
            <li key={`pend:${p.path}`}>{t("preflight.pendingItem", { file: p.file_name })}</li>
          ))}
          {data.version_mismatches.map((m) => (
            <li key={`ver:${m.required_by}:${m.id}`}>
              {t("preflight.mismatchItem", {
                by: m.required_by,
                id: m.id,
                version: m.version_req,
                installed: m.installed_version,
              })}
            </li>
          ))}
          {data.wrong_loader.map((w) => (
            <li key={`loader:${w.file_name}`}>
              {t("preflight.wrongLoaderItem", { name: w.name, file: w.file_name })}
            </li>
          ))}
          {data.client_only.map((c) => (
            <li key={`client:${c.file_name}`}>
              {t("preflight.clientOnlyItem", { name: c.name })}
            </li>
          ))}
        </ul>
      </Notice>
      {startAnyway.isError && <Notice tone="danger">{errorMessage(t, startAnyway.error)}</Notice>}
    </Stack>
  );
}
