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
  },
});
