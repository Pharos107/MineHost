import { describe, expect, it } from "vitest";

import en from "./en.json";
import vi from "./vi.json";

function keys(value: unknown, prefix = ""): string[] {
  if (typeof value !== "object" || value === null) return [prefix];
  return Object.entries(value).flatMap(([key, child]) => keys(child, prefix ? `${prefix}.${key}` : key));
}

describe("i18n", () => {
  it("vi.json và en.json có đủ khóa như nhau", () => {
    expect(keys(en).sort()).toEqual(keys(vi).sort());
  });
});
