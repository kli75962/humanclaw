# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

Template created! To get started run:
  cd phoneclaw
  bun install
  bun run tauri android init

For Desktop development, run:
  bun run tauri dev
  WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11 bun run tauri dev

For Android development, run:
  bun run tauri android dev

debug:
  adb logcat | grep PhoneControlPlugin
