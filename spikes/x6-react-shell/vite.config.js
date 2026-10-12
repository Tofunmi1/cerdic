import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// X6 (throwaway): React and Vite come from the repository root's node_modules.
export default defineConfig({
  plugins: [react()],
  server: { fs: { strict: false } },
});
