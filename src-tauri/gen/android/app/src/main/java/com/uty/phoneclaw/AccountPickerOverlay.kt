package com.uty.phoneclaw

import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.view.Gravity
import android.view.WindowManager
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView

class AccountPickerOverlay(
    private val context: Context,
    private val windowManager: WindowManager,
) {
    private var overlayView: LinearLayout? = null
    private var activeRecognizer: SpeechRecognizer? = null

    fun show(
        title: String,
        options: List<String>,
        onSelected: (index: Int, label: String) -> Unit,
        onCancelled: () -> Unit,
    ) {
        hide()

        val layout = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(Color.argb(240, 20, 20, 30))
                cornerRadius = 24f.dpToPx()
            }
            setPadding(28.dpToPx(), 28.dpToPx(), 28.dpToPx(), 28.dpToPx())
        }

        val titleView = TextView(context).apply {
            text = title
            textSize = 20f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            setPadding(0, 0, 0, 16.dpToPx())
        }
        layout.addView(titleView)

        options.forEachIndexed { index, label ->
            val btn = Button(context).apply {
                text = label
                textSize = 18f
                setTextColor(Color.WHITE)
                background = GradientDrawable().apply {
                    setColor(Color.argb(200, 50, 100, 200))
                    cornerRadius = 12f.dpToPx()
                }
                setPadding(16.dpToPx(), 14.dpToPx(), 16.dpToPx(), 14.dpToPx())
                setOnClickListener {
                    hide()
                    onSelected(index, label)
                }
            }
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 8.dpToPx(), 0, 0) }
            layout.addView(btn, lp)
        }

        // Voice button
        val micBtn = Button(context).apply {
            text = "🎤 Speak your choice"
            textSize = 17f
            setTextColor(Color.WHITE)
            background = GradientDrawable().apply {
                setColor(Color.argb(180, 80, 60, 160))
                cornerRadius = 12f.dpToPx()
            }
            setPadding(16.dpToPx(), 14.dpToPx(), 16.dpToPx(), 14.dpToPx())
            setOnClickListener {
                startVoiceSelection(options, onSelected, onCancelled)
            }
        }
        val micLp = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ).apply { setMargins(0, 16.dpToPx(), 0, 0) }
        layout.addView(micBtn, micLp)

        // Cancel button
        val cancelBtn = Button(context).apply {
            text = "Cancel"
            textSize = 16f
            setTextColor(Color.LTGRAY)
            background = null
            setOnClickListener {
                hide()
                onCancelled()
            }
        }
        val cancelLp = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ).apply { setMargins(0, 8.dpToPx(), 0, 0) }
        layout.addView(cancelBtn, cancelLp)

        val params = WindowManager.LayoutParams(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.CENTER
            width = (context.resources.displayMetrics.widthPixels * 0.88f).toInt()
        }

        try {
            windowManager.addView(layout, params)
            overlayView = layout
        } catch (_: Exception) {}
    }

    fun hide() {
        activeRecognizer?.apply { stopListening(); destroy() }
        activeRecognizer = null
        overlayView?.let {
            try { windowManager.removeView(it) } catch (_: Exception) {}
        }
        overlayView = null
    }

    private fun startVoiceSelection(
        options: List<String>,
        onSelected: (Int, String) -> Unit,
        onCancelled: () -> Unit,
    ) {
        activeRecognizer?.apply { stopListening(); destroy() }

        val recognizer = SpeechRecognizer.createSpeechRecognizer(context)
        activeRecognizer = recognizer

        recognizer.setRecognitionListener(object : SimpleRecognitionListener() {
            override fun onResults(results: Bundle?) {
                val spoken = results
                    ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                    ?.firstOrNull()?.trim() ?: ""
                recognizer.destroy()
                activeRecognizer = null

                val accountsJson = "[${options.joinToString(",") { "\"${it.replace("\"", "\\\"")}\"" }}]"
                hide()
                // Return raw speech + account list so LLM can interpret and call commit_suggestion
                ImeServiceBridge.completeFill(FillResult("voice", spoken, accountsJson))
            }

            override fun onError(error: Int) {
                recognizer.destroy()
                activeRecognizer = null
                onCancelled()
            }
        })

        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        }
        recognizer.startListening(intent)
    }

    private fun Float.dpToPx(): Float = this * context.resources.displayMetrics.density
    private fun Int.dpToPx(): Int = (this * context.resources.displayMetrics.density).toInt()
}

abstract class SimpleRecognitionListener : RecognitionListener {
    override fun onReadyForSpeech(params: Bundle?) = Unit
    override fun onBeginningOfSpeech() = Unit
    override fun onRmsChanged(rmsdB: Float) = Unit
    override fun onBufferReceived(buffer: ByteArray?) = Unit
    override fun onEndOfSpeech() = Unit
    override fun onPartialResults(partialResults: Bundle?) = Unit
    override fun onEvent(eventType: Int, params: Bundle?) = Unit
}
