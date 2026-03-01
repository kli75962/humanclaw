package com.uty.phoneclaw

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // PhoneControlPlugin is registered on the Rust side via api.register_android_plugin
  }
}
