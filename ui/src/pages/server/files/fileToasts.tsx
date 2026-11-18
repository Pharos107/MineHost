import type { TFunction } from "i18next";

import { errorMessage, type Job } from "../../../api/client";
import { uploadFiles, type UploadItem } from "../../../api/files";
import { ProgressBar, toast } from "../../../ui";
import { JobProgress, jobError } from "../JobProgress";

function itemLabel(item: File | UploadItem): string {
  return item instanceof File ? item.name : item.path ?? item.file.name;
}

/**
 * Tải lên với tiến độ trong một toast (spec trang Tệp, mục 2): toast ở lại khi còn gửi, xong thì
 * báo kết quả rồi tự tắt sau 5 giây. Không chèn gì vào trang nên bảng không bị đẩy xuống.
 */
export async function uploadWithToast(t: TFunction, serverId: string, dir: string, files: Array<File | UploadItem>, onDone: () => void) {
  const namesList = files.slice(0, 3).map(itemLabel).join(", ") + (files.length > 3 ? ` (+${files.length - 3})` : "");
  const label = t("files.uploading", { names: namesList });
  const id = toast.show({ sticky: true, body: <ProgressBar label={label} value={0} /> });
  let shown = 0;
  try {
    const names = await uploadFiles(serverId, dir, files, (sent, total) => {
      const percent = total > 0 ? Math.floor((sent / total) * 100) : 0;
      // Chỉ cập nhật khi đổi phần trăm: sự kiện tiến độ bắn hàng trăm lần mỗi giây.
      if (percent === shown) return;
      shown = percent;
      toast.update(id, { sticky: true, body: <ProgressBar label={label} value={percent} /> });
    });
    toast.update(id, { tone: "success", title: t("files.uploaded", { count: names.length }), body: undefined, sticky: false });
    onDone();
  } catch (e) {
    toast.update(id, { tone: "danger", title: t("files.uploadFailed"), body: errorMessage(t, e), sticky: false });
    // Thư mục nhiều file có thể đã lên xong các lô đầu trước khi gặp lỗi ở lô sau.
    onDone();
  }
}

/** Toast theo dõi Job nén / giải nén: thanh tiến độ khi chạy, rồi "Đã xong" hoặc lỗi. */
export function watchJobInToast(t: TFunction, jobId: number, onDone: () => void) {
  let id = 0;
  const finish = (job: Job) => {
    onDone();
    const error = jobError(job);
    if (error) {
      toast.update(id, { tone: "danger", body: errorMessage(t, error), sticky: false });
      return;
    }
    toast.update(id, { tone: "success", title: t(`files.jobDone.${job.kind}`, { defaultValue: t("files.jobDone.other") }), body: undefined, sticky: false });
  };
  id = toast.show({ sticky: true, body: <JobProgress jobId={jobId} onFinish={finish} /> });
}

/** Mục đã chuyển vào thùng rác. */
export function trashedToast(t: TFunction, paths: string[]) {
  toast.show({
    tone: "success",
    title: t("files.trashed"),
    body: paths.map((path) => (
      <div key={path}>
        <code>{path}</code>
      </div>
    )),
  });
}
