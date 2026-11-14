import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import i18n from "../i18n";
import { ProjectCard } from "./ProjectCard";

describe("ProjectCard", () => {
  it("hiện tên, mô tả, lượt tải theo ngôn ngữ, nhãn loader và phiên bản", async () => {
    await i18n.changeLanguage("vi");
    const { rerender } = render(
      <ProjectCard
        title="Sodium"
        description="Tối ưu render cho Minecraft."
        downloads={1_500_000}
        loaders={["Fabric", "Quilt"]}
        version="1.21.4"
      />,
    );
    expect(screen.getByText("Sodium")).toBeTruthy();
    expect(screen.getByText("Tối ưu render cho Minecraft.")).toBeTruthy();
    expect(screen.getByText("Fabric")).toBeTruthy();
    expect(screen.getByText("Quilt")).toBeTruthy();
    expect(screen.getByText("1.21.4")).toBeTruthy();
    const expectedVi = new Intl.NumberFormat("vi", { notation: "compact", maximumFractionDigits: 1 }).format(1_500_000);
    const downloadsVi = screen.getByText(expectedVi.replace(/\u00a0/g, " "));
    expect(downloadsVi.getAttribute("aria-label")).toBe(`${expectedVi} lượt tải`);

    await i18n.changeLanguage("en");
    rerender(
      <ProjectCard
        title="Sodium"
        description="Tối ưu render cho Minecraft."
        downloads={1_500_000}
        loaders={["Fabric"]}
        version="1.21.4"
      />,
    );
    const expectedEn = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(1_500_000);
    const downloadsEn = screen.getByText(expectedEn.replace(/\u00a0/g, " "));
    expect(downloadsEn.getAttribute("aria-label")).toBe(`${expectedEn} downloads`);
    await i18n.changeLanguage("vi");
  });

  it("bấm chọn được khi có onSelect", async () => {
    const onSelect = vi.fn();
    render(
      <ProjectCard
        title="Lithium"
        description="Tối ưu hiệu năng server."
        downloads={500_000}
        loaders={["Fabric"]}
        version="1.21.4"
        selected
        onSelect={onSelect}
      />,
    );
    const button = screen.getByRole("button", { name: /Lithium/ });
    expect(button.getAttribute("aria-pressed")).toBe("true");
    await userEvent.click(button);
    expect(onSelect).toHaveBeenCalledTimes(1);
  });
});
