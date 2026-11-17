import { Plus, RotateCcw, Undo2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { Property } from "../../api/client";
import {
  Badge,
  Button,
  Card,
  DataTable,
  Dialog,
  DialogBody,
  DialogFooter,
  Field,
  InfoTip,
  Notice,
  SegmentedControl,
  Select,
  Stack,
  Switch,
  TextInput,
  type Column,
  type RowAction,
} from "../../ui";
import { HIDDEN_FROM_ADVANCED } from "./properties";
import {
  ADVANCED_FILTERS,
  isValidPropertyKey,
  PROPERTY_META,
  resolvePropertyControl,
  type AdvancedFilter,
} from "./propertiesMeta";

interface AdvancedPropertiesCardProps {
  draft: Property[];
  initial: Property[];
  filter: AdvancedFilter;
  onFilterChange: (filter: AdvancedFilter) => void;
  onSet: (key: string, value: string) => void;
  onRemove: (key: string) => void;
  onAddClick: () => void;
}

/** Bảng Nâng cao: gom nhóm theo chủ đề, dùng công tắc/select/ô số theo kiểu giá trị và thêm khóa mới. */
export function AdvancedPropertiesCard({
  draft,
  initial,
  filter,
  onFilterChange,
  onSet,
  onRemove,
  onAddClick,
}: AdvancedPropertiesCardProps) {
  const { t } = useTranslation();

  const initialMap = new Map(initial.map((p) => [p.key, p.value]));
  const isModified = (p: Property) => initialMap.get(p.key) !== p.value;
  const advancedRows = draft.filter((p) => !HIDDEN_FROM_ADVANCED.has(p.key));
  const modifiedCount = advancedRows.filter(isModified).length;

  const visibleRows = advancedRows.filter((p) => {
    if (filter === "all") return true;
    if (filter === "modified") return isModified(p);
    return resolvePropertyControl(p.key, p.value).category === filter;
  });

  const columns: Column<Property>[] = [
    {
      id: "key",
      header: t("game.key"),
      sortValue: (p) => p.key,
      width: "lg",
      cell: (p) => <PropertyKeyCell prop={p} modified={isModified(p)} />,
    },
    {
      id: "value",
      header: t("game.value"),
      cell: (p) => <PropertyValueCell prop={p} onSet={onSet} />,
    },
  ];

  const rowActions: RowAction<Property>[] = [
    {
      icon: Undo2,
      label: t("game.revertRow"),
      hidden: (p) => !isModified(p),
      onSelect: (p) => {
        const saved = initialMap.get(p.key);
        if (saved !== undefined) onSet(p.key, saved);
        else onRemove(p.key);
      },
    },
    {
      icon: RotateCcw,
      label: t("game.resetDefault"),
      hidden: (p) => {
        const def = PROPERTY_META[p.key]?.defaultValue;
        return def === undefined || p.value === def;
      },
      onSelect: (p) => {
        const def = PROPERTY_META[p.key]?.defaultValue;
        if (def !== undefined) onSet(p.key, def);
      },
    },
  ];

  return (
    <Card
      title={t("game.advanced")}
      description={t("game.advancedHint")}
      actions={
        <Button size="sm" icon={Plus} onClick={onAddClick}>
          {t("game.addKey")}
        </Button>
      }
    >
      {advancedRows.length > 0 && (
        <SegmentedControl
          label={t("game.filter.label")}
          size="sm"
          value={filter}
          onChange={onFilterChange}
          options={ADVANCED_FILTERS.map((value) => ({
            value,
            label:
              value === "modified" && modifiedCount > 0
                ? t("game.filter.modifiedCount", { count: modifiedCount })
                : t(`game.filter.${value}`),
          }))}
        />
      )}
      <DataTable
        label={t("game.advanced")}
        columns={columns}
        rows={visibleRows}
        rowKey={(p) => p.key}
        rowLabel={(p) => p.key}
        searchText={(p) => [p.key, t(`game.advHints.${p.key}`, { defaultValue: "" })].filter(Boolean).join(" ")}
        actions={rowActions}
        empty={<Notice>{t(advancedRows.length === 0 ? "game.empty" : "game.noFilterMatch")}</Notice>}
      />
    </Card>
  );
}

function PropertyKeyCell({ prop, modified }: { prop: Property; modified: boolean }) {
  const { t } = useTranslation();
  const hint = t(`game.advHints.${prop.key}`, { defaultValue: "" });
  const newWorldOnly = PROPERTY_META[prop.key]?.newWorldOnly;

  return (
    <Stack direction="row" gap={2} align="center" wrap>
      <Stack direction="row" gap={1} align="center">
        <code>{prop.key}</code>
        {hint && <InfoTip label={t("game.infoTipLabel", { key: prop.key })} text={hint} />}
      </Stack>
      {modified && <Badge tone="primary">{t("game.modifiedBadge")}</Badge>}
      {newWorldOnly && <Badge tone="info">{t("game.newWorldBadge")}</Badge>}
    </Stack>
  );
}

function PropertyValueCell({ prop, onSet }: { prop: Property; onSet: (key: string, value: string) => void }) {
  const { t } = useTranslation();
  const { kind, meta } = resolvePropertyControl(prop.key, prop.value);

  if (kind === "boolean") {
    return (
      <Switch
        hideLabel
        label={prop.key}
        checked={prop.value === "true"}
        onChange={(on) => onSet(prop.key, String(on))}
      />
    );
  }

  if (kind === "select" && meta?.options) {
    const options = meta.options.includes(prop.value) ? meta.options : [prop.value, ...meta.options];
    return (
      <Select aria-label={prop.key} value={prop.value} onChange={(e) => onSet(prop.key, e.target.value)}>
        {options.map((option) => (
          <option key={option} value={option}>
            {option === "" ? t("game.emptyOption") : t(`game.advOptions.${prop.key}.${option}`, { defaultValue: option })}
          </option>
        ))}
      </Select>
    );
  }

  if (kind === "number") {
    const allowNegative = meta?.min !== undefined && meta.min < 0;
    return (
      <TextInput
        type="number"
        inputMode={allowNegative ? undefined : "numeric"}
        min={meta?.min}
        max={meta?.max}
        aria-label={prop.key}
        value={prop.value}
        onChange={(e) => onSet(prop.key, e.target.value)}
      />
    );
  }

  return (
    <TextInput
      type={kind === "password" ? "password" : "text"}
      aria-label={prop.key}
      value={prop.value}
      onChange={(e) => onSet(prop.key, e.target.value)}
    />
  );
}

interface AddPropertyDialogProps {
  open: boolean;
  existingKeys: ReadonlySet<string>;
  onClose: () => void;
  onAdd: (key: string, value: string) => void;
}

export function AddPropertyDialog({ open, existingKeys, onClose, onAdd }: AddPropertyDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onClose={onClose} title={t("game.addKeyTitle")}>
      <AddPropertyForm existingKeys={existingKeys} onClose={onClose} onAdd={onAdd} />
    </Dialog>
  );
}

function AddPropertyForm({ existingKeys, onClose, onAdd }: Omit<AddPropertyDialogProps, "open">) {
  const { t } = useTranslation();
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const trimmed = key.trim();
  let error: string | undefined;
  if (submitted || trimmed.length > 0) {
    if (!isValidPropertyKey(trimmed)) error = t("game.addKeyInvalid");
    else if (HIDDEN_FROM_ADVANCED.has(trimmed) || existingKeys.has(trimmed)) error = t("game.addKeyExists");
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        setSubmitted(true);
        if (!isValidPropertyKey(trimmed) || HIDDEN_FROM_ADVANCED.has(trimmed) || existingKeys.has(trimmed)) return;
        onAdd(trimmed, value);
      }}
    >
      <DialogBody>
        <Stack gap={4}>
          <Field label={t("game.key")} hint={t("game.addKeyHint")} error={error}>
            <TextInput data-autofocus value={key} placeholder="allow-nether" autoComplete="off" onChange={(e) => setKey(e.target.value)} />
          </Field>
          <Field label={t("game.value")}>
            <TextInput value={value} placeholder="true" autoComplete="off" onChange={(e) => setValue(e.target.value)} />
          </Field>
        </Stack>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onClose}>
          {t("common.cancel")}
        </Button>
        <Button type="submit" variant="primary" icon={Plus}>
          {t("game.addKeyConfirm")}
        </Button>
      </DialogFooter>
    </form>
  );
}
