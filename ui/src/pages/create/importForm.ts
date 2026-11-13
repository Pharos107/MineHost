import type { Detected, ImportServer, Loader } from "../../api/client";
import { isModLoader } from "./form";

/** Giá trị đang nhập ở luồng "Nhập thư mục có sẵn". `loader`/`minecraft` chỉ dùng khi chọn tay. */
export interface ImportChoice {
  path: string;
  loader: Loader | "";
  minecraft: string;
  name: string;
}

/** Đi tiếp được khi đã xem thư mục, có cách chạy (jar hoặc args_file), biết loại + bản MC, và không có mod ở server thường. */
export function importReady(choice: ImportChoice, found: Detected | undefined): boolean {
  if (!found) return false;
  const loader = (found.loader ?? choice.loader) as Loader | "";
  const minecraft = found.minecraft ?? choice.minecraft.trim();
  const hasLaunch = Boolean(found.jar) || found.loader === "forge" || found.loader === "neoforge";
  if (!hasLaunch || loader === "" || minecraft === "") return false;
  if (found.has_mods && !isModLoader(loader)) return false;
  return true;
}

/** Chỉ gửi loại và phiên bản khi người dùng chọn tay; còn lại để backend dùng kết quả nhận diện. */
export function importRequest(choice: ImportChoice, found: Detected): ImportServer {
  return {
    path: choice.path.trim(),
    name: choice.name.trim(),
    loader: found.loader ? null : choice.loader || null,
    minecraft: found.minecraft ? null : choice.minecraft.trim() || null,
  };
}
