use tauri::{plugin::{Builder, TauriPlugin}, Runtime};
#[cfg(target_os = "android")]
use tauri::Manager;

/// Wraps the Android-side PhoneControlPlugin handle so it can be stored in Tauri state
/// and retrieved in other modules without depending on the non-existent
/// `AppHandle::run_mobile_plugin` API.
/// Only defined on Android because `PluginHandle::run_mobile_plugin` is Android-only.
#[cfg(target_os = "android")]
pub struct PhoneControlHandle<R: Runtime>(pub tauri::plugin::PluginHandle<R>);

/// Initializes the phoneControl Tauri plugin.
/// On Android this connects to the Kotlin PhoneControlPlugin via JNI and stores
/// the resulting handle in app state so `apps.rs` and `tools.rs` can retrieve it.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("phoneControl")
        .setup(|_app, api| {
            #[cfg(target_os = "android")]
            {
                // Package = Java package of PhoneControlPlugin.kt
                // Class   = simple class name (no package prefix)
                let handle =
                    api.register_android_plugin("com.uty.phoneclaw", "PhoneControlPlugin")?;
                _app.manage(PhoneControlHandle(handle));
            }
            let _ = api; // suppress unused warning on non-Android
            Ok(())
        })
        .build()
}
