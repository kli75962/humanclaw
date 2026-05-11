package com.uty.phoneclaw

import android.content.Context
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.GradientDrawable
import android.text.InputType
import android.view.Gravity
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

class CredentialAssistOverlay(
    private val context: Context,
    private val windowManager: WindowManager,
) {
    private var overlayView: LinearLayout? = null

    fun show(fieldType: String, onResult: (FillResult) -> Unit) {
        hide()

        val isPassword = fieldType == "password"

        val layout = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(Color.argb(245, 20, 20, 30))
                cornerRadius = 24f.dpToPx()
            }
            setPadding(28.dpToPx(), 28.dpToPx(), 28.dpToPx(), 28.dpToPx())
        }

        val titleView = TextView(context).apply {
            text = if (isPassword) "Enter your password" else "Enter your account info"
            textSize = 20f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            setPadding(0, 0, 0, 16.dpToPx())
        }
        layout.addView(titleView)

        val inputField = EditText(context).apply {
            hint = if (isPassword) "Password" else "Email or username"
            setHintTextColor(Color.GRAY)
            setTextColor(Color.WHITE)
            textSize = 17f
            inputType = if (isPassword)
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            else
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
            background = GradientDrawable().apply {
                setColor(Color.argb(180, 40, 40, 55))
                cornerRadius = 10f.dpToPx()
                setStroke(1.dpToPx(), Color.argb(120, 150, 150, 200))
            }
            setPadding(14.dpToPx(), 12.dpToPx(), 14.dpToPx(), 12.dpToPx())
        }
        val inputLp = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        )
        layout.addView(inputField, inputLp)

        // Confirm button — injects text directly via ACTION_SET_TEXT, AI never sees it
        val confirmBtn = Button(context).apply {
            text = "Confirm"
            textSize = 17f
            setTextColor(Color.WHITE)
            background = GradientDrawable().apply {
                setColor(Color.argb(220, 50, 130, 80))
                cornerRadius = 12f.dpToPx()
            }
            setPadding(16.dpToPx(), 12.dpToPx(), 16.dpToPx(), 12.dpToPx())
            setOnClickListener {
                val value = inputField.text.toString()
                if (value.isNotBlank()) {
                    hide()
                    val success = PhoneControlService.instance?.typeText(value, clearFirst = true) ?: false
                    onResult(FillResult(if (success) "filled" else "cancelled"))
                }
            }
        }
        val confirmLp = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ).apply { setMargins(0, 14.dpToPx(), 0, 0) }
        layout.addView(confirmBtn, confirmLp)

        val divider = TextView(context).apply {
            text = "─────────────────"
            setTextColor(Color.argb(80, 200, 200, 200))
            gravity = Gravity.CENTER
            textSize = 12f
            setPadding(0, 18.dpToPx(), 0, 4.dpToPx())
        }
        layout.addView(divider)

        // "I don't remember" button
        val forgotBtn = Button(context).apply {
            text = "I don't remember"
            textSize = 16f
            setTextColor(Color.argb(220, 220, 180, 80))
            background = null
            setOnClickListener {
                hide()
                onResult(FillResult("forgot"))
            }
        }
        val forgotLp = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ).apply { setMargins(0, 4.dpToPx(), 0, 0) }
        layout.addView(forgotBtn, forgotLp)

        // "Never had one — Register" button
        val registerBtn = Button(context).apply {
            text = "Never had one — Register"
            textSize = 16f
            setTextColor(Color.argb(200, 150, 200, 255))
            background = null
            setOnClickListener {
                hide()
                onResult(FillResult("register"))
            }
        }
        val registerLp = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ).apply { setMargins(0, 4.dpToPx(), 0, 0) }
        layout.addView(registerBtn, registerLp)

        val params = WindowManager.LayoutParams(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            // Allow keyboard to show for the EditText — do NOT set FLAG_NOT_FOCUSABLE
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.CENTER
            width = (context.resources.displayMetrics.widthPixels * 0.88f).toInt()
            // Allow soft keyboard to adjust layout
            softInputMode = WindowManager.LayoutParams.SOFT_INPUT_ADJUST_PAN
        }

        try {
            windowManager.addView(layout, params)
            overlayView = layout
            // Request focus so the soft keyboard appears for the EditText
            inputField.requestFocus()
        } catch (_: Exception) {}
    }

    fun hide() {
        overlayView?.let {
            try { windowManager.removeView(it) } catch (_: Exception) {}
        }
        overlayView = null
    }

    private fun Float.dpToPx(): Float = this * context.resources.displayMetrics.density
    private fun Int.dpToPx(): Int = (this * context.resources.displayMetrics.density).toInt()
}
