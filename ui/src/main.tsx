import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode, type ComponentType } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import "./i18n";
import "./ui/base.css";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false } },
});

/** `#/ui` chỉ có khi chạy `npm run dev`; bản build bỏ hẳn nhánh này và file gallery. */
async function pickRoot(): Promise<ComponentType> {
  if (import.meta.env.DEV && window.location.hash === "#/ui") {
    return (await import("./dev/Gallery")).Gallery;
  }
  return App;
}

const root = document.getElementById("root");
if (root) {
  const Root = await pickRoot();
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <Root />
      </QueryClientProvider>
    </StrictMode>,
  );
}
