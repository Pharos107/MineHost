import { ArrowDown, ArrowUp, ArrowUpDown, X, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button, IconButton } from "./Button";
import { cx } from "./cx";
import styles from "./DataTable.module.css";
import { SearchInput } from "./inputs";
import { Menu } from "./Menu";
import { Spinner } from "./Spinner";
import { filterRows, nextSort, sortRows, type SortState } from "./table-utils";

export interface Column<T> {
  id: string;
  header: string;
  cell: (row: T) => ReactNode;
  /** Có thì bấm tiêu đề cột để sắp xếp. */
  sortValue?: (row: T) => string | number;
  align?: "start" | "center" | "end";
  /** Độ rộng theo token; bỏ trống thì cột tự giãn. */
  width?: "xs" | "sm" | "md" | "lg";
  /** Giữ nội dung trên một dòng (tên ngắn như tên server), để cột không bị bóp thành nhiều dòng. */
  nowrap?: boolean;
}

export interface RowAction<T> {
  label: string;
  icon: LucideIcon;
  onSelect: (row: T) => void;
  tone?: "danger";
  /** Ẩn ở hàng không áp dụng, ví dụ "Tắt" khi server đã tắt. */
  hidden?: (row: T) => boolean;
  disabled?: (row: T) => boolean;
}

/** Chọn nhiều hàng: ô chọn đầu mỗi hàng, ô chọn tất cả ở tiêu đề. Khóa là `rowKey`. */
export interface TableSelection {
  selected: ReadonlySet<string>;
  onChange: (selected: Set<string>) => void;
  /** Nút hiện thay thanh công cụ khi có hàng được chọn, ví dụ "Xóa", "Tải xuống". */
  actions: ReactNode;
}

interface DataTableProps<T> {
  /** Tên bảng cho trình đọc màn hình. */
  label: string;
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  /** Tên của hàng (ví dụ tên server), ghép vào nhãn nút để biết nút thuộc hàng nào. */
  rowLabel: (row: T) => string;
  actions?: RowAction<T>[];
  /** Số thao tác hiện thành icon; phần còn lại vào menu "…". */
  inlineActions?: number;
  /** Nút bên trái thanh trên, ví dụ "Tạo server". */
  toolbar?: ReactNode;
  /** Có thì hiện ô tìm kiếm, lọc theo chuỗi này (không phân biệt dấu). */
  searchText?: (row: T) => string;
  defaultSort?: SortState;
  loading?: boolean;
  /** Hiện thay cho bảng khi chưa có hàng nào, thường là `EmptyState`. */
  empty: ReactNode;
  selection?: TableSelection;
  /** Nút ở cuối ô tìm kiếm, ví dụ menu cài đặt tìm kiếm. */
  searchOptions?: ReactNode;
  /** Thay chữ mặc định "Tìm kiếm" của ô tìm. */
  searchLabel?: string;
  /** Khi truyền, ô tìm do trang quản lý giá trị (ví dụ reset khi đổi thư mục). */
  searchValue?: string;
  /** `false` khi tìm trực tiếp trên server, tránh lọc cục bộ trong lúc chờ debounce. */
  searchClientFilter?: boolean;
  /** Hiện vòng quay nhỏ trong ô tìm thay vì ẩn cả bảng. */
  searching?: boolean;
  /** Báo chuỗi đang gõ, cho trang tự tìm thêm (ví dụ tìm trên server). */
  onSearchChange?: (query: string) => void;
}

/**
 * Bảng dùng chung cho mọi danh sách (server, file, backup, người chơi…). Trang chỉ khai báo
 * cột và thao tác; giao diện, sắp xếp, tìm kiếm và bố cục trên điện thoại nằm ở đây.
 */
export function DataTable<T>(props: DataTableProps<T>) {
  const { label, columns, rows, rowKey, actions, toolbar, searchText, defaultSort, loading, empty, selection } = props;
  const { searchOptions, searchLabel, searchValue, searchClientFilter = true, searching = false, onSearchChange } = props;
  const { t, i18n } = useTranslation();
  const [internalQuery, setInternalQuery] = useState("");
  const query = searchValue ?? internalQuery;
  const [sort, setSort] = useState<SortState | null>(defaultSort ?? null);

  const visible = useMemo(() => {
    const filtered = searchText && searchClientFilter ? filterRows(rows, query, searchText) : rows;
    const sortValue = columns.find((column) => column.id === sort?.columnId)?.sortValue;
    return sort && sortValue ? sortRows(filtered, sortValue, sort.direction, i18n.language) : filtered;
  }, [rows, query, searchText, searchClientFilter, columns, sort, i18n.language]);

  // Đang tìm (có thể tìm trên server) mà không ra gì thì vẫn giữ bảng và ô tìm, chỉ báo không khớp.
  if (!loading && !searching && rows.length === 0 && query === "") return <>{empty}</>;
  const picking = selection !== undefined && selection.selected.size > 0;

  return (
    <div className={styles.surface}>
      {picking ? (
        <div className={cx(styles.toolbar, styles.bulk)}>
          <div className={styles.toolbarActions}>
            <span className={styles.bulkCount}>{t("ui.table.selected", { count: selection.selected.size })}</span>
            {selection.actions}
          </div>
          <Button size="sm" variant="ghost" icon={X} onClick={() => selection.onChange(new Set())}>
            {t("ui.table.clearSelection")}
          </Button>
        </div>
      ) : (toolbar || searchText) && (
        <div className={styles.toolbar}>
          <div className={styles.toolbarActions}>{toolbar}</div>
          {searchText && (
            <div className={styles.search}>
              <SearchInput
                value={query}
                onChange={(value) => {
                  if (searchValue === undefined) setInternalQuery(value);
                  onSearchChange?.(value);
                }}
                label={searchLabel ?? t("ui.table.search")}
                loading={searching}
                trailing={searchOptions}
              />
            </div>
          )}
        </div>
      )}
      {loading ? (
        <div className={styles.state}>
          <Spinner label={t("ui.loading")} />
        </div>
      ) : (
        <div className={styles.scroll}>
          <table className={styles.table}>
            <caption className="visually-hidden">{label}</caption>
            <thead>
              <tr>
                {selection && (
                  <th scope="col" className={styles.selectCol}>
                    <SelectAll keys={visible.map(rowKey)} selection={selection} />
                  </th>
                )}
                {columns.map((column) => (
                  <HeaderCell key={column.id} column={column} sort={sort} onSort={(id) => setSort(nextSort(sort, id))} />
                ))}
                {actions && (
                  <th scope="col" className={styles.actionsCol}>
                    <span className="visually-hidden">{t("ui.table.actions")}</span>
                  </th>
                )}
              </tr>
            </thead>
            <tbody>
              {visible.map((row) => (
                <BodyRow key={rowKey(row)} row={row} {...props} />
              ))}
            </tbody>
          </table>
        </div>
      )}
      {!loading && !searching && visible.length === 0 && <p className={styles.state}>{t("ui.table.noMatch", { query })}</p>}
      {!loading && <footer className={styles.footer}>{t("ui.table.count", { shown: visible.length, total: rows.length })}</footer>}
    </div>
  );
}

function HeaderCell<T>({ column, sort, onSort }: { column: Column<T>; sort: SortState | null; onSort: (id: string) => void }) {
  const active = sort?.columnId === column.id ? sort.direction : undefined;
  const sortable = column.sortValue !== undefined;
  const ariaSort = active === "asc" ? "ascending" : active === "desc" ? "descending" : sortable ? "none" : undefined;
  const Icon = active === "asc" ? ArrowUp : active === "desc" ? ArrowDown : ArrowUpDown;

  return (
    <th scope="col" aria-sort={ariaSort} className={cx(column.width && styles[column.width], styles[column.align ?? "start"])}>
      {sortable ? (
        <button type="button" className={styles.sort} onClick={() => onSort(column.id)}>
          {column.header}
          <Icon size={14} aria-hidden className={cx(styles.sortIcon, active && styles.sortActive)} />
        </button>
      ) : (
        column.header
      )}
    </th>
  );
}

function BodyRow<T>({ row, columns, actions, rowLabel, rowKey, selection, inlineActions = 2 }: DataTableProps<T> & { row: T }) {
  const { t } = useTranslation();
  const key = rowKey(row);
  const checked = selection?.selected.has(key) ?? false;
  return (
    <tr className={cx(checked && styles.picked)}>
      {selection && (
        <td className={styles.selectCell}>
          <input
            type="checkbox"
            className={styles.check}
            checked={checked}
            aria-label={t("ui.table.selectRow", { row: rowLabel(row) })}
            onChange={(e) => {
              const next = new Set(selection.selected);
              if (e.target.checked) next.add(key);
              else next.delete(key);
              selection.onChange(next);
            }}
          />
        </td>
      )}
      {columns.map((column) => (
        <td key={column.id} data-label={column.header} className={cx(styles[column.align ?? "start"], column.nowrap && styles.nowrap)}>
          {column.cell(row)}
        </td>
      ))}
      {actions && (
        <td className={styles.actionsCell}>
          <RowActions row={row} actions={actions} inline={inlineActions} name={rowLabel(row)} />
        </td>
      )}
    </tr>
  );
}

/** Chọn hoặc bỏ mọi hàng đang hiện (sau khi lọc); chọn một phần thì ô ở trạng thái lửng. */
function SelectAll({ keys, selection }: { keys: string[]; selection: TableSelection }) {
  const { t } = useTranslation();
  const ref = useRef<HTMLInputElement>(null);
  const count = keys.filter((key) => selection.selected.has(key)).length;
  const all = keys.length > 0 && count === keys.length;
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = count > 0 && !all;
  }, [count, all]);
  return (
    <input
      ref={ref}
      type="checkbox"
      className={styles.check}
      checked={all}
      aria-label={t("ui.table.selectAll")}
      onChange={() => selection.onChange(all ? new Set() : new Set(keys))}
    />
  );
}

function RowActions<T>({ row, actions, inline, name }: { row: T; actions: RowAction<T>[]; inline: number; name: string }) {
  const { t } = useTranslation();
  const shown = actions.filter((action) => !action.hidden?.(row));
  const direct = shown.slice(0, inline);
  const rest = shown.slice(inline);

  return (
    <div className={styles.actions}>
      {direct.map((action) => (
        <IconButton
          key={action.label}
          icon={action.icon}
          label={t("ui.table.actionOn", { action: action.label, row: name })}
          size="sm"
          tone={action.tone}
          disabled={action.disabled?.(row)}
          onClick={() => action.onSelect(row)}
        />
      ))}
      {rest.length > 0 && (
        <Menu
          size="sm"
          label={t("ui.table.moreFor", { row: name })}
          items={rest.map((action) => ({
            label: action.label,
            icon: action.icon,
            tone: action.tone,
            disabled: action.disabled?.(row),
            onSelect: () => action.onSelect(row),
          }))}
        />
      )}
    </div>
  );
}
