import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

import i18n from "../i18n";

// Test viết theo câu tiếng Việt nên cố định ngôn ngữ, không phụ thuộc máy chạy test.
await i18n.changeLanguage("vi");

// jsdom chưa có `showModal`/`close` cho <dialog>: giả lập đủ để test mở, đóng và sự kiện `close`.
if (typeof HTMLDialogElement !== "undefined" && !HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    if (!this.hasAttribute("open")) return;
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
}

afterEach(() => {
  cleanup();
});
