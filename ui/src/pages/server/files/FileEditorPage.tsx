import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Copy, Download, RotateCw, Save } from "lucide-react";
import { useEffect, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate, useSearchParams } from "react-router";

import { ApiError, errorMessage } from "../../../api/client";
import { filesApi, pathSteps } from "../../../api/files";
import { Breadcrumb, Button, Notice, Spinner, Stack, TextArea } from "../../../ui";
import { useServer } from "../ServerLayout";

function parentOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut === -1 ? "" : path.slice(0, cut);
}

/** Trình sửa file văn bản ≤ 1 MiB (spec P2, mục 6.6). Lưu bằng nút hoặc Ctrl+S. */
export function FileEditorPage() {
  const { t } = useTranslation();
  const server = useServer();
  const path = useSearchParams()[0].get("path") ?? "";
  const key = ["file", server.id, path];
  const queryClient = useQueryClient();
  const file = useQuery({ queryKey: key, queryFn: () => filesApi.read(server.id, path), staleTime: Infinity });
  const [draft, setDraft] = useState<string | null>(null);
  const text = draft ?? file.data?.text ?? "";
  const dirty = draft !== null && draft !== file.data?.text;

  const save = useMutation({
    mutationFn: () => filesApi.write(server.id, path, text, file.data?.etag),
    onSuccess: ({ etag }) => {
      queryClient.setQueryData(key, { text, etag, size: new TextEncoder().encode(text).length });
      setDraft(null);
    },
  });
  const changedOnDisk = save.error instanceof ApiError && save.error.code === "FILE_CHANGED";
  useLeaveWarning(dirty);

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (dirty && !save.isPending) save.mutate();
    }
  }

  async function reload() {
    setDraft(null);
    save.reset();
    await queryClient.invalidateQueries({ queryKey: key });
  }

  return (
    <Stack gap={4}>
      <Header serverId={server.id} path={path} />
      {file.isPending && <Spinner label={t("ui.loading")} />}
      {file.isError && <ReadError serverId={server.id} path={path} error={file.error} />}
      {file.isSuccess && (
        <>
          {changedOnDisk ? (
            <Notice
              tone="warning"
              title={t("files.changedTitle")}
              action={
                <Stack direction="row" gap={2} wrap>
                  <Button size="sm" icon={RotateCw} onClick={() => void reload()}>
                    {t("files.reloadLatest")}
                  </Button>
                  <Button size="sm" icon={Copy} onClick={() => void navigator.clipboard.writeText(text)}>
                    {t("files.copyMine")}
                  </Button>
                </Stack>
              }
            >
              {t("files.changedBody")}
            </Notice>
          ) : (
            save.isError && <Notice tone="danger">{errorMessage(t, save.error)}</Notice>
          )}
          <TextArea variant="code" aria-label={t("files.editorLabel", { path })} value={text} onChange={(e) => setDraft(e.target.value)} onKeyDown={onKeyDown} />
          <Stack direction="row" gap={2} align="center" wrap>
            <Button variant="primary" icon={Save} disabled={!dirty} loading={save.isPending} onClick={() => save.mutate()}>
              {t("files.save")}
            </Button>
            <span>{dirty ? t("files.unsaved") : save.isSuccess ? t("files.saved") : null}</span>
          </Stack>
        </>
      )}
    </Stack>
  );
}

function Header({ serverId, path }: { serverId: string; path: string }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const base = `/servers/${serverId}/files`;
  const steps = pathSteps(path);
  return (
    <Stack direction="row" gap={3} align="center" wrap>
      <Button variant="ghost" size="sm" icon={ArrowLeft} onClick={() => void navigate(`${base}?path=${encodeURIComponent(parentOf(path))}`)}>
        {t("files.back")}
      </Button>
      <Breadcrumb
        label={t("files.path")}
        items={[
          { label: t("files.root"), to: `${base}?path=` },
          ...steps.map((s, i) => ({ label: s.name, to: i < steps.length - 1 ? `${base}?path=${encodeURIComponent(s.path)}` : undefined })),
        ]}
      />
    </Stack>
  );
}

/** File không mở được bằng trình sửa (nhị phân, quá lớn…): vẫn cho tải xuống. */
function ReadError({ serverId, path, error }: { serverId: string; path: string; error: unknown }) {
  const { t } = useTranslation();
  return (
    <Notice
      tone="warning"
      action={
        <Link to={filesApi.downloadUrl(serverId, path)} reloadDocument>
          <Stack direction="row" gap={1} align="center">
            <Download size={16} aria-hidden />
            {t("files.download")}
          </Stack>
        </Link>
      }
    >
      {errorMessage(t, error)}
    </Notice>
  );
}

/** Hỏi lại khi đóng tab hoặc tải lại trang mà còn thay đổi chưa lưu. */
function useLeaveWarning(dirty: boolean) {
  useEffect(() => {
    if (!dirty) return;
    const onBeforeUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [dirty]);
}
