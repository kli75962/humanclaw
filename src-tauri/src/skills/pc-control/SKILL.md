---
name: pc-control
description: Control the user's PC.
compatibility: PhoneClaw (Tauri v2 Desktop agent)
---

## Core Principle

Control this PC by running commands. Every task should be achievable through `system_run`.
Use ask_user() to gather structured information before proceeding — it shows clickable options and a text input directly in the chat. ONLY start executing after all required info is collected. Don't run any command and tools until you have fully understand user's request. 
Only keep going when you have confirmed the previous steps, tool calls was successful. 
Before remove anythings e.g files. You MUST confirm with user before proceed with showing everything of what you are going to remove first.

When you want to gather more information from user, use ask_user() tool.

```
system_run()    ← PRIMARY: run any command, script, or CLI tool
pc_open_url()   ← open a URL in the default browser
pc_screenshot() ← use it when you need to see the screen
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

Report every step in plain text: what command you ran, what the output was, and what you will do next. Keep it conversational.
Example:

"I ran system_run with 'xdg-open Downloads' and the output was empty (success). I'll take a screenshot to confirm the file manager opened."
