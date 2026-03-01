package com.uty.phoneclaw

import android.app.Activity
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.concurrent.atomic.AtomicBoolean

/**
 * PhoneControlPlugin — Tauri v2 Android plugin that bridges the Rust layer with:
 * - PackageManager (get installed apps)
 * - PhoneControlService (accessibility-based phone control + overlay indicator)
 */
@TauriPlugin
class PhoneControlPlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        /**
         * Set to true when user taps the overlay cancel button.
         * Rust polls this via isCancelled() and resets it on read.
         */
        val cancelRequested = AtomicBoolean(false)
    }

    // ----- App list -----

    /**
     * Returns all user-installed apps as a JSON array:
     * [{ name, package_name, is_system }, ...]
     */
    @Command
    fun getInstalledApps(invoke: Invoke) {
        val pm = activity.packageManager
        val apps = pm.getInstalledApplications(PackageManager.GET_META_DATA)

        val result = JSArray()
        for (app in apps) {
            val isSystem = (app.flags and ApplicationInfo.FLAG_SYSTEM) != 0
            val obj = JSObject().apply {
                put("name", pm.getApplicationLabel(app).toString())
                put("package_name", app.packageName)
                put("is_system", isSystem)
            }
            result.put(obj)
        }

        val response = JSObject().apply { put("apps", result) }
        invoke.resolve(response)
    }

    // ----- Tool execution dispatcher -----

    /**
     * Execute a phone-control tool requested by the LLM.
     *
     * Expects: { tool: string, args: object }
     * Returns: { tool_name, success, output }
     */
    @Command
    fun executeTool(invoke: Invoke) {
        val rawArgs = invoke.getArgs()
        val toolName = rawArgs.optString("tool", "").takeIf { it.isNotEmpty() } ?: run {
            invoke.reject("Missing 'tool' parameter")
            return
        }
        // args may be a nested object or absent; wrap safely as JSObject
        val args = rawArgs.optJSONObject("args")
            ?.let { JSObject(it.toString()) } ?: JSObject()

        val service = PhoneControlService.instance
        if (service == null) {
            resolveToolResult(invoke, toolName, false, "Accessibility service is not running. Enable it in Settings → Accessibility → PhoneClaw.")
            return
        }

        try {
            when (toolName) {
                "get_screen" -> {
                    val content = service.getScreenContent()
                    resolveToolResult(invoke, toolName, true, content)
                }

                "tap" -> {
                    val description = args.optString("description", "")
                    val success = if (description.isNotBlank()) {
                        service.tapByDescription(description)
                    } else {
                        val x = args.optDouble("x", 0.0).toFloat()
                        val y = args.optDouble("y", 0.0).toFloat()
                        service.tapByCoordinates(x, y)
                    }
                    resolveToolResult(invoke, toolName, success,
                        if (success) "Tap performed." else "Tap target not found.")
                }

                "type_text" -> {
                    val text = args.optString("text", "")
                    val clearFirst = args.optBoolean("clear_first", false)
                    val success = service.typeText(text, clearFirst)
                    resolveToolResult(invoke, toolName, success,
                        if (success) "Text typed." else "No editable field focused.")
                }

                "swipe" -> {
                    val direction = args.optString("direction", "up")
                    val distance = args.optString("distance", "medium")
                    val success = service.swipe(direction, distance)
                    resolveToolResult(invoke, toolName, success,
                        if (success) "Swipe done." else "Swipe gesture failed.")
                }

                "press_key" -> {
                    val key = args.optString("key", "")
                    val success = service.pressKey(key)
                    resolveToolResult(invoke, toolName, success,
                        if (success) "Key pressed." else "Unknown key: $key")
                }

                "launch_app" -> {
                    val pkg = args.optString("package_name", "").takeIf { it.isNotEmpty() } ?: run {
                        resolveToolResult(invoke, toolName, false, "Missing package_name")
                        return
                    }
                    val success = service.launchApp(pkg)
                    resolveToolResult(invoke, toolName, success,
                        if (success) "App launched." else "Could not launch $pkg.")
                }

                else -> resolveToolResult(invoke, toolName, false, "Unknown tool: $toolName")
            }
        } catch (e: Exception) {
            resolveToolResult(invoke, toolName, false, "Exception: ${e.message}")
        }
    }

    // ----- Overlay commands — delegate to PhoneControlService -----

    /**
     * Show the recording-dot overlay via the AccessibilityService,
     * which keeps it visible over ALL apps even when PhoneClaw is in background.
     */
    @Command
    fun showOverlay(invoke: Invoke) {
        val service = PhoneControlService.instance
        if (service != null) {
            service.showOverlay(onCancel = { cancelRequested.set(true) })
        }
        // If service is null: overlay silently skipped (accessibility not enabled)
        invoke.resolve(JSObject())
    }

    /** Hide the overlay. */
    @Command
    fun hideOverlay(invoke: Invoke) {
        PhoneControlService.instance?.hideOverlay()
        invoke.resolve(JSObject())
    }

    /**
     * Returns { value: true } if the user tapped the cancel button since last call.
     * Atomically resets the flag on read.
     */
    @Command
    fun isCancelled(invoke: Invoke) {
        val cancelled = cancelRequested.getAndSet(false)
        invoke.resolve(JSObject().apply { put("value", cancelled) })
    }

    // ----- Internal helpers -----

    private fun resolveToolResult(
        invoke: Invoke,
        toolName: String,
        success: Boolean,
        output: String,
    ) {
        val obj = JSObject().apply {
            put("tool_name", toolName)
            put("success", success)
            put("output", output)
        }
        invoke.resolve(obj)
    }
}
