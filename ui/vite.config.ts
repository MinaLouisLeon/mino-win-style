import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The port is fixed because tauri.conf.json points at it. `strictPort` makes a
// clash fail loudly instead of silently serving the app somewhere Tauri is not
// looking.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
  build: {
    target: "chrome110",
    sourcemap: true,
    rollupOptions: {
      // Four pages, four windows: the settings window, the dock, the overlay
      // and the bar. They share nothing but the toolchain and a couple of
      // helpers, which is why each has its own entry rather than being a route
      // inside the main app.
      // Relative to the Vite root, which keeps Node types out of this file.
      input: {
        main: "index.html",
        dock: "dock.html",
        hud: "hud.html",
        topbar: "topbar.html",
      },
    },
  },
});
