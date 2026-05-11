package com.uty.phoneclaw

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

object CredentialStore {
    private const val PREFS_FILE = "phoneclaw_credentials"

    private fun prefs(context: Context) = EncryptedSharedPreferences.create(
        context,
        PREFS_FILE,
        MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build(),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
    )

    private fun key(appPackage: String, fieldType: String) = "$appPackage/$fieldType"

    fun get(context: Context, appPackage: String, fieldType: String): String? =
        try {
            prefs(context).getString(key(appPackage, fieldType), null)
        } catch (_: Exception) {
            null
        }

    // internal: only callable from the Settings UI secure overlay, never from executeTool()
    internal fun save(context: Context, appPackage: String, fieldType: String, value: String) {
        try {
            prefs(context).edit().putString(key(appPackage, fieldType), value).apply()
        } catch (_: Exception) {}
    }

    internal fun delete(context: Context, appPackage: String, fieldType: String) {
        try {
            prefs(context).edit().remove(key(appPackage, fieldType)).apply()
        } catch (_: Exception) {}
    }
}
