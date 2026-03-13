#[cfg(not(target_os = "android"))]
const SERVICE: &str = "phoneclaw";

/// Store a secret in the OS keychain.
/// - Desktop: OS keychain (Keychain / SecretService / Credential Manager)
/// - Android: no-op (Android uses native STT, no API key needed)
#[tauri::command]
pub fn store_secret(key: String, value: String) -> Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        keyring::Entry::new(SERVICE, &key)
            .map_err(|e| e.to_string())?
            .set_password(&value)
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "android")]
    {
        let _ = (key, value);
        Ok(())
    }
}

/// Load a secret from the OS keychain. Returns None if not found.
#[tauri::command]
pub fn load_secret(key: String) -> Result<Option<String>, String> {
    #[cfg(not(target_os = "android"))]
    {
        let entry = keyring::Entry::new(SERVICE, &key).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(val) => Ok(Some(val)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    #[cfg(target_os = "android")]
    {
        let _ = key;
        Ok(None)
    }
}
