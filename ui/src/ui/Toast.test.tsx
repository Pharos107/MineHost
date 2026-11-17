import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TOAST_MS, Toaster, toast } from "./Toast";

afterEach(() => {
  act(() => toast.clear());
  vi.useRealTimers();
});

describe("Toast", () => {
  it("tự tắt sau 5 giây; toast đang chạy ở lại tới khi cập nhật xong việc", () => {
    vi.useFakeTimers();
    render(<Toaster />);
    let running = 0;
    act(() => {
      toast.show({ tone: "success", title: "Đã lưu" });
      running = toast.show({ sticky: true, title: "Đang nén" });
    });
    expect(screen.getByText("Đã lưu")).toBeTruthy();
    act(() => vi.advanceTimersByTime(TOAST_MS));
    expect(screen.queryByText("Đã lưu")).toBeNull();
    expect(screen.getByText("Đang nén")).toBeTruthy();

    act(() => toast.update(running, { tone: "success", title: "Đã nén xong", sticky: false }));
    expect(screen.getByText("Đã nén xong")).toBeTruthy();
    act(() => vi.advanceTimersByTime(TOAST_MS - 1));
    expect(screen.getByText("Đã nén xong")).toBeTruthy();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.queryByRole("region", { name: "Thông báo" })).toBeNull();
  });

  it("bấm nút đóng thì tắt ngay", async () => {
    render(<Toaster />);
    act(() => void toast.show({ tone: "danger", title: "Lỗi" }));
    await userEvent.click(screen.getByRole("button", { name: "Đóng" }));
    expect(screen.queryByText("Lỗi")).toBeNull();
  });
});
