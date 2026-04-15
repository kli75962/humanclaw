import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "url";
import path from "path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const host = process.env.TAURI_DEV_HOST;
// Mobile (android/ios) dev uses a different port so desktop and mobile
// can run simultaneously without conflicting.
const isMobile = !!host;
const devPort = isMobile ? 1422 : 1420;
const hmrPort = isMobile ? 1423 : 1421;

// https://vite.dev/config/
// pixi-live2d-display does `import * as PIXI from 'pixi.js'` and then writes
// PIXI.live2d = {}.  A `* as` namespace is always frozen (ES spec), so that
// assignment throws in strict mode.  We transform the offending import to a
// default import — Vite's CJS→ESM interop turns the default into the raw
// `module.exports` object, which IS mutable.
function pixiLive2dCompatPlugin() {
  return {
    name: 'pixi-live2d-compat',
    transform(code: string, id: string) {
      if (!id.includes('pixi-live2d-display')) return;
      let result = code.replace(
        /import\s+\*\s+as\s+PIXI\s+from\s+['"]pixi\.js['"]/g,
        'import PIXI from "pixi.js"',
      );
      // Cubism Core 6.x moved renderOrders from model.drawables.renderOrders
      // to model.renderOrders directly. Patch all call sites so both versions work.
      result = result.replace(
        /this\._model\.drawables\.renderOrders/g,
        '(this._model.renderOrders ?? this._model.drawables.renderOrders)',
      );
      return result;
    },
  };
}

export default defineConfig(async () => ({
  root: "src",
  publicDir: "../public",
  plugins: [react(), pixiLive2dCompatPlugin()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: devPort,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: hmrPort,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  resolve: {
    // Force pixi.js to the CJS build so its exports are a plain mutable object
    // rather than a frozen ESM namespace. pixi-live2d-display writes to
    // PIXI.live2d at load time and throws "readonly property" on the ESM version.
    alias: {
      'pixi.js': path.resolve(__dirname, 'node_modules/pixi.js/dist/pixi.js'),
    },
    dedupe: ['pixi.js'],
  },
  optimizeDeps: {
    include: [
      'pixi.js',
      '@pixi/app',
      '@pixi/ticker',
      'pixi-live2d-display',
      'pixi-live2d-display/cubism4',
    ],
    esbuildOptions: {
      plugins: [
        {
          name: 'cubism6-compat',
          setup(build) {
            // Core 6.x moved renderOrders from model.drawables to model directly.
            // Patch all pixi-live2d-display source files during esbuild pre-bundling.
            build.onLoad({ filter: /pixi-live2d-display/ }, async (args) => {
              const fs = await import('fs/promises');
              let contents = await fs.readFile(args.path, 'utf8');
              contents = contents.replace(
                /this\._model\.drawables\.renderOrders/g,
                '(this._model.renderOrders ?? this._model.drawables.renderOrders)',
              );
              return { contents, loader: 'js' };
            });
          },
        },
      ],
    },
  },

  // Multi-page: main app + live2d overlay window
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, "src/index.html"),
        live2d: path.resolve(__dirname, "src/live2d.html"),
      },
    },
  },
}));
