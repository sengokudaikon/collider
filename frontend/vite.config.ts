import path from "path"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    // Enable build optimizations
    rollupOptions: {
      output: {
        // Code splitting for better caching
        manualChunks: {
          vendor: ['react', 'react-dom'],
          ui: ['@radix-ui/react-avatar', '@radix-ui/react-table']
        }
      }
    },
    // Target modern browsers for smaller bundles
    target: 'es2022',
    // Enable minification
    minify: 'esbuild',
    // Source maps for debugging
    sourcemap: true,
    // Report compressed file sizes
    reportCompressedSize: true,
    // Chunk size warning limit
    chunkSizeWarningLimit: 1000
  },
  // Preview server configuration
  preview: {
    port: 3000,
    host: true
  }
})
