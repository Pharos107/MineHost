import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const CONTROLLER = "http://127.0.0.1:7878";

// Khi chạy `npm run dev`, request `/api` được chuyển tới controller đang chạy ở cổng mặc định.
// Controller chỉ nhận request thay đổi dữ liệu có Origin của chính nó, nên proxy ghi đè
// Origin của Vite (cổng 5173) bằng địa chỉ của controller.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: CONTROLLER,
        changeOrigin: true,
        headers: { Origin: CONTROLLER },
      },
    },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
