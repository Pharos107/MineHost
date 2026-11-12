/** Giống backend: đếm ký tự Unicode, không đếm byte hay đơn vị UTF-16. */
export const MIN_PASSWORD_CHARS = 8;

export interface PasswordForm {
  current: string;
  next: string;
  confirm: string;
}

export const EMPTY_PASSWORD_FORM: PasswordForm = { current: "", next: "", confirm: "" };

/** Khóa i18n của lỗi từng ô; `undefined` là hợp lệ. Ô còn trống thì chưa báo lỗi. */
export function passwordErrors(form: PasswordForm): { next?: string; confirm?: string } {
  const errors: { next?: string; confirm?: string } = {};
  if (form.next !== "" && [...form.next].length < MIN_PASSWORD_CHARS) errors.next = "errors.PASSWORD_TOO_SHORT";
  if (form.confirm !== "" && form.confirm !== form.next) errors.confirm = "appSettings.mismatch";
  return errors;
}

/** Đủ điều kiện gửi: mật khẩu mới hợp lệ, nhập lại khớp, và có mật khẩu hiện tại khi cần. */
export function passwordReady(form: PasswordForm, needsCurrent: boolean): boolean {
  const errors = passwordErrors(form);
  return form.next !== "" && form.confirm !== "" && !errors.next && !errors.confirm && (!needsCurrent || form.current !== "");
}
