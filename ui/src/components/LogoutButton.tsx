import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut } from "lucide-react";
import { useTranslation } from "react-i18next";

import { authApi, SESSION_KEY } from "../api/auth";
import { Button } from "../ui";

/** Nút Đăng xuất ở chân thanh bên. Chỉ hiện khi đang đăng nhập. */
export function LogoutButton() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = useQuery({ queryKey: SESSION_KEY, queryFn: authApi.session });
  const logout = useMutation({
    mutationFn: authApi.logout,
    // Xóa dữ liệu đã tải: người dùng kế tiếp trên trình duyệt này không thấy gì trước khi đăng nhập.
    onSettled: () => {
      queryClient.removeQueries({ predicate: (query) => query.queryKey[0] !== SESSION_KEY[0] });
      void queryClient.invalidateQueries({ queryKey: SESSION_KEY });
    },
  });

  if (!session.data?.logged_in) return null;
  return (
    <Button variant="ghost" size="sm" icon={LogOut} loading={logout.isPending} onClick={() => logout.mutate()}>
      {t("auth.logout")}
    </Button>
  );
}
