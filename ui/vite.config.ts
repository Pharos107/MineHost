import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Khi chạy `npm run dev`, request `/api` được chuyển tới controller đang chạy ở cổng mặc định.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: { "/api": "http://127.0.0.1:7878" },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
