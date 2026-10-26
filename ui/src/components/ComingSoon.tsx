import { Construction } from "lucide-react";
import { useTranslation } from "react-i18next";

import { EmptyState, PageHeader } from "../ui";

/**
 * Chỗ giữ cho trang chưa làm trong P2. Có `title` thì kèm đầu trang (trang chung); trong một server
 * thì không, vì `ServerLayout` đã có `h1` là tên server.
 */
export function ComingSoon({ title }: { title?: string }) {
  const { t } = useTranslation();

  return (
    <>
      {title && <PageHeader title={title} />}
      <EmptyState icon={Construction} title={t("soon.title")} description={t("soon.description")} />
    </>
  );
}
