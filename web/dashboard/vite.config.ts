import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

const CONTROL_PLANE_PROXY_PATTERN =
  "^/(health|doctor|diagnose|host|profiles|instances|leases|tabs|browser|artifacts|runs|scenarios|events|replay|trace|recording|tools)";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const target =
    env.PENGU_MESH_DASHBOARD_API_ORIGIN?.trim() || "http://127.0.0.1:43127";
  const proxy = {
    [CONTROL_PLANE_PROXY_PATTERN]: {
      target,
      changeOrigin: false,
    },
  };

  return {
    plugins: [react()],
    server: {
      port: 5173,
      proxy,
    },
    preview: {
      port: 4173,
      proxy,
    },
  };
});
