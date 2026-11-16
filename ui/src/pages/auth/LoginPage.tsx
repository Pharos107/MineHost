import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleHelp, LogIn } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Navigate, useNavigate, useSearchParams } from "react-router";

import { authApi, needsLogin, safeNext, SESSION_KEY } from "../../api/auth";
import { ApiError, errorMessage } from "../../api/client";
import { BlockWordmark, Button, Field, HangingSign, Notice, Stack, TextInput } from "../../ui";

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

function ForgotHelp({ forced }: { forced: boolean }) {
  const { t } = useTranslation();
  const [showForgot, setShowForgot] = useState(false);
  const forgotVisible = showForgot || forced;

  return (
    <Stack gap={3}>
      <Stack direction="row" justify="center">
        <Button
          variant="ghost"
          size="sm"
          icon={CircleHelp}
          aria-expanded={forgotVisible}
          onClick={() => setShowForgot((current) => !current)}
        >
          {t("auth.forgotToggle")}
        </Button>
      </Stack>
      {forgotVisible && <Notice tone="info">{t("auth.forgot")}</Notice>}
    </Stack>
  );
}

function useLoginForm(next: string) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [password, setPassword] = useState("");
  const [emptyError, setEmptyError] = useState(false);
  const [lock, setLock] = useState({ until: 0, seconds: 0 });
  const now = useNow(lock.until);
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
  const submit = () => {
    if (lockedFor > 0) return;
    if (password === "") {
      setEmptyError(true);
      return;
    }
    setEmptyError(false);
    login.mutate();
  };
  return { password, setPassword: (v: string) => { setPassword(v); setEmptyError(false); }, emptyError, lockedFor, login, submit };
}

/** Trang Đăng nhập (spec P2, mục 6.10): một ô mật khẩu, xong thì quay về trang đang mở dở. */
export function LoginPage() {
  const { t } = useTranslation();
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  const session = useQuery({ queryKey: SESSION_KEY, queryFn: authApi.session });
  const { password, setPassword, emptyError, lockedFor, login, submit } = useLoginForm(next);

  if (session.data && !needsLogin(session.data) && !login.isPending) return <Navigate to={next} replace />;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <HangingSign
        logo={<BlockWordmark label="MineHost" />}
        title={t("auth.title")}
        titleAs="h1"
        description={t("auth.description")}
        footer={<ForgotHelp forced={login.isError || lockedFor > 0} />}
      >
        <Stack gap={5}>
          <Field label={t("auth.password")} error={emptyError ? t("auth.passwordRequired") : undefined}>
            <TextInput
              type="password"
              size="lg"
              autoComplete="current-password"
              autoFocus
              placeholder={t("auth.passwordPlaceholder")}
              disabled={lockedFor > 0}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          {lockedFor > 0 && <Notice tone="warning">{t("auth.locked", { seconds: lockedFor })}</Notice>}
          {login.isError && lockedFor === 0 && <Notice tone="danger">{errorMessage(t, login.error)}</Notice>}
          <Button type="submit" variant="primary" size="lg" icon={LogIn} fullWidth loading={login.isPending} disabled={lockedFor > 0}>
            {t("auth.submit")}
          </Button>
        </Stack>
      </HangingSign>
    </form>
  );
}
