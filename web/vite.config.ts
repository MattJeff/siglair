import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

const API = 'http://localhost:8080';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    // L'éditeur importe src/render/anim.css (contrat §3.2 : une seule copie des presets,
    // celle du renderer Rust). Sans ça le serveur de développement refuse de la servir.
    fs: { allow: ['..'] },
    // /s et /c sont les URL publiques servies aux clients mail : elles doivent
    // fonctionner en dev, sinon l'aperçu du GIF publié est cassé en local.
    // Clés en expression régulière, pas en préfixe : un simple '/s' capturerait
    // /src/main.tsx et casserait tout le serveur de développement.
    proxy: {
      '^/api/': API,
      '^/s/': API,
      '^/c/': API,
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
    rollupOptions: {
      output: {
        // Un seul chunk pour le socle : il ne change presque jamais, le cache le garde.
        // Le reste du découpage vient des import() paresseux de App.tsx.
        codeSplitting: {
          groups: [{ name: 'react', test: /node_modules[\\/](react|react-dom|react-router)/ }],
        },
      },
    },
  },
});
