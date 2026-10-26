/// <reference types="vitest/config" />
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const DEV_PORT = 5173;
const CONTROLLER = "http://127.0.0.1:7878";
const DEV_ORIGINS = [`http://localhost:${DEV_PORT}`, `http://127.0.0.1:${DEV_PORT}`];

/**
 * Controller chỉ nhận request thay đổi dữ liệu có Origin của chính nó. Khi phát triển, UI
 * chạy ở cổng của Vite, nên đổi Origin sang địa chỉ controller — nhưng chỉ khi request thật
 * sự đến từ trang của Vite. Origin khác giữ nguyên để controller vẫn chặn được trang lạ.
 */
function rewriteDevOrigin(req: { headers: { origin?: string } }) {
  const origin = req.headers.origin;
  if (origin !== undefined && DEV_ORIGINS.includes(origin)) {
    req.headers.origin = CONTROLLER;
  }
}

// Khi chạy `npm run dev`, request `/api` được chuyển tới controller đang chạy ở cổng mặc định.
export default defineConfig({
  plugins: [react()],
  server: {
    port: DEV_PORT,
    proxy: {
      "/api": {
        target: CONTROLLER,
        changeOrigin: true,
        // Vite gọi `bypass` cho cả request nâng cấp WebSocket, nên Origin cũng được đổi ở đó.
        ws: true,
        bypass: (req) => {
          rewriteDevOrigin(req);
          return undefined;
        },
      },
    },
  },
  build: { outDir: "dist", emptyOutDir: true },
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    // `e2e/` là test Playwright chạy với binary thật, không phải Vitest.
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
