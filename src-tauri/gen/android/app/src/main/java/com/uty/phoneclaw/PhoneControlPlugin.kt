package com.uty.phoneclaw

import android.app.Activity
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.util.Log
import androidx.core.content.ContextCompat
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

        // Cache the installed apps list for 60 s to avoid repeated PackageManager queries
        // on every user message (build_base_prompt fires once per chat_ollama invocation).
        @Volatile private var appCacheTime = 0L
        @Volatile private var appCacheValue: JSObject? = null
        private const val APP_CACHE_TTL_MS = 60_000L

        @Volatile private var activeRecognizer: SpeechRecognizer? = null
    }

    @Synchronized
    private fun clearActiveRecognizer(recognizer: SpeechRecognizer) {
        if (activeRecognizer === recognizer) {
            activeRecognizer = null
        }
    }

    @Synchronized
    private fun cancelActiveRecognizer() {
        try {
            activeRecognizer?.stopListening()
        } catch (_: Exception) {
        }
        try {
            activeRecognizer?.destroy()
        } catch (_: Exception) {
        }
        activeRecognizer = null
    }

    // ----- Native speech-to-text (Android) -----

    /**
     * One-shot native Android speech recognition.
     * Returns: { text: string }
     */
    @Command
    fun recognizeSpeech(invoke: Invoke) {
        cancelActiveRecognizer()

        if (ContextCompat.checkSelfPermission(activity, android.Manifest.permission.RECORD_AUDIO)
            != PackageManager.PERMISSION_GRANTED
        ) {
            invoke.reject("Microphone permission is not granted")
            return
        }

        if (!SpeechRecognizer.isRecognitionAvailable(activity)) {
            invoke.reject("Speech recognition is not available on this device")
            return
        }

        val recognizer = SpeechRecognizer.createSpeechRecognizer(activity)
        activeRecognizer = recognizer
        val settled = AtomicBoolean(false)
        val mainHandler = Handler(Looper.getMainLooper())

        fun finishSuccess(text: String) {
            if (!settled.compareAndSet(false, true)) return
            recognizer.stopListening()
            recognizer.destroy()
            clearActiveRecognizer(recognizer)
            invoke.resolve(JSObject().apply { put("text", text) })
        }

        fun finishError(message: String) {
            if (!settled.compareAndSet(false, true)) return
            recognizer.stopListening()
            recognizer.destroy()
            clearActiveRecognizer(recognizer)
            invoke.reject(message)
        }

        recognizer.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) = Unit
            override fun onBeginningOfSpeech() = Unit
            override fun onRmsChanged(rmsdB: Float) = Unit
            override fun onBufferReceived(buffer: ByteArray?) = Unit
            override fun onEndOfSpeech() = Unit
            override fun onEvent(eventType: Int, params: Bundle?) = Unit

            override fun onResults(results: Bundle?) {
                val list = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                val best = list?.firstOrNull()?.trim().orEmpty()
                finishSuccess(best)
            }

            override fun onPartialResults(partialResults: Bundle?) = Unit

            override fun onError(error: Int) {
                val msg = when (error) {
                    SpeechRecognizer.ERROR_AUDIO -> "Audio capture error"
                    SpeechRecognizer.ERROR_CLIENT -> "Client error"
                    SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "Insufficient microphone permission"
                    SpeechRecognizer.ERROR_NETWORK -> "Network error"
                    SpeechRecognizer.ERROR_NETWORK_TIMEOUT -> "Network timeout"
                    SpeechRecognizer.ERROR_NO_MATCH -> "No speech matched"
                    SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "Speech recognizer is busy"
                    SpeechRecognizer.ERROR_SERVER -> "Speech server error"
                    SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "No speech input detected"
                    else -> "Speech recognition failed ($error)"
                }
                finishError(msg)
            }
        })

        // Guard against rare devices that never dispatch callbacks.
        mainHandler.postDelayed({
            if (!settled.get()) {
                finishError("Speech recognition timed out")
            }
        }, 20_000)

        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, false)
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, java.util.Locale.getDefault())
        }

        recognizer.startListening(intent)
    }

    /** Cancel active native speech recognition, if running. */
    @Command
    fun cancelSpeechRecognition(invoke: Invoke) {
        cancelActiveRecognizer()
        invoke.resolve(JSObject())
    }

    // ----- App list -----

    /**
     * Returns all user-installed apps as a JSON array:
     * [{ name, package_name, is_system }, ...]
     */
    @Command
    fun getInstalledApps(invoke: Invoke) {
        val now = System.currentTimeMillis()
        val cached = appCacheValue
        if (cached != null && now - appCacheTime < APP_CACHE_TTL_MS) {
            invoke.resolve(cached)
            return
        }

        val pm = activity.packageManager
        // Flag 0: GET_META_DATA is not needed — flags, label, and packageName are all
        // available in the base ApplicationInfo without loading full manifest metadata.
        val apps = pm.getInstalledApplications(0)

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
        appCacheTime = now
        appCacheValue = response
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
                    Log.d("PhoneControlPlugin", "Screen content: $content")
                    resolveToolResult(invoke, toolName, true, content)
                }

                "get_screen_deep" -> {
                    // Deep scan: includes [hidden-area] hints for visible leaf nodes
                    // whose children are hidden from the accessibility tree.
                    // Use only when get_screen lacks expected buttons and you're stuck.
                    val content = service.getScreenContentDeep()
                    Log.d("PhoneControlPlugin", "Screen content (deep): $content")
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

    // ----- Accessibility helpers -----

    /** Returns { enabled: boolean } — whether the PhoneClaw accessibility service is active. */
    @Command
    fun checkAccessibility(invoke: Invoke) {
        val am = activity.getSystemService(android.content.Context.ACCESSIBILITY_SERVICE)
                as android.view.accessibility.AccessibilityManager
        val enabled = am.getEnabledAccessibilityServiceList(
            android.accessibilityservice.AccessibilityServiceInfo.FEEDBACK_ALL_MASK
        ).any { it.resolveInfo.serviceInfo.packageName == activity.packageName }
        invoke.resolve(JSObject().apply { put("enabled", enabled) })
    }

    /** Opens Android's Accessibility Settings screen. */
    @Command
    fun openAccessibilitySettings(invoke: Invoke) {
        activity.startActivity(android.content.Intent(android.provider.Settings.ACTION_ACCESSIBILITY_SETTINGS))
        invoke.resolve(JSObject())
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
