import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LogIn } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Navigate, useNavigate, useSearchParams } from "react-router";

import { authApi, needsLogin, safeNext, SESSION_KEY } from "../../api/auth";
import { ApiError, errorMessage } from "../../api/client";
import { Button, Card, Field, Notice, Stack, TextInput } from "../../ui";

/** Giờ hiện tại, cập nhật mỗi giây cho tới `until`. Dùng để đếm lùi thời gian bị khóa. */
function useNow(until: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (until <= Date.now()) return;
    const timer = setInterval(() => {
      const current = Date.now();
      setNow(current);
      if (current >= until) clearInterval(timer);
    }, 1000);
    return () => clearInterval(timer);
  }, [until]);
  return now;
}

/** Trang Đăng nhập (spec P2, mục 6.10): một ô mật khẩu, xong thì quay về trang đang mở dở. */
export function LoginPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  const [password, setPassword] = useState("");
  const [lock, setLock] = useState({ until: 0, seconds: 0 });
  const now = useNow(lock.until);
  const session = useQuery({ queryKey: SESSION_KEY, queryFn: authApi.session });
  const login = useMutation({
    mutationFn: () => authApi.login(password),
    onSuccess: async () => {
      await queryClient.invalidateQueries();
      void navigate(next, { replace: true });
    },
    onError: (error) => {
      if (error instanceof ApiError && error.code === "AUTH_RATE_LIMITED") {
        const seconds = Number(error.params.retry_after_s) || 0;
        setLock({ until: Date.now() + seconds * 1000, seconds });
      }
    },
  });
  // `now` có thể cũ hơn lúc bị khóa tới một giây, nên chặn trên bằng số giây server trả về.
  const lockedFor = Math.max(0, Math.min(lock.seconds, Math.ceil((lock.until - now) / 1000)));

  if (session.data && !needsLogin(session.data) && !login.isPending) return <Navigate to={next} replace />;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (password !== "" && lockedFor === 0) login.mutate();
      }}
    >
      <Card title={t("auth.title")} description={t("auth.description")}>
        <Stack gap={4}>
          <Field label={t("auth.password")}>
            <TextInput
              type="password"
              autoComplete="current-password"
              autoFocus
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          {lockedFor > 0 && <Notice tone="warning">{t("auth.locked", { seconds: lockedFor })}</Notice>}
          {login.isError && lockedFor === 0 && <Notice tone="danger">{errorMessage(t, login.error)}</Notice>}
          <Notice tone="info">{t("auth.forgot")}</Notice>
          <Stack direction="row" justify="end">
            <Button type="submit" variant="primary" icon={LogIn} loading={login.isPending} disabled={password === "" || lockedFor > 0}>
              {t("auth.submit")}
            </Button>
          </Stack>
        </Stack>
      </Card>
    </form>
  );
}
