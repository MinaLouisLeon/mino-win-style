import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Dock } from "./Dock";
import { applyJarvisTheme, watchJarvisMode } from "../lib/jarvis";
import "./dock.css";

// The dock is a page of its own, so it has to hear about JARVIS mode itself.
// Never unsubscribed: this lives as long as the window does.
watchJarvisMode((config) => applyJarvisTheme(config.enabled));

// WebView2 would otherwise offer "Save image as..." on every icon. The dock has
// its own menu; the browser one has no business here.
document.addEventListener("contextmenu", (event) => event.preventDefault());

const container = document.getElementById("root");
if (!container) throw new Error("dock.html is missing #root");

createRoot(container).render(
  <StrictMode>
    <Dock />
  </StrictMode>,
);
