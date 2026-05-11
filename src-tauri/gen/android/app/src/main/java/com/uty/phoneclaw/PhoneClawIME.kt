package com.uty.phoneclaw

import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Bundle
import android.util.Size
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InlineSuggestionsRequest
import android.view.inputmethod.InlineSuggestionsResponse
import androidx.annotation.RequiresApi

class PhoneClawIME : InputMethodService() {

    override fun onCreateInputView(): View {
        // Return a zero-height transparent view — this IME never shows a visible keyboard.
        // All typing is driven by PhoneControlService via ACTION_SET_TEXT.
        return View(this).apply {
            layoutParams = android.view.ViewGroup.LayoutParams(
                android.view.ViewGroup.LayoutParams.MATCH_PARENT, 0
            )
        }
    }

    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        ImeServiceBridge.imeInstance = this
    }

    override fun onFinishInput() {
        ImeServiceBridge.imeInstance = null
        ImeServiceBridge.pendingSuggestions = null
        ImeServiceBridge.rawSuggestions = null
        super.onFinishInput()
    }

    // ----- Inline autofill suggestions (Android 11+) -----

    @RequiresApi(Build.VERSION_CODES.R)
    override fun onCreateInlineSuggestionsRequest(uiExtras: android.os.Bundle): InlineSuggestionsRequest {
        return InlineSuggestionsRequest.Builder(emptyList()).build()
    }

    @RequiresApi(Build.VERSION_CODES.R)
    override fun onInlineSuggestionsResponse(response: InlineSuggestionsResponse): Boolean {
        val suggestions = response.inlineSuggestions
        val service = PhoneControlService.instance

        when {
            suggestions.isEmpty() -> {
                // No suggestions: show CredentialAssistOverlay so user can type manually
                ImeServiceBridge.pendingSuggestions = null
                ImeServiceBridge.rawSuggestions = null
                service?.showCredentialAssist("") { result ->
                    ImeServiceBridge.completeFill(result)
                }
            }
            suggestions.size == 1 -> {
                // Single suggestion: silently auto-commit, no UI shown
                ImeServiceBridge.rawSuggestions = suggestions
                suggestions[0].inflate(applicationContext, Size(1, 1), mainExecutor) { inflated ->
                    inflated?.rootView?.performClick()
                    ImeServiceBridge.rawSuggestions = null
                    ImeServiceBridge.completeFill(FillResult("filled"))
                }
            }
            else -> {
                // Multiple suggestions: store them and show the account picker.
                // InlineSuggestion display text is only accessible after inflate(), so we
                // label them by number here and the overlay shows them as "Account 1", "Account 2", etc.
                ImeServiceBridge.rawSuggestions = suggestions
                val infos = suggestions.mapIndexed { idx, _ ->
                    InlineSuggestionInfo(idx, "Account ${idx + 1}")
                }
                ImeServiceBridge.pendingSuggestions = InlineSuggestionSet(infos)

                val optionLabels = infos.map { it.displayText }
                val accountsJson = "[${optionLabels.joinToString(",") { "\"${it.replace("\"", "\\\"")}\"" }}]"

                service?.showAccountPicker(
                    title = "Choose an account",
                    options = optionLabels,
                    onSelected = { index, label ->
                        commitSuggestion(index)
                    },
                    onCancelled = {
                        ImeServiceBridge.rawSuggestions = null
                        ImeServiceBridge.pendingSuggestions = null
                        ImeServiceBridge.completeFill(FillResult("cancelled"))
                    }
                )
            }
        }
        return true
    }

    // Called by PhoneControlService after user selects — commits a specific inline suggestion.
    @RequiresApi(Build.VERSION_CODES.R)
    fun commitSuggestion(index: Int): Boolean {
        val suggestions = ImeServiceBridge.rawSuggestions ?: return false
        val suggestion = suggestions.getOrNull(index) ?: return false
        suggestion.inflate(applicationContext, Size(1, 1), mainExecutor) { inflated ->
            inflated?.rootView?.performClick()
            ImeServiceBridge.rawSuggestions = null
            ImeServiceBridge.pendingSuggestions = null
            ImeServiceBridge.completeFill(FillResult("filled"))
        }
        return true
    }
}
