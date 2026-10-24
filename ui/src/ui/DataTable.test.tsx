import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Play, Square, SquareTerminal, Trash2 } from "lucide-react";
import { describe, expect, it, vi } from "vitest";

import { DataTable, type Column, type RowAction } from ".";
import { filterRows, foldText, sortRows } from "./table-utils";

interface Row {
  id: string;
  name: string;
  on: boolean;
}

const ROWS: Row[] = [
  { id: "1", name: "Server 10", on: true },
  { id: "2", name: "Máy chủ Đức", on: false },
  { id: "3", name: "Server 2", on: false },
];

const COLUMNS: Column<Row>[] = [{ id: "name", header: "Tên", cell: (row) => row.name, sortValue: (row) => row.name }];

function renderTable(actions: RowAction<Row>[] = [], rows = ROWS) {
  render(
    <DataTable
      label="Server"
      columns={COLUMNS}
      rows={rows}
      rowKey={(row) => row.id}
      rowLabel={(row) => row.name}
      actions={actions}
      searchText={(row) => row.name}
      empty={<p>trống</p>}
    />,
  );
}

const names = () => screen.getAllByRole("row").slice(1).map((row) => row.querySelector("td")?.textContent);

describe("table-utils", () => {
  it("tìm không phân biệt dấu và hoa thường, kể cả chữ đ", () => {
    expect(foldText("Máy chủ Đức")).toBe("may chu duc");
    expect(filterRows(ROWS, "may chu duc", (row) => row.name).map((row) => row.id)).toEqual(["2"]);
  });

  it("sắp xếp hiểu số trong chuỗi", () => {
    const sorted = sortRows(ROWS, (row) => row.name, "asc", "vi").map((row) => row.name);
    expect(sorted).toEqual(["Máy chủ Đức", "Server 2", "Server 10"]);
  });
});

describe("DataTable", () => {
  it("bấm tiêu đề cột để sắp tăng rồi giảm, aria-sort đổi theo", async () => {
    renderTable();
    const header = screen.getByRole("columnheader", { name: /Tên/ });
    expect(header.getAttribute("aria-sort")).toBe("none");

    await userEvent.click(within(header).getByRole("button"));
    expect(header.getAttribute("aria-sort")).toBe("ascending");
    expect(names()).toEqual(["Máy chủ Đức", "Server 2", "Server 10"]);

    await userEvent.click(within(header).getByRole("button"));
    expect(header.getAttribute("aria-sort")).toBe("descending");
    expect(names()).toEqual(["Server 10", "Server 2", "Máy chủ Đức"]);
  });

  it("lọc theo ô tìm kiếm và báo khi không có kết quả", async () => {
    renderTable();
    const search = screen.getByRole("searchbox", { name: "Tìm kiếm" });
    await userEvent.type(search, "duc");
    expect(names()).toEqual(["Máy chủ Đức"]);
    expect(screen.getByText("Hiển thị 1/3")).toBeTruthy();

    await userEvent.type(search, "xyz");
    expect(screen.getByText("Không có kết quả cho “ducxyz”.")).toBeTruthy();
  });

  it("chưa có hàng nào thì chỉ hiện phần empty", () => {
    renderTable([], []);
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.getByText("trống")).toBeTruthy();
  });

  it("thao tác: icon trước, phần dư vào menu, ẩn theo từng hàng, nhãn kèm tên hàng", async () => {
    const remove = vi.fn();
    renderTable([
      { label: "Console", icon: SquareTerminal, onSelect: vi.fn() },
      { label: "Bật", icon: Play, onSelect: vi.fn(), hidden: (row) => row.on },
      { label: "Tắt", icon: Square, onSelect: vi.fn(), hidden: (row) => !row.on },
      { label: "Xóa", icon: Trash2, tone: "danger", onSelect: remove },
    ]);

    expect(screen.getByRole("button", { name: "Tắt: Server 10" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Bật: Server 10" })).toBeNull();
    expect(screen.getByRole("button", { name: "Bật: Server 2" })).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Thao tác khác cho Server 2" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Xóa" }));
    expect(remove).toHaveBeenCalledWith(ROWS[2]);
  });
});
