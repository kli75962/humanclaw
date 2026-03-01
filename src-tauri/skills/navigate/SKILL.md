---
name: navigate
description: How to read get_screen output, interact with UI elements (tap, type, scroll, toggle), and navigate through any app to find and operate a target. Use whenever the task involves opening an app, searching within it, or changing a setting.
compatibility: PhoneClaw (Tauri v2 Android agent)
---

## Screen Output Format

`get_screen()` returns the accessibility tree as indented plain text. Each element is prefixed with its role:

| Prefix | Meaning | How to use |
|--------|---------|------------|
| `[button] Label` | Tappable element | `tap(description: "Label")` |
| `[input] Label` | Text field | `tap` to focus → `type_text` |
| `[on] Label` | Toggle currently ON | `tap(description: "Label")` to turn OFF |
| `[off] Label` | Toggle currently OFF | `tap(description: "Label")` to turn ON |
| `Label` (no prefix) | Non-interactive text / heading | Read only |

**Always tap using the EXACT label text shown after the prefix.**
- `[button] Display & touch` → `tap(description: "Display & touch")`
- `[on] Bluetooth` → `tap(description: "Bluetooth")`
- `[input] Search settings` → tap it then `type_text`

---

## Analyzing get_screen — Decision Priority

After every `get_screen`, evaluate in this order:

**1. Direct match** — is there a `[button]` / `[on]` / `[off]` whose label matches the goal?
→ Tap it immediately.

**2. Search bar** — is there an `[input]` labeled `Search`, `Find`, `Filter`, or `type URL`?
→ Tap it → `type_text(keyword)` → `press_key(enter)` → `get_screen`.
→ **Always prefer this over scrolling.**

**3. Fuzzy match** — any `[button]` with a related word / synonym / parent category?
→ Examples: "bluetooth" → `Connected devices` / "dark mode" → `Display & touch` / "volume" → `Sound`
→ Tap the best match → **immediately call `get_screen` on the new screen** → re-run this decision loop from step 1.
→ **Do NOT stop here.** Entering a sub-screen is a navigation step, not the goal.

**4. Scroll** — nothing matched on current screen.
→ `swipe(direction: "up")` → `get_screen` → repeat up to 3 times.

**5. Backtrack** — reached bottom with no match.
→ `press_key(key: "back")` → `get_screen` → re-analyze from parent screen (up to 3 levels).

**6. Give up** — exhausted all paths.
→ Report to user what `[button]` sections ARE visible.

> **The loop only ends when:**
> - A toggle `[on]`/`[off]` state is confirmed changed, OR
> - An input was submitted and results are visible, OR
> - An action is confirmed via `get_screen` and reported to the user.

---

## Tool Execution Order

```
launch_app(package_name)          ← open target app
get_screen()                      ← confirm app open, read UI
tap / type_text / swipe           ← interact
press_key                         ← navigate (back / home / enter)
get_screen()                      ← verify result
```

Always call `get_screen` **before and after** every tap or type_text.
Never assume a step succeeded without verifying.

---

## Example A — Open app and search within it

User: "open YouTube and search cat"

```
tool: launch_app(package_name: "com.google.android.youtube")
result: "App launched."

tool: get_screen()
result:
  [button] Home
  [button] Search
  [button] Shorts
// [button] Search is visible → tap it.

tool: tap(description: "Search")
result: "Tap performed."

tool: get_screen()
result:
  [input] Search YouTube
  [button] Cancel
// [input] focused → type the query.

tool: type_text(text: "cat")
result: "Text typed."

tool: press_key(key: "enter")
result: "Key pressed."

tool: get_screen()
result:
  [button] Cute Cats Compilation  10M views
  [button] Funny Cats 2024  5M views
// Results confirmed.

Final reply: "Search results for 'cat' are now showing on YouTube."
```

---

## Example B — Navigate settings through a sub-screen to find a toggle

User: "turn off dark mode"

```
tool: launch_app(package_name: "com.android.settings")
result: "App launched."

tool: get_screen()
result:
  Settings
  [input] Search settings
  [button] Network & internet
  [button] Connected devices
  [button] Display & touch        ← fuzzy match for "dark mode"
  [button] Sound
// No direct match. [input] Search exists — use it.

tool: tap(description: "Search settings")
result: "Tap performed."

tool: type_text(text: "dark mode")
result: "Text typed."

tool: get_screen()
result:
  [button] Dark theme             ← direct match found in results.
// Tap it.

tool: tap(description: "Dark theme")
result: "Tap performed."

tool: get_screen()
result:
  Display
  [on] Dark theme                 ← toggle is ON → tap to turn off.

tool: tap(description: "Dark theme")
result: "Tap performed."

tool: get_screen()
result:
  Display
  [off] Dark theme                ← confirmed OFF. Loop ends here.

Final reply: "Dark mode has been turned off."
```

**If there is no search bar, navigate via fuzzy match sub-screens:**

```
tool: get_screen()
result:
  Settings
  [button] Network & internet
  [button] Connected devices
  [button] Display & touch        ← fuzzy match. Tap and continue on new screen.

tool: tap(description: "Display & touch")
result: "Tap performed."

tool: get_screen()                ← MUST call get_screen after every tap.
result:
  Display & touch
  [button] Brightness
  [button] Colour scheme
  [on] Dark theme                 ← direct match, toggle is ON.
// Tap to turn off.

tool: tap(description: "Dark theme")
result: "Tap performed."

tool: get_screen()
result:
  [off] Dark theme                ← confirmed OFF.

Final reply: "Dark mode has been turned off."
```

```
tool: launch_app(package_name: "com.android.settings")
result: "App launched."

tool: get_screen()
result:
  Settings
  [input] Search settings        ← search bar found! use it.
  [button] Network & internet
  [button] Connected devices

tool: tap(description: "Search settings")
result: "Tap performed."

tool: type_text(text: "bluetooth")
result: "Text typed."

tool: get_screen()
result:
  [button] Bluetooth
  [button] Connected devices
// tap best match.

tool: tap(description: "Bluetooth")
result: "Tap performed."

tool: get_screen()
result:
  Bluetooth
  [on] Bluetooth           ← toggle is ON → tap to turn off.
  [button] Pair new device

tool: tap(description: "Bluetooth")
result: "Tap performed."

tool: get_screen()
result:
  [off] Bluetooth          ← confirmed OFF.

Final reply: "Bluetooth has been turned off."
```

---

## Example C — Web search via browser

User: "google best coffee near me"

```
// Check [INSTALLED APPS] for a browser in this priority:
// com.android.chrome → com.brave.browser → org.mozilla.firefox → com.microsoft.emmx
// If none found → report to user and stop.

tool: launch_app(package_name: "com.android.chrome")
result: "App launched."

tool: get_screen()
result:
  [input] Search or type URL    ← address bar is an [input].

tool: tap(description: "Search or type URL")
result: "Tap performed."

tool: type_text(text: "best coffee near me")
result: "Text typed."

tool: press_key(key: "enter")
result: "Key pressed."

tool: get_screen()
result:
  best coffee near me - Google Search
  [button] Blue Bottle Coffee  4.5★
  [button] Stumptown Coffee  4.3★

Final reply: "Top results: Blue Bottle Coffee and Stumptown Coffee."
```
