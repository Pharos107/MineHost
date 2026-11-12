import { useMutation, useQueryClient } from "@tanstack/react-query";
import { KeyRound, LockKeyhole, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ACCESS_KEY, authApi, SESSION_KEY, type Session } from "../../api/auth";
import { errorMessage } from "../../api/client";
import { Button, Card, Field, Notice, Stack, TextInput } from "../../ui";
import { EMPTY_PASSWORD_FORM, passwordErrors, passwordReady, type PasswordForm } from "./password";

/** Mục "Bảo mật": đặt, đổi, xóa mật khẩu admin. Đổi và xóa phải nhập mật khẩu hiện tại. */
export function SecurityCard({ session }: { session: Session }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState<PasswordForm>(EMPTY_PASSWORD_FORM);
  const hasPassword = session.password_set;
  // Đặt hoặc đổi mật khẩu thì mọi phiên bị hủy: đọc lại phiên, AuthGate đưa về trang Đăng nhập.
  const refreshSession = () => {
    setForm(EMPTY_PASSWORD_FORM);
    void queryClient.invalidateQueries({ queryKey: SESSION_KEY });
    void queryClient.invalidateQueries({ queryKey: ACCESS_KEY });
  };
  const save = useMutation({
    mutationFn: () => authApi.setPassword(hasPassword ? form.current : undefined, form.next),
    onSuccess: refreshSession,
  });
  const clear = useMutation({ mutationFn: () => authApi.clearPassword(form.current), onSuccess: refreshSession });
  const errors = passwordErrors(form);
  const update = (patch: Partial<PasswordForm>) => setForm((current) => ({ ...current, ...patch }));
  const busy = save.isPending || clear.isPending;
  const failure = save.error ?? clear.error;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (passwordReady(form, hasPassword)) save.mutate();
      }}
    >
      <Card title={t("appSettings.security")} description={t(hasPassword ? "appSettings.securityOn" : "appSettings.securityOff")} icon={LockKeyhole} layout="split">
        <Stack gap={4}>
          {hasPassword && (
            <Field label={t("appSettings.current")}>
              <TextInput type="password" autoComplete="current-password" value={form.current} onChange={(e) => update({ current: e.target.value })} />
            </Field>
          )}
          <Field label={t("appSettings.new")} hint={t("appSettings.newHint")} error={errors.next && t(errors.next)}>
            <TextInput type="password" autoComplete="new-password" value={form.next} onChange={(e) => update({ next: e.target.value })} />
          </Field>
          <Field label={t("appSettings.confirm")} error={errors.confirm && t(errors.confirm)}>
            <TextInput type="password" autoComplete="new-password" value={form.confirm} onChange={(e) => update({ confirm: e.target.value })} />
          </Field>
          {failure && <Notice tone="danger">{errorMessage(t, failure)}</Notice>}
          <Stack direction="row" gap={2} justify="end" wrap>
            {hasPassword && (
              <Button variant="danger" icon={Trash2} disabled={form.current === "" || busy} loading={clear.isPending} onClick={() => clear.mutate()}>
                {t("appSettings.remove")}
              </Button>
            )}
            <Button type="submit" variant="primary" icon={KeyRound} disabled={!passwordReady(form, hasPassword) || busy} loading={save.isPending}>
              {t(hasPassword ? "appSettings.change" : "appSettings.set")}
            </Button>
          </Stack>
        </Stack>
      </Card>
    </form>
  );
}
