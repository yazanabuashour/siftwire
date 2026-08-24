import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

export default defineConfig({
  plugins: [tailwindcss(), react()],
  server: {
    host: "127.0.0.1",
    port: 5175,
    strictPort: true,
    proxy: {
      "/api": {
        target: process.env["API_PROXY_TARGET"] ?? "http://127.0.0.1:8790",
        changeOrigin: true,
      },
    },
  },
})
