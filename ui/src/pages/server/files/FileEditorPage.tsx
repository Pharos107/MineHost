import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Copy, Download, RotateCw, Save } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate, useSearchParams } from "react-router";

import { ApiError, errorMessage } from "../../../api/client";
import { filesApi, parentOf, pathSteps } from "../../../api/files";
import { formatBytes } from "../../../format";
import { Breadcrumb, Button, CodeEditor, ImagePreview, languageFor, Notice, Spinner, Stack, Switch } from "../../../ui";
import { useServer } from "../ServerLayout";

const WRAP_KEY = "minehost.editor.wrap";
const isImage = (path: string) => /\.(png|jpe?g|gif|webp)$/i.test(path);
const isNbt = (path: string) => /\.(dat|dat_old|nbt)$/i.test(path);

/** Trình sửa code và xem trước ảnh / NBT / file log lớn (spec trang Tệp & nâng cấp). */
export function FileEditorPage() {
  const server = useServer();
  const path = useSearchParams()[0].get("path") ?? "";
  if (isImage(path)) return <ImageView key={path} serverId={server.id} path={path} />;
  if (isNbt(path)) return <NbtView key={path} serverId={server.id} path={path} />;
  return <TextEditorView key={path} serverId={server.id} path={path} />;
}

function ImageView({ serverId, path }: { serverId: string; path: string }) {
  const { t, i18n } = useTranslation();
  const [failed, setFailed] = useState(false);
  const list = useQuery({ queryKey: ["files", serverId, parentOf(path)], queryFn: () => filesApi.list(serverId, parentOf(path)) });
  const size = list.data?.find((e) => e.name === path.slice(path.lastIndexOf("/") + 1))?.size;

  return (
    <Stack gap={4}>
      <Stack direction="row" gap={3} align="center" justify="between" wrap>
        <Header serverId={serverId} path={path} />
        <Button size="sm" icon={Download} onClick={() => window.location.assign(filesApi.downloadUrl(serverId, path))}>
          {t("files.download")}
        </Button>
      </Stack>
      {failed ? (
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
          {t("files.preview.imageError")}
        </Notice>
      ) : (
        <ImagePreview
          src={filesApi.rawUrl(serverId, path)}
          alt={path}
          onError={() => setFailed(true)}
          caption={(dims) =>
            dims && size != null ? t("files.preview.imageMeta", { width: dims.width, height: dims.height, size: formatBytes(size, i18n.language) }) : ""
          }
        />
      )}
    </Stack>
  );
}

function NbtView({ serverId, path }: { serverId: string; path: string }) {
  const { t, i18n } = useTranslation();
  const [wrap, setWrap] = useWrapSetting();
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  const nbt = useQuery({ queryKey: ["file-nbt", serverId, path], queryFn: () => filesApi.nbt(serverId, path) });

  return (
    <Stack gap={4}>
      <Stack direction="row" gap={3} align="center" justify="between" wrap>
        <Header serverId={serverId} path={path} />
        <Stack direction="row" gap={3} align="center" wrap>
          {nbt.isSuccess && <Switch label={t("files.wrap")} checked={wrap} onChange={setWrap} />}
          <Button size="sm" icon={Download} onClick={() => window.location.assign(filesApi.downloadUrl(serverId, path))}>
            {t("files.download")}
          </Button>
        </Stack>
      </Stack>
      {nbt.isPending && <Spinner label={t("ui.loading")} />}
      {nbt.isError && <ReadError serverId={serverId} path={path} error={nbt.error} />}
      {nbt.isSuccess && (
        <>
          <Notice tone="info">
            {t("files.preview.nbtBanner", { kind: t(nbt.data.compressed ? "files.preview.nbtGzip" : "files.preview.nbtRaw") })}
          </Notice>
          <CodeEditor
            label={t("files.editorLabel", { path })}
            value={nbt.data.json}
            onChange={() => {}}
            language="json"
            readOnly
            wrap={wrap}
            onCursor={(line, column) => setCursor({ line, column })}
            status={
              <>
                <span>{t("files.cursor", cursor)}</span>
                <span>{t("files.language.json")}</span>
                <span>{formatBytes(nbt.data.size, i18n.language)}</span>
              </>
            }
          />
        </>
      )}
    </Stack>
  );
}

function TextEditorView({ serverId, path }: { serverId: string; path: string }) {
  const { t, i18n } = useTranslation();
  const key = ["file", serverId, path];
  const queryClient = useQueryClient();
  const file = useQuery({
    queryKey: key,
    queryFn: () => filesApi.read(serverId, path),
    staleTime: Infinity,
    retry: false,
  });
  const tooLarge = file.error instanceof ApiError && file.error.code === "FILE_TOO_LARGE";
  const preview = useQuery({
    queryKey: ["file-preview", serverId, path],
    queryFn: () => filesApi.preview(serverId, path),
    enabled: tooLarge,
    retry: false,
  });

  const [draft, setDraft] = useState<string | null>(null);
  const [wrap, setWrap] = useWrapSetting();
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  const text = draft ?? file.data?.text ?? "";
  const dirty = draft !== null && draft !== file.data?.text;

  const save = useMutation({
    mutationFn: () => filesApi.write(serverId, path, text, file.data?.etag),
    onSuccess: ({ etag }) => {
      queryClient.setQueryData(key, { text, etag, size: new TextEncoder().encode(text).length });
      setDraft(null);
    },
  });
  const changedOnDisk = save.error instanceof ApiError && save.error.code === "FILE_CHANGED";
  const trySave = () => {
    if (dirty && !save.isPending) save.mutate();
  };
  useLeaveWarning(dirty);

  async function reload() {
    setDraft(null);
    save.reset();
    await queryClient.invalidateQueries({ queryKey: key });
  }

  return (
    <Stack gap={4}>
      <Stack direction="row" gap={3} align="center" justify="between" wrap>
        <Header serverId={serverId} path={path} />
        {(file.isSuccess || preview.isSuccess) && (
          <Stack direction="row" gap={3} align="center" wrap>
            <Switch label={t("files.wrap")} checked={wrap} onChange={setWrap} />
            <Button size="sm" icon={Download} onClick={() => window.location.assign(filesApi.downloadUrl(serverId, path))}>
              {t("files.download")}
            </Button>
            {file.isSuccess && (
              <Button size="sm" variant="primary" icon={Save} disabled={!dirty} loading={save.isPending} onClick={trySave}>
                {t("files.save")}
              </Button>
            )}
          </Stack>
        )}
      </Stack>
      {(file.isPending || (tooLarge && preview.isPending)) && <Spinner label={t("ui.loading")} />}
      {file.isError && !tooLarge && <ReadError serverId={serverId} path={path} error={file.error} />}
      {tooLarge && preview.isError && <ReadError serverId={serverId} path={path} error={preview.error} />}
      {tooLarge && preview.isSuccess && (
        <>
          <Notice tone="info">
            {t("files.preview.largeBanner", {
              total: formatBytes(preview.data.total_bytes, i18n.language),
              shown: formatBytes(preview.data.shown_bytes, i18n.language),
            })}
          </Notice>
          <CodeEditor
            label={t("files.editorLabel", { path })}
            value={preview.data.text}
            onChange={() => {}}
            language={languageFor(path)}
            readOnly
            wrap={wrap}
            onCursor={(line, column) => setCursor({ line, column })}
            status={
              <>
                <span>{t("files.cursor", cursor)}</span>
                <span>{t(`files.language.${languageFor(path)}`)}</span>
                <span>{formatBytes(preview.data.total_bytes, i18n.language)}</span>
              </>
            }
          />
        </>
      )}
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
          <Editor path={path} text={text} dirty={dirty} wrap={wrap} onChange={setDraft} onSave={trySave} />
        </>
      )}
    </Stack>
  );
}

interface EditorProps {
  path: string;
  text: string;
  dirty: boolean;
  wrap: boolean;
  onChange: (text: string) => void;
  onSave: () => void;
}

/** Trình sửa kèm thanh trạng thái: lưu chưa, vị trí con trỏ, ngôn ngữ, kích thước. */
function Editor({ path, text, dirty, wrap, onChange, onSave }: EditorProps) {
  const { t, i18n } = useTranslation();
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  const language = languageFor(path);
  const size = new TextEncoder().encode(text).length;
  return (
    <CodeEditor
      label={t("files.editorLabel", { path })}
      value={text}
      onChange={onChange}
      language={language}
      wrap={wrap}
      onSave={onSave}
      onCursor={(line, column) => setCursor({ line, column })}
      status={
        <>
          <span role="status">{dirty ? t("files.stateUnsaved") : t("files.stateSaved")}</span>
          <span>{t("files.cursor", cursor)}</span>
          <span>{t(`files.language.${language}`)}</span>
          <span>{formatBytes(size, i18n.language)}</span>
          <span>{t("files.encoding")}</span>
        </>
      }
    />
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

/** Tùy chọn xuống dòng nhớ theo trình duyệt; bộ nhớ trình duyệt bị chặn thì chỉ giữ trong phiên. */
function useWrapSetting(): [boolean, (wrap: boolean) => void] {
  const [wrap, setWrap] = useState(() => {
    try {
      return localStorage.getItem(WRAP_KEY) === "1";
    } catch {
      return false;
    }
  });
  const update = (next: boolean) => {
    setWrap(next);
    try {
      localStorage.setItem(WRAP_KEY, next ? "1" : "0");
    } catch {
      // Không lưu được thì thôi, công tắc vẫn chạy trong phiên này.
    }
  };
  return [wrap, update];
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
