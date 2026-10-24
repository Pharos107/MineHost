import { House, SearchX } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { Button, EmptyState } from "../ui";

export function NotFoundPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  return (
    <EmptyState
      icon={SearchX}
      title={t("notFound.title")}
      action={
        <Button icon={House} onClick={() => void navigate("/")}>
          {t("notFound.home")}
        </Button>
      }
    />
  );
}
