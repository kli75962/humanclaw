package com.uty.phoneclaw

import kotlinx.coroutines.CompletableDeferred

data class AccountSelection(val selectedIndex: Int, val accountHint: String)
data class InlineSuggestionInfo(val index: Int, val displayText: String)
data class InlineSuggestionSet(val suggestions: List<InlineSuggestionInfo>)
data class FillResult(
    val status: String,           // "filled" | "forgot" | "register" | "voice" | "cancelled"
    val hint: String = "",        // voice: spoken text
    val accountsJson: String = "" // voice: JSON array of available account display texts
)

object ImeServiceBridge {
    @Volatile var imeInstance: PhoneClawIME? = null
    @Volatile var pendingSuggestions: InlineSuggestionSet? = null

    // Holds the raw InlineSuggestion objects until commitSuggestion() is called.
    // Must be kept alive because they hold live Binder references.
    @Volatile var rawSuggestions: List<android.view.inputmethod.InlineSuggestion>? = null

    // CompletableDeferred that fill_credential_field suspends on.
    @Volatile var pendingFill: CompletableDeferred<FillResult>? = null

    fun completeFill(result: FillResult) {
        pendingFill?.complete(result)
        pendingFill = null
    }

    fun cancelFill() {
        pendingFill?.cancel()
        pendingFill = null
    }
}
