import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// https://vite.dev/config/
export default defineConfig({
  plugins: [tailwindcss(), react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/v1": {
        target: "http://127.0.0.1:43891",
        changeOrigin: true,
      },
      "/.well-known": {
        target: "http://127.0.0.1:43891",
        changeOrigin: true,
      },
    },
  },
  test: {
    exclude: ["node_modules", "dist", ".idea", ".git", ".cache", "e2e/*"],
  },
  build: {
    chunkSizeWarningLimit: 1000,
  },
});
