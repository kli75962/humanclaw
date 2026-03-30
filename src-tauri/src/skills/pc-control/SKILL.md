---
name: pc-control
description: Control the user's PC like a human — see the screen, click elements, and type text. Covers apps, browser, terminal, files, forms, and menus. Always interact through the UI; never run background commands.
compatibility: PhoneClaw (Tauri v2 Desktop agent)
---

## Core Principle

You control this PC the same way a human does: look at the screen, click things, and type. Every action goes through the UI tools. You NEVER execute shell commands or scripts in the background. If asked to "run ls ~/Downloads", you open a terminal window through the desktop UI and type the command there, just like a human would.

The three actions available to you:
- See: pc_ui_elements() — list what interactive elements are currently on screen
- Click: pc_activate(name, window_title) — click/invoke an element by its label
- Type: pc_set_text(name, text, window_title) — type text into a focused input field

---

## Tool Reference

| Tool | What it does |
|------|-------------|
| pc_get_platform() | Check OS (linux/windows). Call once at the very start. |
| pc_ui_elements(window_title?) | List all interactive elements visible right now. Pass window_title to limit scope. |
| pc_activate(name, window_title?) | Click a button, link, menu item, tab, checkbox, or icon by name. |
| pc_set_text(name, text, window_title?) | Type text into an input field, search box, or terminal line by name. |
| pc_screenshot() | Take a screenshot for visual inspection. Use only when ui_elements is insufficient. |

---

## The See-Click-Type Loop

Every task follows this pattern. Never skip the "see" step.

```
1. pc_ui_elements()               ← what is currently on screen?
2. pc_activate(name, window)      ← click the target element
3. pc_ui_elements()               ← confirm the screen changed
4. pc_set_text(name, text, window) ← type if needed
5. pc_ui_elements()               ← verify result
```

After every click or type, call pc_ui_elements() again to confirm the screen changed before taking the next action.

---

## When to Use pc_screenshot

Use pc_screenshot only when:
- pc_ui_elements returns elements but the target is not listed (visual-only content)
- You need to confirm a loading state, animation, or colour change
- A dialog appeared but has no named interactive elements
- The terminal output needs to be read after typing a command

After pc_screenshot, return to pc_ui_elements to continue interaction.

---

## Element Matching Rules

- pc_activate and pc_set_text match by substring (case-insensitive). You do NOT need the exact full name.
- Always pass window_title when multiple windows are open to target the right one.
- If two elements share the same name in different windows, always specify window_title.

Example — two "OK" buttons:
```
pc_activate(name: "OK", window_title: "Save File")   ← correct
pc_activate(name: "OK")                               ← may hit wrong window
```

---

## Recovery: Element Not Found

If the target element is not in pc_ui_elements output:
1. Call pc_ui_elements() without a window filter to see all open windows.
2. Check if the correct window is open. If not, open it first.
3. If the window is open but the element is missing, use pc_screenshot to see the current state visually.
4. If the element exists but has a different name, use a shorter substring to match it.
5. If still stuck, report the exact element names and window titles visible and ask the user.

---

## Recovery: App Did Not Open

After calling pc_activate on an app icon:
1. Call pc_ui_elements() again — the app may need a moment to launch.
2. If still not open after two attempts, use pc_screenshot to check the desktop state.
3. Report what you see and ask the user if the app path or name is different.

---

## Output Format

Report every step in plain text: what tool you called, what the result was, and what you will do next. Do NOT use markdown headers, bold markers, or bullet symbols. Keep it conversational. Example:

"I called pc_ui_elements and found a Terminal icon in the taskbar. I clicked it with pc_activate. The terminal window opened with the title 'Kitty'. I can see a text entry area. I will now type the command."
