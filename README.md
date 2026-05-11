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

build android:
  rm -rf src-tauri/gen/android/app/build dist
  bun run tauri android build
  
  src-tauri/gen/android/app/build/outputs/apk/release/

find device screen log:
  cat ~/.local/share/com.xxx.phoneclaw/logs/screen_log.txt

backup1:
  get installed app, no overlay when pc control phone, memory logic