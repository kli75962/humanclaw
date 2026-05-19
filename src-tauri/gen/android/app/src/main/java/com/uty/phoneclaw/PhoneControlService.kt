package com.uty.phoneclaw

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.animation.ObjectAnimator
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.PixelFormat
import android.graphics.Rect
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import android.text.InputType
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject

/**
 * PhoneControlService — Android AccessibilityService that exposes low-level
 * UI automation tools to the PhoneControlPlugin (and through it, to the LLM).
 *
 * Enable via:  Settings → Accessibility → PhoneClaw → turn on
 *
 * Also manages the LLM-running overlay indicator. Hosting it here (inside the
 * AccessibilityService) ensures it stays visible over ALL apps, including when
 * PhoneClaw itself is in the background.
 */
class PhoneControlService : AccessibilityService() {

    companion object {
        @Volatile
        var instance: PhoneControlService? = null
            private set

        /**
         * Pre-built indent strings for depths 0-6.
         * Avoids allocating a new String via repeat() for every node in the tree.
         */
        private val INDENTS = Array(7) { depth -> "  ".repeat(depth) }
    }

    // ----- Lifecycle -----

    override fun onServiceConnected() {
        instance = this
    }

    override fun onDestroy() {
        hideOverlay()
        hideAccountPicker()
        hideCredentialAssist()
        instance = null
        super.onDestroy()
    }

    @Volatile private var screenChangeCallback: (() -> Unit)? = null

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        event ?: return
        if (event.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED) {
            screenChangeCallback?.let { cb -> screenChangeCallback = null; cb() }
        }
        if (isRecording && event.eventType == AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED) {
            handleTypingDuringRecording(event)
        }
    }

    override fun onInterrupt() = Unit

    override fun onMotionEvent(event: MotionEvent) {
        if (!isRecording) return
        val now = System.currentTimeMillis()
        when (event.action) {
            MotionEvent.ACTION_DOWN -> {
                val metrics = windowManager.currentWindowMetrics.bounds
                val fx = event.rawX / metrics.width()
                val fy = event.rawY / metrics.height()
                lastMotionX = fx
                lastMotionY = fy
                lastMotionTime = now
                pendingTapFx = fx
                pendingTapFy = fy
                motionStartTime = now
            }
            MotionEvent.ACTION_MOVE -> {
                if (now - lastMotionTime >= 50) {
                    val metrics = windowManager.currentWindowMetrics.bounds
                    val fx = event.rawX / metrics.width()
                    val fy = event.rawY / metrics.height()
                    if (abs(fx - lastMotionX) > 0.01f || abs(fy - lastMotionY) > 0.01f) {
                        rawGestureEvents.add(RawGestureEvent.Move(fx, fy, now))
                    }
                    lastMotionX = fx
                    lastMotionY = fy
                    lastMotionTime = now
                }
            }
            MotionEvent.ACTION_UP -> {
                val metrics = windowManager.currentWindowMetrics.bounds
                val fx = event.rawX / metrics.width()
                val fy = event.rawY / metrics.height()
                val duration = now - motionStartTime
                if (duration < 200 && rawGestureEvents.none { it is RawGestureEvent.Move }) {
                    // short press with no movement = tap
                    val tapFx = pendingTapFx
                    val tapFy = pendingTapFy
                    val credential = pendingFillCredential
                    if (credential != null) {
                        pendingFillCredential = null
                        rawGestureEvents.add(RawGestureEvent.FillCredential(credential))
                    } else {
                        rawGestureEvents.add(RawGestureEvent.Tap(tapFx, tapFy))
                    }
                } else {
                    rawGestureEvents.add(RawGestureEvent.Up(fx, fy, now))
                }
            }
        }
    }

    // ----- Overlay (LLM-running indicator) -----

    private var overlayView: View? = null
    private var overlayParams: WindowManager.LayoutParams? = null

    /** Cached handler for the main thread — reused by tap/swipe feedback. */
    private val mainHandler = Handler(Looper.getMainLooper())

    /** Cached WindowManager — lifetime matches the service, so caching is safe. */
    private val windowManager by lazy { getSystemService(WINDOW_SERVICE) as WindowManager }

    /**
     * Pre-allocated Paint objects for swipe feedback (accessed on main thread only).
     * Avoids two Paint allocations per swipe in the agentic loop.
     */
    private val swipeLinePaint by lazy {
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color       = Color.argb(200, 100, 200, 255)
            strokeWidth = 7.dpToPx().toFloat()
            style       = Paint.Style.STROKE
            strokeCap   = Paint.Cap.ROUND
        }
    }
    private val swipeArrowPaint by lazy {
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.argb(220, 100, 200, 255)
            style = Paint.Style.FILL
        }
    }

    /**
     * Show a draggable red recording-dot indicator floating above all apps.
     * Drag to reposition; tap to cancel the LLM agent loop.
     */
    fun showOverlay(onCancel: (() -> Unit)?) {
        mainHandler.post {
            if (overlayView != null) return@post

            val wm = windowManager
            val sizePx   = 56.dpToPx()
            val innerPx  = 30.dpToPx()
            val strokePx =  3.dpToPx()

            val container = FrameLayout(this)

            // Outer white ring
            val outerRing = View(this).apply {
                background = GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(Color.TRANSPARENT)
                    setStroke(strokePx, Color.WHITE)
                }
            }

            // Inner red dot
            val innerDot = View(this).apply {
                background = GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(Color.RED)
                }
            }

            container.addView(outerRing, FrameLayout.LayoutParams(sizePx, sizePx))
            container.addView(innerDot,  FrameLayout.LayoutParams(innerPx, innerPx, Gravity.CENTER))

            // Initial position: vertically centred, right edge
            val display = wm.currentWindowMetrics.bounds
            val params = WindowManager.LayoutParams(
                sizePx,
                sizePx,
                // TYPE_ACCESSIBILITY_OVERLAY: shows above ALL apps, no SYSTEM_ALERT_WINDOW needed.
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.TRANSLUCENT,
            ).apply {
                gravity = Gravity.TOP or Gravity.START
                x = display.width() - sizePx - 16.dpToPx()
                y = (display.height() - sizePx) / 2
            }

            // Touch handler — distinguishes drag from tap
            var downRawX = 0f
            var downRawY = 0f
            var startParamX = 0
            var startParamY = 0
            val TAP_SLOP = 10.dpToPx()   // max movement to still count as a tap

            container.setOnTouchListener { _, event ->
                when (event.action) {
                    MotionEvent.ACTION_DOWN -> {
                        downRawX    = event.rawX
                        downRawY    = event.rawY
                        startParamX = params.x
                        startParamY = params.y
                        true
                    }
                    MotionEvent.ACTION_MOVE -> {
                        params.x = (startParamX + (event.rawX - downRawX)).toInt()
                        params.y = (startParamY + (event.rawY - downRawY)).toInt()
                        try { wm.updateViewLayout(container, params) } catch (_: Exception) {}
                        true
                    }
                    MotionEvent.ACTION_UP -> {
                        val dx = event.rawX - downRawX
                        val dy = event.rawY - downRawY
                        if (dx * dx + dy * dy < TAP_SLOP * TAP_SLOP) {
                            onCancel?.invoke()
                        }
                        true
                    }
                    else -> false
                }
            }

            try {
                wm.addView(container, params)
                overlayView   = container
                overlayParams = params
            } catch (_: Exception) {
                // Silently ignore if overlay permission not granted
            }
        }
    }

    fun hideOverlay() {
        mainHandler.post {
            overlayView?.let {
                try {
                    windowManager.removeView(it)
                } catch (_: Exception) {}
            }
            overlayView   = null
            overlayParams = null
        }
    }

    // ----- Gesture visual feedback -----

    /**
     * Show a light-blue ripple circle at (x, y) that fades out in 1 second.
     * Safe to call from any thread.
     */
    fun showTapFeedback(x: Float, y: Float) {
        mainHandler.post {
            val wm = windowManager
            val sizePx = 40.dpToPx()

            val ripple = View(this).apply {
                background = GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    // Light blue fill
                    setColor(Color.argb(170, 100, 200, 255))
                    // Slightly darker border
                    setStroke(3.dpToPx(), Color.argb(220, 60, 160, 255))
                }
                alpha = 0.9f
            }

            val params = WindowManager.LayoutParams(
                sizePx, sizePx,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.TRANSLUCENT,
            ).apply {
                gravity = Gravity.TOP or Gravity.START
                this.x = (x - sizePx / 2f).toInt()
                this.y = (y - sizePx / 2f).toInt()
            }

            try {
                wm.addView(ripple, params)
                ObjectAnimator.ofFloat(ripple, View.ALPHA, 0.9f, 0f).apply {
                    duration = 1000
                    addListener(object : AnimatorListenerAdapter() {
                        override fun onAnimationEnd(animation: Animator) {
                            try { wm.removeView(ripple) } catch (_: Exception) {}
                        }
                    })
                    start()
                }
            } catch (_: Exception) {}
        }
    }

    /**
     * Show a light-blue line with arrowhead from (startX,startY) to (endX,endY)
     * that fades out in 1 second. Safe to call from any thread.
     */
    fun showSwipeFeedback(startX: Float, startY: Float, endX: Float, endY: Float) {
        mainHandler.post {
            val wm = windowManager
            val dm  = resources.displayMetrics
            val sw  = dm.widthPixels
            val sh  = dm.heightPixels

            // Pre-compute arrowhead length once (avoids dpToPx call inside onDraw).
            val arrowLen = 22.dpToPx().toFloat()
            val lineView = object : View(this) {
                override fun onDraw(canvas: Canvas) {
                    canvas.drawLine(startX, startY, endX, endY, swipeLinePaint)

                    // Arrowhead at endX/endY
                    val dx  = endX - startX
                    val dy  = endY - startY
                    val len = sqrt((dx * dx + dy * dy).toDouble()).toFloat()
                    if (len > 0f) {
                        val ux = dx / len
                        val uy = dy / len
                        val angle    = 0.45  // radians (~26°)
                        val ax1 = endX - arrowLen * (ux * cos(-angle) - uy * sin(-angle)).toFloat()
                        val ay1 = endY - arrowLen * (ux * sin(-angle) + uy * cos(-angle)).toFloat()
                        val ax2 = endX - arrowLen * (ux * cos(angle)  - uy * sin(angle)).toFloat()
                        val ay2 = endY - arrowLen * (ux * sin(angle)  + uy * cos(angle)).toFloat()
                        val arrowPath = Path().apply {
                            moveTo(endX, endY)
                            lineTo(ax1, ay1)
                            lineTo(ax2, ay2)
                            close()
                        }
                        canvas.drawPath(arrowPath, swipeArrowPaint)
                    }
                }
            }

            val params = WindowManager.LayoutParams(
                sw, sh,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                        WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
                PixelFormat.TRANSLUCENT,
            ).apply {
                gravity = Gravity.TOP or Gravity.START
                x = 0; y = 0
            }

            try {
                wm.addView(lineView, params)
                ObjectAnimator.ofFloat(lineView, View.ALPHA, 0.9f, 0f).apply {
                    duration = 1000
                    addListener(object : AnimatorListenerAdapter() {
                        override fun onAnimationEnd(animation: Animator) {
                            try { wm.removeView(lineView) } catch (_: Exception) {}
                        }
                    })
                    start()
                }
            } catch (_: Exception) {}
        }
    }

    private fun Int.dpToPx(): Int =
        (this * resources.displayMetrics.density).toInt()

    // ----- Screen reading -----

    /**
     * Walk the accessibility node tree and return a flat text summary of all
     * visible and interactive elements (label | text | description).
     * Iterates ALL visible windows (including dialogs/popups) so overlay UI
     * like "Got it" buttons in Play Store dialogs are captured.
     */
    fun getScreenContent(): String = buildScreenContent(showHiddenAreas = false)

    /**
     * Like getScreenContent() but also emits [hidden-area] entries for visible
     * leaf nodes that have no text and no children. Some apps (e.g. Compose
     * buttons with importantForAccessibility=NO_HIDE_DESCENDANTS) hide their
     * children from the accessibility tree entirely. Call this when the normal
     * get_screen result lacks expected buttons and the LLM is stuck.
     */
    fun getScreenContentDeep(): String = buildScreenContent(showHiddenAreas = true)

    private fun buildScreenContent(showHiddenAreas: Boolean): String {
        val builder = StringBuilder(4096)
        val processedWindowIds = mutableSetOf<Int>()

        windows?.forEach { window ->
            if (window.type == AccessibilityWindowInfo.TYPE_ACCESSIBILITY_OVERLAY) return@forEach
            val root = window.root ?: return@forEach
            processedWindowIds.add(window.id)
            root.refresh()
            collectNodeText(root, builder, depth = 0, showHiddenAreas = showHiddenAreas)
            root.recycle()
        }

        // Fallback: rootInActiveWindow may expose dialog sub-panels not in the windows list.
        rootInActiveWindow?.let { activeRoot ->
            if (activeRoot.windowId !in processedWindowIds) {
                activeRoot.refresh()
                collectNodeText(activeRoot, builder, depth = 0, showHiddenAreas = showHiddenAreas)
            }
            activeRoot.recycle()
        }

        return if (builder.isEmpty()) "[screen not accessible]"
               else builder.toString().trim().take(8000)
    }

    private fun collectNodeText(
        node: AccessibilityNodeInfo,
        sb: StringBuilder,
        depth: Int,
        showHiddenAreas: Boolean,
    ) {
        val indent = INDENTS[depth.coerceAtMost(6)]
        // Null-safe chain replaces listOfNotNull(...).firstOrNull() — zero allocation.
        val label = (node.text?.toString()
            ?: node.contentDescription?.toString()
            ?: node.hintText?.toString())?.trim()

        val isInteractive = node.isClickable || node.isCheckable || node.isEditable
        val bounds = Rect()
        node.getBoundsInScreen(bounds)
        val vis = node.isVisibleToUser

        when {
            !label.isNullOrEmpty() -> {
                val role = when {
                    node.isEditable  -> "[input]"
                    node.isCheckable -> if (node.isChecked) "[on]" else "[off]"
                    node.isClickable -> "[button]"
                    else             -> ""
                }
                val coords = if (!bounds.isEmpty) " @(${bounds.centerX()},${bounds.centerY()})" else ""
                sb.appendLine("$indent$role $label$coords")
            }
            isInteractive && vis && !bounds.isEmpty -> {
                // Clickable/editable node with no text — emit with fallback label.
                val fallback = node.viewIdResourceName?.substringAfterLast('/')
                    ?: node.className?.toString()?.substringAfterLast('.')
                    ?: "interactive"
                val role = when {
                    node.isEditable  -> "[input]"
                    node.isCheckable -> if (node.isChecked) "[on]" else "[off]"
                    node.isClickable -> "[button]"
                    else             -> ""
                }
                sb.appendLine("$indent$role <$fallback> @(${bounds.centerX()},${bounds.centerY()})")
            }
            showHiddenAreas && vis && !bounds.isEmpty && node.childCount == 0 -> {
                // Deep-scan only: visible leaf with no exposed text or interactivity.
                // Could be a Compose button hidden from the accessibility tree.
                val w = bounds.width()
                val h = bounds.height()
                if (w > 40 && h > 40) {
                    sb.appendLine("$indent[hidden-area] @(${bounds.centerX()},${bounds.centerY()})")
                }
            }
        }

        for (i in 0 until node.childCount) {
            node.getChild(i)?.let { child ->
                collectNodeText(child, sb, depth + 1, showHiddenAreas)
                child.recycle()
            }
        }
    }

    // ----- Tap -----

    /**
     * Tap by accessibility content description or visible text.
     * Falls back to coordinate tap if neither is found.
     */
    fun tapByDescription(description: String): Boolean {
        val root = rootInActiveWindow ?: return false
        val target = findNodeByText(root, description)
        return if (target != null) {
            // Show visual feedback at the node's centre before tapping
            val bounds = Rect()
            target.getBoundsInScreen(bounds)
            if (!bounds.isEmpty) showTapFeedback(bounds.exactCenterX(), bounds.exactCenterY())

            val result = target.performAction(AccessibilityNodeInfo.ACTION_CLICK)
            target.recycle()
            root.recycle()
            result
        } else {
            root.recycle()
            false
        }
    }

    fun tapByCoordinates(x: Float, y: Float): Boolean {
        showTapFeedback(x, y)
        val path = Path().apply { moveTo(x, y) }
        val stroke = GestureDescription.StrokeDescription(path, 0, 50)
        val gesture = GestureDescription.Builder().addStroke(stroke).build()
        return dispatchGesture(gesture, null, null)
    }

    // ----- Type text -----

    fun typeText(text: String, clearFirst: Boolean): Boolean {
        val root = rootInActiveWindow ?: return false
        val focused = findFocusedEditable(root)
        root.recycle()
        focused ?: return false

        if (clearFirst) {
            focused.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION,
                Bundle().apply {
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 0)
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT,
                        focused.text?.length ?: 0)
                })
            focused.performAction(AccessibilityNodeInfo.ACTION_CUT)
        }

        val args = Bundle().apply {
            putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text)
        }
        val result = focused.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)
        focused.recycle()
        return result
    }

    // ----- Swipe -----

    fun swipe(direction: String, distance: String): Boolean {
        val display = resources.displayMetrics
        val w = display.widthPixels.toFloat()
        val h = display.heightPixels.toFloat()
        val cx = w / 2f
        val cy = h / 2f

        val delta = when (distance) {
            "short" -> h * 0.15f
            "long" -> h * 0.60f
            else -> h * 0.35f // medium
        }

        val (startX, startY, endX, endY) = when (direction) {
            "up" -> arrayOf(cx, cy + delta, cx, cy - delta)
            "down" -> arrayOf(cx, cy - delta, cx, cy + delta)
            "left" -> arrayOf(cx + delta, cy, cx - delta, cy)
            "right" -> arrayOf(cx - delta, cy, cx + delta, cy)
            else -> return false
        }

        showSwipeFeedback(startX, startY, endX, endY)

        val path = Path().apply {
            moveTo(startX, startY)
            lineTo(endX, endY)
        }
        val stroke = GestureDescription.StrokeDescription(path, 0, 300)
        val gesture = GestureDescription.Builder().addStroke(stroke).build()
        return dispatchGesture(gesture, null, null)
    }

    // ----- Press key -----

    fun pressKey(key: String): Boolean = when (key) {
        "home" -> performGlobalAction(GLOBAL_ACTION_HOME)
        "back" -> performGlobalAction(GLOBAL_ACTION_BACK)
        "recents" -> performGlobalAction(GLOBAL_ACTION_RECENTS)
        "notifications" -> performGlobalAction(GLOBAL_ACTION_NOTIFICATIONS)
        "enter" -> {
            val root = rootInActiveWindow
            val focused = root?.let { findFocusedEditable(it) }
            root?.recycle()
            if (focused == null) {
                false
            } else {
                // ACTION_IME_ENTER is API 30+; fall back to ACTION_CLICK (submits search bars)
                val success = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                    @Suppress("NewApi")
                    focused.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_IME_ENTER.id)
                } else {
                    focused.performAction(AccessibilityNodeInfo.ACTION_CLICK)
                }
                focused.recycle()
                success
            }
        }
        else -> false
    }

    // ----- Launch app -----

    fun launchApp(packageName: String): Boolean {
        return try {
            val intent = packageManager.getLaunchIntentForPackage(packageName)
                ?: return false
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            applicationContext.startActivity(intent)
            true
        } catch (_: Exception) {
            false
        }
    }

    // ----- Helpers -----

    private fun findNodeByText(root: AccessibilityNodeInfo, text: String): AccessibilityNodeInfo? {
        val lc = text.lowercase()
        // System API search — may return non-clickable text nodes
        val candidates = root.findAccessibilityNodeInfosByText(text)
        for (candidate in candidates) {
            val clickable = findClickableAncestor(candidate)
            if (clickable != null) return clickable
            candidate.recycle()
        }
        // DFS fallback for partial/lowercase matching
        return searchNode(root, lc)
    }

    /**
     * Walk UP the node tree to find the nearest clickable ancestor (including self).
     * Returns null if the entire parent chain is non-clickable.
     */
    private fun findClickableAncestor(node: AccessibilityNodeInfo): AccessibilityNodeInfo? {
        if (node.isClickable) return node
        val parent = node.parent ?: return null
        val result = findClickableAncestor(parent)
        // Only recycle parent if we are NOT returning it
        if (result !== parent) parent.recycle()
        return result
    }

    private fun searchNode(node: AccessibilityNodeInfo, lc: String): AccessibilityNodeInfo? {
        // Direct null checks instead of listOfNotNull(...).any{} to avoid List allocation
        // on every DFS node visit.
        val matches = node.text?.toString()?.lowercase()?.contains(lc) == true
            || node.contentDescription?.toString()?.lowercase()?.contains(lc) == true

        if (matches) {
            // Return this node if clickable, else climb to a clickable ancestor
            if (node.isClickable) return node
            val ancestor = findClickableAncestor(node)
            if (ancestor != null) return ancestor
        }

        for (i in 0 until node.childCount) {
            val child = node.getChild(i) ?: continue
            val found = searchNode(child, lc)
            if (found != null) {
                child.recycle()
                return found
            }
            child.recycle()
        }
        return null
    }

    private fun findFocusedEditable(root: AccessibilityNodeInfo): AccessibilityNodeInfo? {
        if (root.isEditable && root.isFocused) return root
        for (i in 0 until root.childCount) {
            val child = root.getChild(i) ?: continue
            val found = findFocusedEditable(child)
            if (found != null) { child.recycle(); return found }
            child.recycle()
        }
        return null
    }

    // ----- Credential tools -----

    fun typeCredential(appPackage: String, fieldType: String): Boolean {
        val value = CredentialStore.get(applicationContext, appPackage, fieldType)
            ?: return false
        return typeText(value, clearFirst = true)
    }

    @androidx.annotation.RequiresApi(android.os.Build.VERSION_CODES.R)
    fun commitSuggestion(index: Int): Boolean {
        val ime = ImeServiceBridge.imeInstance ?: return false
        return ime.commitSuggestion(index)
    }

    // ----- Account picker overlay -----

    private var accountPickerOverlay: AccountPickerOverlay? = null
    private var credentialAssistOverlay: CredentialAssistOverlay? = null

    fun showAccountPicker(title: String, options: List<String>, onSelected: (Int, String) -> Unit, onCancelled: () -> Unit) {
        mainHandler.post {
            accountPickerOverlay?.hide()
            accountPickerOverlay = AccountPickerOverlay(applicationContext, windowManager)
            accountPickerOverlay!!.show(title, options, onSelected, onCancelled)
        }
    }

    fun hideAccountPicker() {
        mainHandler.post { accountPickerOverlay?.hide(); accountPickerOverlay = null }
    }

    fun showLoginMethodPicker(methods: List<String>, onSelected: (String) -> Unit, onCancelled: () -> Unit) {
        showAccountPicker(
            title = "How do you want to login?",
            options = methods,
            onSelected = { _, label -> onSelected(label) },
            onCancelled = onCancelled
        )
    }

    fun showCredentialAssist(fieldType: String, onResult: (FillResult) -> Unit) {
        mainHandler.post {
            credentialAssistOverlay?.hide()
            credentialAssistOverlay = CredentialAssistOverlay(applicationContext, windowManager)
            credentialAssistOverlay!!.show(fieldType, onResult)
        }
    }

    fun hideCredentialAssist() {
        mainHandler.post { credentialAssistOverlay?.hide(); credentialAssistOverlay = null }
    }

    // ----- Gesture recording -----

    sealed class RawGestureEvent {
        data class Tap(val fx: Float, val fy: Float) : RawGestureEvent()
        data class Move(val fx: Float, val fy: Float, val time: Long) : RawGestureEvent()
        data class Up(val fx: Float, val fy: Float, val time: Long) : RawGestureEvent()
        data class FillCredential(val fieldType: String) : RawGestureEvent()
    }

    @Volatile var isRecording = false
        private set
    private var isSharingMode = false
    private val rawGestureEvents = mutableListOf<RawGestureEvent>()
    private var lastMotionX = 0f
    private var lastMotionY = 0f
    private var lastMotionTime = 0L
    private var pendingTapFx = 0f
    private var pendingTapFy = 0f
    private var motionStartTime = 0L
    @Volatile var pendingFillCredential: String? = null
    private var sensitiveOverlayShown = false
    private var recordingOverlayView: View? = null

    fun startRecording(sharing: Boolean) {
        isSharingMode = sharing
        rawGestureEvents.clear()
        pendingFillCredential = null
        sensitiveOverlayShown = false
        isRecording = true
        showRecordingIndicator()
    }

    fun stopRecording(): String {
        isRecording = false
        hideRecordingIndicator()
        val metrics = windowManager.currentWindowMetrics.bounds
        val w = metrics.width()
        val h = metrics.height()
        val eventsJson = buildEventsJson(w, h)
        return """{"events":$eventsJson,"screen_width":$w,"screen_height":$h,"has_credential_events":${rawGestureEvents.any { it is RawGestureEvent.FillCredential }}}"""
    }

    private fun buildEventsJson(screenW: Int, screenH: Int): String {
        val arr = JSONArray()
        var moveStartFx = 0f
        var moveStartFy = 0f
        var moveStartTime = 0L
        var inSwipe = false

        for (ev in rawGestureEvents) {
            when (ev) {
                is RawGestureEvent.Tap -> {
                    val obj = JSONObject()
                    obj.put("type", "tap")
                    obj.put("fx", ev.fx.toDouble())
                    obj.put("fy", ev.fy.toDouble())
                    obj.put("wait_for_screen_change", false)
                    obj.put("delay_ms", 200)
                    arr.put(obj)
                    inSwipe = false
                }
                is RawGestureEvent.Move -> {
                    if (!inSwipe) {
                        moveStartFx = ev.fx
                        moveStartFy = ev.fy
                        moveStartTime = ev.time
                        inSwipe = true
                    }
                }
                is RawGestureEvent.Up -> {
                    if (inSwipe) {
                        val obj = JSONObject()
                        obj.put("type", "swipe")
                        obj.put("fx_start", moveStartFx.toDouble())
                        obj.put("fy_start", moveStartFy.toDouble())
                        obj.put("fx_end", ev.fx.toDouble())
                        obj.put("fy_end", ev.fy.toDouble())
                        obj.put("wait_for_screen_change", false)
                        obj.put("delay_ms", 200)
                        arr.put(obj)
                        inSwipe = false
                    }
                }
                is RawGestureEvent.FillCredential -> {
                    val obj = JSONObject()
                    obj.put("type", "fill_credential")
                    obj.put("field_type", ev.fieldType)
                    arr.put(obj)
                    inSwipe = false
                }
            }
        }
        return arr.toString()
    }

    private fun handleTypingDuringRecording(event: AccessibilityEvent) {
        val source = event.source ?: return
        val isPassword = source.isPassword
        val isEmail = (source.inputType and InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS != 0)
            || source.hintText?.contains("email", ignoreCase = true) == true
        source.recycle()
        if (!isPassword && !isEmail) return
        val fieldType = if (isPassword) "password" else "email"

        if (!isSharingMode) {
            pendingFillCredential = fieldType
            return
        }
        if (sensitiveOverlayShown) return
        sensitiveOverlayShown = true
        showSensitiveInputWarning(
            onStop = { stopRecording(); sensitiveOverlayShown = false },
            onSkip = { pendingFillCredential = fieldType; sensitiveOverlayShown = false },
            onContinue = { sensitiveOverlayShown = false }
        )
    }

    private fun showRecordingIndicator() {
        mainHandler.post {
            if (recordingOverlayView != null) return@post
            val wm = windowManager

            val row = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(12.dpToPx(), 6.dpToPx(), 8.dpToPx(), 6.dpToPx())
                background = GradientDrawable().apply {
                    setColor(Color.argb(220, 0, 0, 0))
                    cornerRadius = 8.dpToPx().toFloat()
                }
            }

            val recLabel = TextView(this).apply {
                text = "● REC"
                setTextColor(Color.RED)
                textSize = 12f
            }

            val stopBtn = TextView(this).apply {
                text = "  ■ Stop"
                setTextColor(Color.WHITE)
                textSize = 12f
                setOnClickListener {
                    stopRecording()
                    hideRecordingIndicator()
                }
            }

            row.addView(recLabel)
            row.addView(stopBtn)

            val params = WindowManager.LayoutParams(
                WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
                PixelFormat.TRANSLUCENT,
            ).apply {
                gravity = Gravity.TOP or Gravity.START
                x = 16.dpToPx()
                y = 48.dpToPx()
            }
            try {
                wm.addView(row, params)
                recordingOverlayView = row
            } catch (_: Exception) {}
        }
    }

    private fun hideRecordingIndicator() {
        mainHandler.post {
            recordingOverlayView?.let {
                try { windowManager.removeView(it) } catch (_: Exception) {}
            }
            recordingOverlayView = null
        }
    }

    private fun showSensitiveInputWarning(onStop: () -> Unit, onSkip: () -> Unit, onContinue: () -> Unit) {
        mainHandler.post {
            val wm = windowManager
            val container = LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(20.dpToPx(), 16.dpToPx(), 20.dpToPx(), 16.dpToPx())
                background = GradientDrawable().apply {
                    setColor(Color.argb(240, 30, 30, 30))
                    cornerRadius = 12.dpToPx().toFloat()
                }
            }

            val msg = TextView(this).apply {
                text = "Sensitive input detected. This recording may be shared with the community."
                setTextColor(Color.WHITE)
                textSize = 14f
            }
            container.addView(msg, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { bottomMargin = 12.dpToPx() })

            var overlayContainer: View? = null
            fun dismiss() {
                overlayContainer?.let { v ->
                    try { wm.removeView(v) } catch (_: Exception) {}
                }
            }

            fun makeBtn(label: String, color: Int, action: () -> Unit): Button {
                return Button(this).apply {
                    text = label
                    setTextColor(Color.WHITE)
                    setBackgroundColor(color)
                    setOnClickListener { dismiss(); action() }
                }
            }

            container.addView(makeBtn("Stop Recording", Color.rgb(180, 40, 40), onStop))
            container.addView(makeBtn("Skip Keyboard Input (Recommended)", Color.rgb(40, 120, 40), onSkip),
                LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT).apply {
                    topMargin = 8.dpToPx()
                })
            container.addView(makeBtn("Continue Anyway", Color.rgb(80, 80, 80), onContinue),
                LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT).apply {
                    topMargin = 8.dpToPx()
                })

            val display = wm.currentWindowMetrics.bounds
            val params = WindowManager.LayoutParams(
                (display.width() * 0.85).toInt(),
                WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.TRANSLUCENT,
            ).apply {
                gravity = Gravity.CENTER
            }
            try {
                wm.addView(container, params)
                overlayContainer = container
            } catch (_: Exception) {
                onSkip()
            }
        }
    }

    // ----- Gesture replay -----

    fun replayGestureMap(eventsJson: String, screenWidth: Int, screenHeight: Int, onDone: (Boolean, String) -> Unit) {
        val events = try { JSONArray(eventsJson) } catch (e: Exception) {
            onDone(false, "Invalid events JSON: ${e.message}")
            return
        }
        val metrics = windowManager.currentWindowMetrics.bounds
        val w = metrics.width().toFloat()
        val h = metrics.height().toFloat()
        replayStep(events, 0, w, h, onDone)
    }

    private fun replayStep(events: JSONArray, index: Int, screenW: Float, screenH: Float, onDone: (Boolean, String) -> Unit) {
        if (index >= events.length()) {
            onDone(true, "Replay complete.")
            return
        }
        val ev = events.optJSONObject(index) ?: run {
            replayStep(events, index + 1, screenW, screenH, onDone)
            return
        }
        val type = ev.optString("type", "")
        val delayMs = ev.optLong("delay_ms", 200)
        val waitForChange = ev.optBoolean("wait_for_screen_change", false)

        fun next() {
            if (waitForChange) {
                val timeout = ev.optLong("max_wait_ms", 8000)
                val timer = Handler(Looper.getMainLooper())
                val timeoutRunnable = Runnable {
                    screenChangeCallback = null
                    replayStep(events, index + 1, screenW, screenH, onDone)
                }
                screenChangeCallback = {
                    timer.removeCallbacks(timeoutRunnable)
                    mainHandler.postDelayed({ replayStep(events, index + 1, screenW, screenH, onDone) }, 300)
                }
                timer.postDelayed(timeoutRunnable, timeout)
            } else {
                mainHandler.postDelayed({ replayStep(events, index + 1, screenW, screenH, onDone) }, delayMs)
            }
        }

        when (type) {
            "tap" -> {
                val x = (ev.optDouble("fx") * screenW).toFloat()
                val y = (ev.optDouble("fy") * screenH).toFloat()
                tapByCoordinates(x, y)
                next()
            }
            "swipe" -> {
                val xStart = (ev.optDouble("fx_start") * screenW).toFloat()
                val yStart = (ev.optDouble("fy_start") * screenH).toFloat()
                val xEnd   = (ev.optDouble("fx_end")   * screenW).toFloat()
                val yEnd   = (ev.optDouble("fy_end")   * screenH).toFloat()
                val duration = ev.optLong("duration_ms", 300)
                val path = GestureDescription.StrokeDescription(
                    android.graphics.Path().apply { moveTo(xStart, yStart); lineTo(xEnd, yEnd) },
                    0L, duration
                )
                dispatchGesture(GestureDescription.Builder().addStroke(path).build(), null, null)
                next()
            }
            "fill_credential" -> {
                val pkg = ev.optString("app_package", "")
                val fieldType = ev.optString("field_type", "")
                val credential = CredentialStore.get(applicationContext, pkg, fieldType)
                if (credential != null) {
                    typeText(credential, clearFirst = true)
                    next()
                } else {
                    // No stored credential — trigger the IME/overlay flow and wait
                    val deferred = CompletableDeferred<FillResult>()
                    ImeServiceBridge.pendingFill = deferred
                    MainScope().launch {
                        try {
                            val result = withTimeout(60_000) { deferred.await() }
                            if (result.status == "filled") next()
                            else onDone(false, "Credential fill cancelled: ${result.status}")
                        } catch (_: Exception) {
                            onDone(false, "Credential fill timed out.")
                        }
                    }
                }
                return
            }
            "wait" -> {
                val ms = ev.optLong("duration_ms", 500)
                mainHandler.postDelayed({ replayStep(events, index + 1, screenW, screenH, onDone) }, ms)
                return
            }
            else -> replayStep(events, index + 1, screenW, screenH, onDone)
        }
    }
}
