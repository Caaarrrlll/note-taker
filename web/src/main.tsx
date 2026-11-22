import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";

import "@blocknote/core/fonts/inter.css";
import "@blocknote/mantine/style.css";
import "./styles.css";

import App from "./App";
import { flushPendingSaves } from "./pendingSaves";

// Finish pending saves before the window closes. Tauri waits for this handler,
// then closes the window (needs the core:window:allow-destroy permission).
void getCurrentWindow().onCloseRequested(async () => {
  await flushPendingSaves();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
