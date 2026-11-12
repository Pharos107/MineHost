import type { Detected, ImportServer, Loader } from "../../api/client";

/** Giá trị đang nhập ở luồng "Nhập thư mục có sẵn". `loader`/`minecraft` chỉ dùng khi chọn tay. */
export interface ImportChoice {
  path: string;
  loader: Loader | "";
  minecraft: string;
  name: string;
}

/** Đi tiếp được khi đã xem thư mục, không có mod, có jar, và biết loại cùng phiên bản. */
export function importReady(choice: ImportChoice, found: Detected | undefined): boolean {
  if (!found || found.has_mods || !found.jar) return false;
  const loader = found.loader ?? choice.loader;
  const minecraft = found.minecraft ?? choice.minecraft.trim();
  return loader !== "" && minecraft !== "";
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
