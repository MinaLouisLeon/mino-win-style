import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { TopBar } from "./TopBar";
import { I18nProvider } from "../i18n";
import { applyLookTheme, watchShellLook } from "../lib/shell-look";
import { trace } from "./api";
import "./topbar.css";

// The bar is a page of its own, so it has to hear about the Look itself.
// Never unsubscribed: this lives as long as the window does.
watchShellLook((config) => applyLookTheme(config.active));

// WebView2 would otherwise offer a context menu on the bar. It has its own.
document.addEventListener("contextmenu", (event) => event.preventDefault());

const container = document.getElementById("root");
if (!container) throw new Error("topbar.html is missing #root");

trace("mounting");

createRoot(container).render(
  <StrictMode>
    <I18nProvider>
      <TopBar />
    </I18nProvider>
  </StrictMode>,
);
