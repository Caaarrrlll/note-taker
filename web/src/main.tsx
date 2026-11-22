import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@blocknote/core/fonts/inter.css";
import "@blocknote/mantine/style.css";
import "./styles.css";

import App from "./App";
import { IS_WEB } from "./api";
import { flushPendingSaves, hasSavesInFlight } from "./pendingSaves";

if (IS_WEB) {
  // Browser: start pending saves when the tab is hidden, and ask before leaving
  // while a save is still on its way (the browser shows its own "leave page?").
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") void flushPendingSaves();
  });
  window.addEventListener("beforeunload", (e) => {
    void flushPendingSaves();
    if (hasSavesInFlight()) e.preventDefault();
  });
} else {
  // Desktop: finish pending saves before the window closes. Tauri waits for this
  // handler, then closes the window (needs the core:window:allow-destroy permission).
  void import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
    getCurrentWindow().onCloseRequested(async () => {
      await flushPendingSaves();
    }),
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
