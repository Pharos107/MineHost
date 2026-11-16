import type { TFunction } from "i18next";
import { Info } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { Server } from "../../../api/client";
import { formatMemoryMb } from "../../../format";
import { Card, Facts } from "../../../ui";

const EVERY = /^every_(\d+)h$/;
const DAILY = /^daily@(\d{2}:\d{2})$/;

/** Lịch sao lưu dạng `off`, `every_<n>h`, `daily@HH:MM` (spec P2, mục 4.1) thành câu đọc được. */
export function backupText(t: TFunction, schedule: string | undefined): string {
  const every = schedule ? EVERY.exec(schedule) : null;
  if (every) return t("overview.backupEvery", { count: Number(every[1]) });
  const daily = schedule ? DAILY.exec(schedule) : null;
  if (daily) return t("overview.backupDaily", { time: daily[1] });
  return t("overview.no");
}

/** Cài đặt hay hỏi của server trên một thẻ, để không phải mở trang Cài đặt mới biết. */
export function ServerInfoCard({ server }: { server: Server }) {
  const { t, i18n } = useTranslation();
  const yesNo = (value: boolean | null | undefined) => (value ? t("overview.yes") : t("overview.no"));
  const memory = server.memory_mb;

  return (
    <Card title={t("overview.info")} icon={Info}>
      <Facts
        items={[
          { label: t("overview.java"), value: server.java_major ?? "—" },
          {
            label: t("overview.ram"),
            value: memory
              ? t("overview.ramRange", { min: formatMemoryMb(memory.min, i18n.language), max: formatMemoryMb(memory.max, i18n.language) })
              : "—",
          },
          { label: t("overview.port"), value: server.port ?? "—" },
          { label: t("overview.autostart"), value: yesNo(server.autostart) },
          { label: t("overview.restartOnCrash"), value: yesNo(server.restart_on_crash) },
          { label: t("overview.dailyRestart"), value: server.restart_at?.length ? server.restart_at.join(", ") : t("overview.no") },
          { label: t("overview.backup"), value: backupText(t, server.backup?.schedule) },
        ]}
      />
    </Card>
  );
}
