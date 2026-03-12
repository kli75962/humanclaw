package com.uty.phoneclaw

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {

  companion object {
    private const val REQUEST_CAMERA = 1001
    private const val PREFS_NAME = "phoneclaw_prefs"
    private const val KEY_PERMISSIONS_ASKED = "permissions_asked"
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    ViewCompat.setOnApplyWindowInsetsListener(findViewById(android.R.id.content)) { view, insets ->
      val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
      view.setPadding(systemBars.left, systemBars.top, systemBars.right, systemBars.bottom)
      insets
    }

    val prefs = getSharedPreferences(PREFS_NAME, MODE_PRIVATE)
    if (!prefs.getBoolean(KEY_PERMISSIONS_ASKED, false)) {
      prefs.edit().putBoolean(KEY_PERMISSIONS_ASKED, true).apply()
      requestInitialPermissions()
    }
  }

  private fun requestInitialPermissions() {
    if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA)
      != PackageManager.PERMISSION_GRANTED) {
      ActivityCompat.requestPermissions(this, arrayOf(Manifest.permission.CAMERA), REQUEST_CAMERA)
    }
  }
}
