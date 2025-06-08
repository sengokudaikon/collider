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
    
    rollupOptions: {
      output: {
        
        manualChunks: {
          vendor: ['react', 'react-dom'],
          ui: ['@radix-ui/react-avatar', '@radix-ui/react-table']
        }
      }
    },
    
    target: 'es2022',
    
    minify: 'esbuild',
    
    sourcemap: true,
    
    reportCompressedSize: true,
    
    chunkSizeWarningLimit: 1000
  },
  
  preview: {
    port: 3000,
    host: true
  }
})
