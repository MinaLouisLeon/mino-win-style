import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Hud } from "./Hud";
import { I18nProvider } from "../i18n";
import { trace } from "./api";
import "./hud.css";

// The overlay is click-through, so a context menu should be impossible — but
// WebView2 can still raise one from the keyboard, and a "Save image as..." over
// the arc reactor would be a strange thing to find on a HUD.
document.addEventListener("contextmenu", (event) => event.preventDefault());

const container = document.getElementById("root");
if (!container) throw new Error("hud.html is missing #root");

trace("mounting");

createRoot(container).render(
  <StrictMode>
    <I18nProvider>
      <Hud />
    </I18nProvider>
  </StrictMode>,
);
