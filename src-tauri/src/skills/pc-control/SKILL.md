---
name: pc-control
description: Control the user's PC by executing system commands directly. Use system_run as the primary tool; fall back to pc_screenshot only to verify results visually.
compatibility: PhoneClaw (Tauri v2 Desktop agent)
---

## Core Principle

Control this PC by running commands, not by simulating mouse clicks or reading pixel images. Every task should be achievable through `system_run`. Use `pc_screenshot` only after completing a task to confirm it worked.

```
system_run()    ← PRIMARY: run any command, script, or CLI tool
pc_open_url()   ← open a URL in the default browser
pc_screenshot() ← VERIFY ONLY: confirm the result visually
pc_get_platform() ← check OS once at session start
```

---

## system_run — Primary Tool

Use for everything that can be expressed as a command:

| Task | Example |
|------|---------|
| Open a file or app | `system_run("xdg-open", ["~/Downloads/file.pdf"])` |
| Run a script | `system_run("bash", ["-c", "echo hello > ~/out.txt"])` |
| Query system info | `system_run("bash", ["-c", "df -h"])` |
| Install a package | `system_run("bash", ["-c", "sudo apt install -y curl"])` |
| Send a notification | `system_run("notify-send", ["Done", "Task complete"])` |
| Windows equivalent | `system_run("powershell", ["-Command", "Get-Process"])` |
| Windows open file | `system_run("cmd", ["/c", "start", "", "C:\\file.pdf"])` |

**Always pass each argument as a separate string in `args`.** Never combine arguments into one string with spaces (the shell won't split them).

```
✅ system_run(command: "bash", args: ["-c", "ls ~/Downloads"])
❌ system_run(command: "bash -c ls ~/Downloads")
```

**Timeout:** Default is 30 seconds. For long-running tasks, set `timeout_secs` higher or fire-and-forget with a short timeout.

---

## pc_open_url — Open Browser

```
pc_open_url(url: "https://example.com")
```

Opens in the default browser. Faster than launching the browser through `system_run`.

---

## pc_get_platform — Detect OS

Call once at the start to adapt commands to the OS:

```
pc_get_platform() → { "os": "linux" | "windows" | "macos", "arch": "x86_64" | ... }
```

Use the result to choose the right commands:
- Linux → `bash`, `xdg-open`, `notify-send`
- Windows → `powershell`, `cmd /c start`

---

## pc_screenshot — Verify Only

Use `pc_screenshot` only to confirm that a task completed correctly, or when you must read content that commands cannot return (e.g. rendered UI state, image content).

```
1. system_run(...)          ← do the task
2. pc_screenshot()          ← confirm it worked
```

Never use screenshots to decide what to do next — always prefer running a command to query state (`system_run("bash", ["-c", "cat ~/file.txt"])`).

---

## Recovery

**Command not found:**
1. `pc_get_platform()` — confirm the OS.
2. Check if the tool exists: `system_run("which", ["tool_name"])` (Linux) or `system_run("where", ["tool_name"])` (Windows).
3. Install it with the package manager, or find an alternative command.

**Command succeeded but result is wrong:**
1. Use `pc_screenshot()` to see the current state.
2. Run a diagnostic command to query the actual state.
3. Report what you found and ask the user.

---

## Output Format

Report every step in plain text: what command you ran, what the output was, and what you will do next. Keep it conversational. Example:

"I ran system_run with 'xdg-open Downloads' and the output was empty (success). I'll take a screenshot to confirm the file manager opened."
