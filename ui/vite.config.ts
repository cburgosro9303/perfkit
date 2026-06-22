import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri espera un puerto fijo y no debe limpiar la pantalla del proceso Rust.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: false,
  },
});
