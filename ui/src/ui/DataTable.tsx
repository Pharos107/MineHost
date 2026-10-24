import { ArrowDown, ArrowUp, ArrowUpDown, type LucideIcon } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { IconButton } from "./Button";
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
}

/**
 * Bảng dùng chung cho mọi danh sách (server, file, backup, người chơi…). Trang chỉ khai báo
 * cột và thao tác; giao diện, sắp xếp, tìm kiếm và bố cục trên điện thoại nằm ở đây.
 */
export function DataTable<T>(props: DataTableProps<T>) {
  const { label, columns, rows, rowKey, actions, toolbar, searchText, defaultSort, loading, empty } = props;
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortState | null>(defaultSort ?? null);

  const visible = useMemo(() => {
    const filtered = searchText ? filterRows(rows, query, searchText) : rows;
    const sortValue = columns.find((column) => column.id === sort?.columnId)?.sortValue;
    return sort && sortValue ? sortRows(filtered, sortValue, sort.direction, i18n.language) : filtered;
  }, [rows, query, searchText, columns, sort, i18n.language]);

  if (!loading && rows.length === 0) return <>{empty}</>;

  return (
    <div className={styles.surface}>
      {(toolbar || searchText) && (
        <div className={styles.toolbar}>
          <div className={styles.toolbarActions}>{toolbar}</div>
          {searchText && (
            <div className={styles.search}>
              <SearchInput value={query} onChange={setQuery} label={t("ui.table.search")} />
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
      {!loading && visible.length === 0 && <p className={styles.state}>{t("ui.table.noMatch", { query })}</p>}
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

function BodyRow<T>({ row, columns, actions, rowLabel, inlineActions = 2 }: DataTableProps<T> & { row: T }) {
  return (
    <tr>
      {columns.map((column) => (
        <td key={column.id} data-label={column.header} className={styles[column.align ?? "start"]}>
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
