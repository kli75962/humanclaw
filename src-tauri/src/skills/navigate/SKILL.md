---
name: navigate
description: Read get_screen output and interact with UI elements to navigate any Android app.
compatibility: PhoneClaw (Tauri v2 Android agent)
---

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

Browser priority for web tasks: `com.android.chrome` → `com.brave.browser` → `org.mozilla.firefox` → `com.microsoft.emmx`

````
