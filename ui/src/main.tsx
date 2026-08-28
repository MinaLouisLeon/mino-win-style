import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { I18nProvider } from "./i18n";
import "./styles.css";

const container = document.getElementById("root");
if (!container) throw new Error("index.html is missing #root");

createRoot(container).render(
  <StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </StrictMode>,
);
