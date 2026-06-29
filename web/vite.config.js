import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// Dev server proxies /api and /health to the local AgentSpan gateway so the
// dashboard can run on a separate port without CORS during development.
export default defineConfig({
    plugins: [react()],
    server: {
        port: 5173,
        proxy: {
            "/api": "http://localhost:8080",
            "/health": "http://localhost:8080",
        },
    },
    build: {
        chunkSizeWarningLimit: 600,
        rollupOptions: {
            output: {
                manualChunks(id) {
                    // Split only the heavy charting library into its own chunk; keeping a
                    // single one-way split (app -> charts) avoids circular-chunk warnings.
                    if (id.includes("node_modules") && (id.includes("recharts") || id.includes("d3-"))) {
                        return "charts";
                    }
                    return undefined;
                },
            },
        },
    },
});
