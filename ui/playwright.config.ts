import { defineConfig, devices } from "@playwright/test";

const PORT = process.env.MINEHOST_E2E_PORT ?? "7879";
const BASE = `http://127.0.0.1:${PORT}`;

/**
 * Test E2E chạy với binary `minehost` thật ở chế độ e2e (spec P2, mục 9): build UI trước
 * (`npm run build`) vì controller phục vụ `ui/dist`, rồi build Rust như trong `e2e/start-minehost.mjs`.
 */
export default defineConfig({
  testDir: "e2e",
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: BASE,
    locale: "vi-VN",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "node e2e/start-minehost.mjs",
    url: `${BASE}/api/v1/health`,
    reuseExistingServer: false,
    timeout: 60_000,
    env: { MINEHOST_E2E_PORT: PORT },
  },
});
