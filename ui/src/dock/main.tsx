import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Dock } from "./Dock";
import "./dock.css";

const container = document.getElementById("root");
if (!container) throw new Error("dock.html is missing #root");

createRoot(container).render(
  <StrictMode>
    <Dock />
  </StrictMode>,
);
