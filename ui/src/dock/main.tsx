import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Dock } from "./Dock";
import "./dock.css";

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
