mod ai;
mod chat;
mod device;
mod network;
mod skills;
mod social;
mod session;
mod tools;
mod files;
mod live2d_key;
#[cfg(target_os = "linux")]
mod live2d_overlay;

use chat::{get_memory_file, set_memory_file, list_chats, load_chat_messages, create_chat, save_chat_messages, delete_chat, list_memos, load_memo_messages, create_memo, save_memo_messages, delete_memo};
use files::{read_file_text, extract_file_text_from_bytes, read_file_as_base64, get_clipboard_image, get_clipboard_uri_list};
use social::character::{list_characters, save_character, delete_character};
use social::post::{list_posts, save_post, delete_post, like_post, unlike_post, list_comments, add_comment, generate_character_post, trigger_character_reactions, generate_character_dm, react_to_user_post, react_to_user_comment, resume_post_gen_queue, hide_post, record_post_preference, get_due_posts, mark_post_generated};
use ai::{cancel_chat, chat_claude, chat_ollama, list_models, list_models_at, explain_text};
use device::stt::{stt_android_cancel, stt_android_once, stt_start, stt_stop};
use device::secrets::{store_secret, load_secret};
use session::{add_paired_device, get_session, list_personas, remove_paired_device, set_device_label, set_ollama_endpoint, set_pc_permissions, set_persona, set_session_hash_key};
use skills::{create_persona_background, get_persona_build_status, clear_persona_build_status};
use tools::{respond_pc_permission, PendingPermissions, respond_ask_user, PendingAskUserRequests};
use device::phone::{check_accessibility_enabled, open_accessibility_settings};
use network::{check_peer_online, discover_and_pair, get_all_local_addresses, get_all_peer_status, get_local_address, get_qr_pair_svg, pair_from_qr, send_to_device, start_bridge_server, start_peer_monitor};
use network::delivery::flush_all_pending;
use social::queue::commands::{flush_queue, get_pending_queue, get_queue, queue_command};
use social::queue::commands::{get_post_gen_queue, get_post_gen_pending, cleanup_post_gen_stale};
use social::config::{get_social_config, save_social_config};

// ── Live2D native GTK overlay commands (Linux only) ──────────────────────────

#[cfg(target_os = "linux")]
#[tauri::command]
fn send_live2d_frame(
    sender: tauri::State<'_, live2d_overlay::OverlaySender>,
    latest: tauri::State<'_, live2d_overlay::LatestFrame>,
    request: tauri::ipc::Request<'_>,
) {
    let tauri::ipc::InvokeBody::Raw(data) = request.body() else { return; };
    if data.len() < 8 { return; }
    let width  = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let height = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let pixels = data[8..].to_vec();
    // Overwrite latest-frame slot — any undrawn frame already in the glib
    // channel will find None and become a no-op, so Y-flip only runs once.
    *latest.0.lock().unwrap() = Some((pixels, width, height));
    let _ = sender.0.lock().unwrap().try_send(live2d_overlay::OverlayCmd::DrawLatest);
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn send_live2d_frame(_request: tauri::ipc::Request<'_>) {}

#[cfg(target_os = "linux")]
#[tauri::command]
fn show_live2d_overlay(
    state: tauri::State<'_, live2d_overlay::OverlaySender>,
    x: i32, y: i32, width: i32, height: i32,
    nat_aspect: f64,
) {
    let _ = state.0.lock().unwrap()
        .try_send(live2d_overlay::OverlayCmd::Show { x, y, width, height, nat_aspect });
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn show_live2d_overlay(_x: i32, _y: i32, _width: i32, _height: i32, _nat_aspect: f64) {}

#[cfg(target_os = "linux")]
#[tauri::command]
fn hide_live2d_overlay(state: tauri::State<'_, live2d_overlay::OverlaySender>) {
    let _ = state.0.lock().unwrap().try_send(live2d_overlay::OverlayCmd::Hide);
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn hide_live2d_overlay() {}

// ─────────────────────────────────────────────────────────────────────────────

/// App entry point — registers Tauri commands and starts the event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(dependency_on_unit_never_type_fallback)]
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(device::phone::plugin::init());

    #[cfg(not(target_os = "android"))]
    {
        // Exclude the live2d overlay window from state persistence —
        // restoring stale size/position cascades into wrong canvas size,
        // wrong initial fit, and wrong character bounds on every open.
        builder = builder.plugin(
            tauri_plugin_window_state::Builder::new()
                .with_filter(|label| label != "live2d")
                .skip_initial_state("live2d")
                .build(),
        );
    }

    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }

    builder
        .manage(PendingPermissions(std::sync::Mutex::new(std::collections::HashMap::new())))
        .manage(PendingAskUserRequests(std::sync::Mutex::new(std::collections::HashMap::new())))
        .register_uri_scheme_protocol("live2d", |app, request| {
            use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};
            use tauri::Manager;

            let uri_path = request.uri().path().trim_start_matches('/');
            let enc_filename = format!("{}.enc", uri_path);

            let resource_path = match app.app_handle().path().resource_dir() {
                Ok(dir) => dir.join("live2d-encrypted").join(&enc_filename),
                Err(_) => return tauri::http::Response::builder()
                    .status(500).body(vec![]).unwrap(),
            };

            let raw = match std::fs::read(&resource_path) {
                Ok(d) => d,
                Err(_) => return tauri::http::Response::builder()
                    .status(404).body(vec![]).unwrap(),
            };

            if raw.len() < 28 {
                return tauri::http::Response::builder()
                    .status(400).body(vec![]).unwrap();
            }

            let iv = &raw[0..12];
            let tag = &raw[12..28];
            let ciphertext = &raw[28..];

            let key = Key::<Aes256Gcm>::from_slice(live2d_key::KEY);
            let cipher = Aes256Gcm::new(key);
            let nonce = Nonce::from_slice(iv);

            // aes-gcm expects tag appended to ciphertext for decryption
            let mut payload = ciphertext.to_vec();
            payload.extend_from_slice(tag);

            let decrypted = match cipher.decrypt(nonce, payload.as_ref()) {
                Ok(d) => d,
                Err(_) => return tauri::http::Response::builder()
                    .status(403).body(vec![]).unwrap(),
            };

            let mime = match uri_path.rsplit('.').next().unwrap_or("") {
                "json" => "application/json",
                "png"  => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "moc3" | "bin" => "application/octet-stream",
                _ => "application/octet-stream",
            };

            tauri::http::Response::builder()
                .header("Content-Type", mime)
                .header("Access-Control-Allow-Origin", "*")
                .body(decrypted)
                .unwrap()
        })
        .setup(|app| {
            // 0. On Linux: create the native GTK overlay window and register it
            //    as Tauri state so commands can send frames to it.
            //    Must be done here (after GTK is initialized by Tauri) not in run().
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager;
                let latest = live2d_overlay::LatestFrame(
                    std::sync::Arc::new(std::sync::Mutex::new(None::<live2d_overlay::LatestFrameData>))
                );
                let overlay_tx = live2d_overlay::create_overlay(app.handle().clone(), latest.0.clone());
                app.manage(live2d_overlay::OverlaySender(std::sync::Mutex::new(overlay_tx)));
                app.manage(latest);
            }

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

            // 4. On startup: resume any interrupted post generation tasks and cleanup stale entries.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Resume pending post generation
                let _ = crate::social::post::resume_post_gen_queue(handle.clone()).await;
                // Cleanup entries older than 7 days
                let _ = cleanup_post_gen_stale(handle);
            });

            // 5. Grant microphone (and other media) permission requests from the WebView.
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
            // posts
            list_posts,
            save_post,
            delete_post,
            like_post,
            unlike_post,
            list_comments,
            add_comment,
            generate_character_post,
            trigger_character_reactions,
            generate_character_dm,
            react_to_user_post,
            react_to_user_comment,
            resume_post_gen_queue,
            hide_post,
            record_post_preference,
            get_due_posts,
            mark_post_generated,
            // session / pairing
            get_session,
            set_device_label,
            set_session_hash_key,
            set_ollama_endpoint,
            list_personas,
            set_persona,
            set_pc_permissions,
            add_paired_device,
            remove_paired_device,
            // pc control permissions
            respond_pc_permission,
            // ask_user interactive bubble
            respond_ask_user,
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
            // post generation queue
            get_post_gen_queue,
            get_post_gen_pending,
            cleanup_post_gen_stale,
            // social config
            get_social_config,
            save_social_config,
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
            // persona creation
            create_persona_background,
            get_persona_build_status,
            clear_persona_build_status,
            // chat control
            cancel_chat,
            // file reading
            read_file_text,
            extract_file_text_from_bytes,
            read_file_as_base64,
            get_clipboard_image,
            get_clipboard_uri_list,
            // explain + memos
            explain_text,
            list_memos,
            load_memo_messages,
            create_memo,
            save_memo_messages,
            delete_memo,
            // live2d native GTK overlay (Linux)
            send_live2d_frame,
            show_live2d_overlay,
            hide_live2d_overlay,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

