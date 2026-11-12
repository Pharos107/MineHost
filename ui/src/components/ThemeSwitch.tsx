import { Moon, Sun } from "lucide-react";
import { useTranslation } from "react-i18next";

import { IconButton, useTheme } from "../ui";

/** Nút chuyển giao diện sáng / tối, cạnh ô đổi ngôn ngữ. Mặc định theo hệ điều hành. */
export function ThemeSwitch() {
  const { t } = useTranslation();
  const [theme, toggle] = useTheme();
  return theme === "dark" ? (
    <IconButton icon={Sun} label={t("app.themeToLight")} onClick={toggle} />
  ) : (
    <IconButton icon={Moon} label={t("app.themeToDark")} onClick={toggle} />
  );
}
