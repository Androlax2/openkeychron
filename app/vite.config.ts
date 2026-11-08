import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

// Tauri serves the built files from disk, so there is no server: every page is prerendered.
export default defineConfig(({ mode }) => ({
  plugins: [sveltekit({ adapter: adapter() })],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  // Vitest runs in the "test" mode. Its component tests mount into jsdom, so Svelte must
  // resolve to its browser build, not the server one.
  resolve: mode === "test" ? { conditions: ["browser"] } : undefined,
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
}));
