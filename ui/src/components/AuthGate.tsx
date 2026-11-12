import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Navigate, useLocation } from "react-router";

import { authApi, needsLogin, SESSION_KEY } from "../api/auth";
import { onAuthRequired } from "../api/client";
import { Spinner } from "../ui";

/**
 * Đã đặt mật khẩu mà chưa đăng nhập thì chuyển về `/login`, kèm trang đang mở để quay lại
 * (spec P2, mục 6.10). Chưa đặt mật khẩu thì như P1. Không đọc được phiên (controller tắt) thì
 * để các trang tự báo lỗi như trước.
 */
export function AuthGate({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const location = useLocation();
  const session = useQuery({ queryKey: SESSION_KEY, queryFn: authApi.session });

  // Phiên hết hạn hoặc mật khẩu vừa đổi ở tab khác: đọc lại phiên, gate sẽ chuyển trang.
  useEffect(() => onAuthRequired(() => void queryClient.invalidateQueries({ queryKey: SESSION_KEY })), [queryClient]);

  if (session.isPending) return <Spinner label={t("auth.checking")} />;
  if (needsLogin(session.data) && location.pathname !== "/login") {
    const next = `${location.pathname}${location.search}`;
    return <Navigate to={`/login?next=${encodeURIComponent(next)}`} replace />;
  }
  return children;
}
