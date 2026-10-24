import { useQuery } from "@tanstack/react-query";
import { RotateCw } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, errorMessage } from "../api/client";
import { formatBytes } from "../format";
import { Button, Card, Facts, Notice, Spinner } from "../ui";

export function SystemCard() {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });

  if (system.isPending) {
    return (
      <Card>
        <Spinner label={t("system.loading")} />
      </Card>
    );
  }

  if (system.isError) {
    return (
      <Notice
        tone="danger"
        action={
          <Button size="sm" icon={RotateCw} onClick={() => void system.refetch()}>
            {t("common.retry")}
          </Button>
        }
      >
        {errorMessage(t, system.error)}
      </Notice>
    );
  }

  const info = system.data;
  const bytes = (value: number) => formatBytes(value, i18n.language);
  const disk = info.data_disk;

  return (
    <Card title={t("system.heading")}>
      <Facts
        items={[
          { label: t("system.os"), value: `${info.os_version ?? info.os} (${info.arch})` },
          { label: t("system.cpu"), value: t("system.cpuValue", { brand: info.cpu_brand, threads: info.cpu_threads }) },
          {
            label: t("system.memory"),
            value: t("system.freeOfTotal", {
              free: bytes(info.memory_available_bytes),
              total: bytes(info.memory_total_bytes),
            }),
          },
          {
            label: t("system.disk"),
            value: disk
              ? t("system.freeOfTotal", { free: bytes(disk.available_bytes), total: bytes(disk.total_bytes) })
              : t("system.unknown"),
          },
          { label: t("system.dataDir"), value: <code>{info.data_dir}</code> },
          { label: t("system.version"), value: info.version },
        ]}
      />
    </Card>
  );
}
