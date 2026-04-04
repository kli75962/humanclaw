---
name: pc-control
description: Control the user's PC using a three-layer strategy — prefer direct commands first, structured UI second, and screenshots only for verification.
compatibility: PhoneClaw (Tauri v2 Desktop agent)
---

## Three-Layer Strategy

Always attempt actions in this priority order. Escalate only when the current layer cannot complete the task.

```
Layer 1 (fastest):  system_run()       — direct command execution
Layer 2 (reliable): pc_ui_elements() + pc_activate() / pc_set_text()
Layer 3 (verify only): pc_screenshot() — confirm result, never for control
```

---

## Layer 1 — system_run (Primary)

Use `system_run` whenever the task can be expressed as a command. This is fast, reliable, accessible, and does not depend on the visual state of the screen.

**When to use:**
- Run scripts, open files, query system info, install packages
- Any task expressible as a shell command or CLI tool
- Keyboard shortcuts via `xdotool` / `xdg-open` / PowerShell

**Examples:**
```
system_run(command: "xdg-open", args: ["https://example.com"])
system_run(command: "bash",     args: ["-c", "ls ~/Downloads"])
system_run(command: "notify-send", args: ["Done", "Task complete"])
system_run(command: "powershell", args: ["-Command", "Get-Process"])
```

**Important:** Always specify each argument separately in `args`, never combine them into one string with spaces.

---

## Layer 2 — Structured UI (Fallback)

Use `pc_ui_elements` + `pc_activate` / `pc_set_text` when the task requires interacting with a running GUI application that has no CLI equivalent.

The three structured UI actions:
- **See:** `pc_ui_elements(window_title?)` — list interactive elements on screen
- **Click:** `pc_activate(name, window_title?)` — invoke an element by its label
- **Type:** `pc_set_text(name, text, window_title?)` — type into a field by name

**The See-Click-Type loop:**
```
1. pc_ui_elements()               ← what is on screen?
2. pc_activate(name, window)      ← click the element
3. pc_ui_elements()               ← confirm screen changed
4. pc_set_text(name, text, window) ← type if needed
5. pc_ui_elements()               ← verify result
```

**Element matching rules:**
- Matching is case-insensitive substring — you do NOT need the exact full name.
- Always pass `window_title` when multiple windows are open.

**Open a URL with a specific browser:**
```
pc_open_url(url: "https://example.com")   ← opens in default browser
```

---

## Layer 3 — Screenshot (Verification Only)

`pc_screenshot` is for **confirming** that something worked, not for deciding what to do next.

**Only use pc_screenshot when:**
- Layer 1 and Layer 2 have completed — you need to verify the result visually
- A dialog appeared with no named accessible elements
- You need to read text that the accessibility API does not expose
- The terminal output needs to be read after a command

**Never use pc_screenshot as the primary way to find elements or decide actions.** After taking a screenshot, return to Layer 1 or 2 for the next action.

---

## Platform Info

Check once at the start of a session to adapt commands to the OS:
```
pc_get_platform()  → { "os": "linux" | "windows", "arch": "x86_64" | ... }
```

---

## Recovery

**system_run fails (command not found):**
1. Try `pc_get_platform()` — confirm the OS and adapt the command.
2. Check if the tool is installed with `system_run(command: "which", args: ["tool_name"])` (Linux) or `where tool_name` (Windows).
3. Fall back to Layer 2 if no CLI path exists.

**pc_activate: element not found:**
1. Call `pc_ui_elements()` without a window filter to see all open windows.
2. Check if the target window is open; if not, open it with `system_run`.
3. Use `pc_screenshot()` to see the current state visually.
4. Report exact element names and window titles visible, then ask the user.

---

## Output Format

Report every step in plain text: what tool you called, what the result was, and what you will do next. Do NOT use markdown headers, bold markers, or bullet symbols. Keep it conversational. Example:

"I ran system_run with 'xdg-open Downloads' and the file manager opened. I can see the Downloads folder is now open."
