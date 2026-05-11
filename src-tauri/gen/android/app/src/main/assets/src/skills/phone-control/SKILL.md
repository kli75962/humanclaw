---
name: phone-control
description: Control any Android app.
compatibility: PhoneClaw (Tauri v2 Android agent)
---
## Principal
Keep ASKING. ONLY start proceed request have gather enough information. Don't run any tools until you have fully understand user's request.

When you want to gather more information from user, use ask_user() tool.

Only ask user if the information you want to gather cannot obtain from using the tools you already have.
For example, you don't need to ask user if he already install the app cause you can check using `get_installed_apps()` tool.

See [phone-login guide](skills/phone-login/SKILL.md) if app needs to login to use.

## App Discovery

Call `get_installed_apps()` when user request is related with installed apps on the phone or you need to check if the app is installed on the phone before executing any other tools.

Do NOT call `get_installed_apps()` on every turn. Call it once, find the target app, then proceed.

## Screen Output Format

`get_screen()` returns an accessibility tree. Interactive elements include `@(x,y)` coordinates.

| Prefix | Action |
|--------|--------|
| `[button] Label @(x,y)` | `tap(description: "Label")` |
| `[input] Label @(x,y)` | `tap` to focus → `type_text` |
| `[on] Label` | Tap to turn OFF |
| `[off] Label` | Tap to turn ON |
| `Label` (no prefix) | Read-only text |

Always call `get_screen` **before and after** every interaction. Never assume success without verifying.
Only call `get_screen_deep` **when** your get stuck and unable to continue for next step.
**Output plain text only. NEVER use raw markdown symbols (`#`, `##`, `**`, `*`, `---`).**
---

## Duplicate Button Labels — ALWAYS Use Coordinates

**NEVER use `tap(description: ...)` when the same label appears more than once on screen.**
`tap(description: ...)` always taps the FIRST match — which is almost always the WRONG one.

**Rule: Before tapping any button, scan ALL elements on screen for duplicate labels.**
If duplicates exist → identify which entry belongs to the target app/item by reading the text directly ABOVE the button in the tree → use `tap(x, y)` with that button's exact coordinates.

Example — Play Store, two "Install" buttons:
```
rednote                          ← NOT the target
  [button] Install @(318,178)    ← WRONG button (first match)
Instagram                        ← target app
  [button] Install @(318,350)    ← CORRECT button
```
→ Task: install Instagram → `tap(x: 318, y: 350)` — NEVER `tap(description: "Install")`

---

## Decision Loop (run after every get_screen)

1. **Popup?** → Dismiss (see above) → `get_screen` → restart loop
2. **Duplicate labels?** → Use `tap(x, y)` with the correct item's coordinates (see above)
3. **Direct match** `[button]`/`[on]`/`[off]` (unique) → `tap(description: "Label")`
4. **Search bar** `[input]` → tap → `type_text(keyword)` → `press_key(enter)` → `get_screen` *(prefer over scrolling)*
5. **Fuzzy match** — related word / parent category → tap → `get_screen` → re-run loop
6. **Scroll** — `swipe(direction: "up")` → `get_screen`, up to 3 times
7. **Backtrack** — `press_key(key: "back")` → `get_screen`, up to 3 levels
8. **Give up** — report visible `[button]` sections to user

Loop ends only when: toggle state confirmed · input submitted and results visible · action verified via `get_screen`.

---

## Tool Order

```
launch_app(package_name)   ← open app
get_screen()               ← read UI
tap / type_text / swipe    ← interact
press_key                  ← back / home / enter
get_screen()               ← verify
```

---

## FLAG_SECURE Apps (get_screen returns [GESTURE_MAP_HINT])

Some apps (React Native, FLAG_SECURE) block accessibility tree access — get_screen returns almost nothing and screenshots are black.

**MANDATORY protocol — you MUST follow every step exactly. Do NOT respond with plain text asking the user to reply again. Execute the tools immediately.**

### Step 1 — Search for existing maps (REQUIRED, call immediately)
When output contains `[GESTURE_MAP_HINT]`, your FIRST action MUST be to call:
```
search_gesture_maps(app_package="<pkg>")
```
Do not skip this. Do not ask the user anything first.

### Step 2a — Maps found → replay immediately
Call `replay_gesture_map(app_package, name)`.
`fill_credential` steps auto-fill stored credentials. No further action needed after replay.

### Step 2b — No maps found → ask user via tool (REQUIRED, use ask_user())
You MUST call `ask_user()` with a question like:
```
"This app blocks screen reading. I can record your gestures to automate it later. Do you want to record the steps now?"
```
Do NOT just write a text message asking the user — you MUST use `ask_user()` so the UI captures the answer properly.

### Step 3 — Based on user's answer (act immediately, do not wait)
- **User says yes** → Call `start_gesture_recording()` immediately, then instruct the user to perform the actions on their phone, then call `stop_gesture_recording(app_package, name, description)` when done.
- **User says no** → Inform the user that automated control is not possible for this app without a recorded gesture map.
