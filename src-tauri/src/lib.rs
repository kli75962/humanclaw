mod memory;
mod characters;
mod model;
mod phone;
mod tools;
mod skills;
mod session;
mod bridge;
mod queue;
mod stt;
mod secrets;

use memory::{get_memory_file, set_memory_file, list_chats, load_chat_messages, create_chat, save_chat_messages, delete_chat};
use characters::{list_characters, save_character, delete_character};
use model::{cancel_chat, chat_claude, chat_ollama, list_models, list_models_at};
use stt::{stt_android_cancel, stt_android_once, stt_start, stt_stop};
use secrets::{store_secret, load_secret};
use session::{add_paired_device, get_session, list_personas, remove_paired_device, set_device_label, set_ollama_endpoint, set_persona, set_session_hash_key};
use phone::{check_accessibility_enabled, open_accessibility_settings};
use bridge::{check_peer_online, discover_and_pair, get_all_local_addresses, get_all_peer_status, get_local_address, get_qr_pair_svg, pair_from_qr, send_to_device, start_bridge_server, start_peer_monitor};
use queue::{flush_all_pending, flush_queue, get_pending_queue, get_queue, queue_command};

/// App entry point — registers Tauri commands and starts the event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .secrets — desktop only (Android has no access to the host filesystem).
    #[cfg(not(target_os = "android"))]
    {
        let secrets_path = concat!(env!("CARGO_MANIFEST_DIR"), "/.secrets");
        let _ = dotenvy::from_filename(secrets_path);
    }

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(phone::plugin::init());

    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }

    builder
        .setup(|app| {
            // 1. Start the bridge HTTP server so peers can reach this device.
            start_bridge_server(app.handle().clone());

            // 2. Start the background peer health monitor (emits peer-status-changed events).
            start_peer_monitor(app.handle().clone());

            // 3. On startup: try to deliver any messages that were queued while
            //    the target device was offline.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                flush_all_pending(&handle).await;
            });

            // 4. Grant microphone (and other media) permission requests from the WebView.
            //    On Linux/WebKit, permission-request signals are silently ignored unless
            //    we explicitly allow them here.
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager;
                use webkit2gtk::{PermissionRequest, PermissionRequestExt, WebViewExt};
                if let Some(window) = app.handle().get_webview_window("main") {
                    let _ = window.with_webview(|wv| {
                        wv.inner().connect_permission_request(
                            |_wv: &webkit2gtk::WebView, request: &PermissionRequest| {
                                request.allow();
                                true
                            },
                        );
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            chat_ollama,
            chat_claude,
            list_models,
            list_models_at,
            get_memory_file,
            set_memory_file,
            list_chats,
            load_chat_messages,
            create_chat,
            save_chat_messages,
            delete_chat,
            // characters
            list_characters,
            save_character,
            delete_character,
            // session / pairing
            get_session,
            set_device_label,
            set_session_hash_key,
            set_ollama_endpoint,
            list_personas,
            set_persona,
            add_paired_device,
            remove_paired_device,
            // bridge / health
            check_peer_online,
            get_all_peer_status,
            send_to_device,
            get_local_address,
            get_all_local_addresses,
            discover_and_pair,
            get_qr_pair_svg,
            pair_from_qr,
            // queue
            get_queue,
            get_pending_queue,
            queue_command,
            flush_queue,
            // stt
            stt_android_cancel,
            stt_android_once,
            stt_start,
            stt_stop,
            // secrets
            store_secret,
            load_secret,
            // phone
            check_accessibility_enabled,
            open_accessibility_settings,
            // chat control
            cancel_chat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

