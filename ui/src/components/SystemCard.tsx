import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, errorMessage } from "../api/client";
import { formatBytes } from "../format";

export function SystemCard() {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });

  if (system.isPending) {
    return <section className="card">{t("system.loading")}</section>;
  }

  if (system.isError) {
    return (
      <section className="card card-error" role="alert">
        <p>{errorMessage(t, system.error)}</p>
        <button type="button" onClick={() => void system.refetch()}>
          {t("common.retry")}
        </button>
      </section>
    );
  }

  const info = system.data;
  const bytes = (value: number) => formatBytes(value, i18n.language);
  const disk = info.data_disk;

  return (
    <section className="card">
      <h2>{t("system.heading")}</h2>
      <dl className="facts">
        <dt>{t("system.os")}</dt>
        <dd>{info.os_version ?? info.os} ({info.arch})</dd>
        <dt>{t("system.cpu")}</dt>
        <dd>{t("system.cpuValue", { brand: info.cpu_brand, threads: info.cpu_threads })}</dd>
        <dt>{t("system.memory")}</dt>
        <dd>
          {t("system.freeOfTotal", {
            free: bytes(info.memory_available_bytes),
            total: bytes(info.memory_total_bytes),
          })}
        </dd>
        <dt>{t("system.disk")}</dt>
        <dd>
          {disk
            ? t("system.freeOfTotal", { free: bytes(disk.available_bytes), total: bytes(disk.total_bytes) })
            : t("system.unknown")}
        </dd>
        <dt>{t("system.dataDir")}</dt>
        <dd>
          <code>{info.data_dir}</code>
        </dd>
        <dt>{t("system.version")}</dt>
        <dd>{info.version}</dd>
      </dl>
    </section>
  );
}
