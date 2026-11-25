import { defineConfig } from "vite";

export default defineConfig({
  root: ".",

  build: {
    outDir: "dist",
    emptyOutDir: true,
    // Minify with esbuild (fastest; terser is slower for marginal gain)
    minify: "esbuild",
    // Drop console/debugger statements in production
    target: "esnext",
    rollupOptions: {
      output: {
        // Single chunk — avoids network round-trips for tiny adapter
        manualChunks: undefined,
        // Minimal asset filenames
        entryFileNames: "assets/[name].[hash].js",
        chunkFileNames: "assets/[name].[hash].js",
        assetFileNames: "assets/[name].[hash][extname]",
      },
    },
  },

  // Vite dev server on the port Tauri expects
  server: {
    port: 5173,
    strictPort: true,
  },

  clearScreen: false,

  esbuild: {
    // Drop console and debugger in production builds
    drop: process.env.NODE_ENV === "production" ? ["console", "debugger"] : [],
  },
});
