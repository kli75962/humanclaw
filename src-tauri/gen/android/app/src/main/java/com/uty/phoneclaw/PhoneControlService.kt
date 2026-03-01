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
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt

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
    }

    // ----- Lifecycle -----

    override fun onServiceConnected() {
        instance = this
    }

    override fun onDestroy() {
        hideOverlay()   // clean up if service stops
        instance = null
        super.onDestroy()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) = Unit
    override fun onInterrupt() = Unit

    // ----- Overlay (LLM-running indicator) -----

    private var overlayView: View? = null
    private var overlayParams: WindowManager.LayoutParams? = null

    /**
     * Show a draggable red recording-dot indicator floating above all apps.
     * Drag to reposition; tap to cancel the LLM agent loop.
     */
    fun showOverlay(onCancel: (() -> Unit)?) {
        if (overlayView != null) return   // already visible

        val wm = getSystemService(WINDOW_SERVICE) as WindowManager
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
                    downRawX   = event.rawX
                    downRawY   = event.rawY
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
                    // Treat as tap only if finger barely moved
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

    fun hideOverlay() {
        overlayView?.let {
            try {
                val wm = getSystemService(WINDOW_SERVICE) as WindowManager
                wm.removeView(it)
            } catch (_: Exception) {}
        }
        overlayView   = null
        overlayParams = null
    }

    // ----- Gesture visual feedback -----

    /**
     * Show a light-blue ripple circle at (x, y) that fades out in 1 second.
     * Safe to call from any thread.
     */
    fun showTapFeedback(x: Float, y: Float) {
        Handler(Looper.getMainLooper()).post {
            val wm = getSystemService(WINDOW_SERVICE) as WindowManager
            val sizePx = 52.dpToPx()

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
                    duration = 900
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
        Handler(Looper.getMainLooper()).post {
            val wm  = getSystemService(WINDOW_SERVICE) as WindowManager
            val dm  = resources.displayMetrics
            val sw  = dm.widthPixels
            val sh  = dm.heightPixels

            val linePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color       = Color.argb(200, 100, 200, 255)
                strokeWidth = 7.dpToPx().toFloat()
                style       = Paint.Style.STROKE
                strokeCap   = Paint.Cap.ROUND
            }
            val arrowPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.argb(220, 100, 200, 255)
                style = Paint.Style.FILL
            }

            val lineView = object : View(this) {
                override fun onDraw(canvas: Canvas) {
                    canvas.drawLine(startX, startY, endX, endY, linePaint)

                    // Arrowhead at endX/endY
                    val dx  = endX - startX
                    val dy  = endY - startY
                    val len = sqrt((dx * dx + dy * dy).toDouble()).toFloat()
                    if (len > 0f) {
                        val ux = dx / len
                        val uy = dy / len
                        val arrowLen = 22.dpToPx().toFloat()
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
                        canvas.drawPath(arrowPath, arrowPaint)
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
     */
    fun getScreenContent(): String {
        val root = rootInActiveWindow ?: return "[screen not accessible]"
        val builder = StringBuilder()
        collectNodeText(root, builder, depth = 0)
        root.recycle()
        return builder.toString().trim().take(4000) // cap to avoid huge prompts
    }

    private fun collectNodeText(node: AccessibilityNodeInfo, sb: StringBuilder, depth: Int) {
        val indent = "  ".repeat(depth.coerceAtMost(6))
        val label = listOfNotNull(
            node.text?.toString(),
            node.contentDescription?.toString(),
            node.hintText?.toString(),
        ).firstOrNull()?.trim()

        if (!label.isNullOrEmpty()) {
            val role = when {
                node.isEditable -> "[input]"
                node.isCheckable -> if (node.isChecked) "[on]"
                                    else "[off]"
                node.isClickable -> "[button]"
                else -> ""
            }
            sb.appendLine("$indent$role $label")
        }

        for (i in 0 until node.childCount) {
            node.getChild(i)?.let { child ->
                collectNodeText(child, sb, depth + 1)
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
            val args = Bundle()
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
        val matches = listOfNotNull(
            node.text?.toString(),
            node.contentDescription?.toString(),
        ).any { it.lowercase().contains(lc) }

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
}
