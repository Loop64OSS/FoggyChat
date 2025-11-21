#![allow(unused_imports)]
mod crypt;
mod protocol_utils;

use aes_gcm::{Aes256Gcm, Key, KeyInit};
use crypt::asymmetric;
use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use protocol_utils::fctp;
use protocol_utils::fctp_me;
use protocol_utils::fctp_secure::{
    get_e2ee_pub, get_exchange_pub, get_exchange_sec, get_session_key, set_e2ee, set_exchange,
    set_session_key, E2EE_KEY_TABLE,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout, Instant};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::protocol_utils::fctp::LAST_ACK;
use crate::protocol_utils::fctp_secure::handle_non_existent_e2ee_key;
use crate::protocol_utils::fctp_secure::remove_pk_from_e2ee_key_table;

// Constants
const BUFFER_SIZE: usize = 8192;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const PONG_TIMEOUT_SECS: u64 = 240;
const PING_INTERVAL_SECS: u64 = 120;

// Global state
static TASKS: Lazy<Arc<Mutex<Vec<JoinHandle<()>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));
static TX: Lazy<Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd",
    ))]
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| Ok(()))
        .invoke_handler(tauri::generate_handler![
            ui_command_send_fctp_message,
            ui_command_request_connection,
            ui_command_status,
            ui_command_select_recipient,
            ui_command_remove_recipient
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn ui_command_send_fctp_message(
    message: &str,
    recipient: &str,
    app: AppHandle,
) -> Result<(), String> {
    // Validate inputs
    if message.is_empty() {
        return Err("Message cannot be empty".into());
    }
    if recipient.is_empty() {
        return Err("Recipient cannot be empty".into());
    }

    if message.starts_with("/") {
        handle_command_message(message, &app).await
    } else {
        handle_regular_message(message, recipient, &app).await
    }
}

async fn handle_command_message(message: &str, _app: &AppHandle) -> Result<(), String> {
    let command = message.trim_start_matches('/').trim();

    let server_id = fctp::get_server_id().map_err(|e| format!("Failed to get server ID: {}", e))?;

    let packet = fctp::encapsulate_to_fctp(
        &fctp::FctpMessage::new(
            fctp::FctpCode::Command,
            fctp_me::get_id(),
            command,
            server_id,
        ),
        &get_session_key(),
    )
    .map_err(|e| format!("FctpError: {:?}", e))?;

    send_packet(packet).await
}

async fn handle_regular_message(
    message: &str,
    recipient: &str,
    app: &AppHandle,
) -> Result<(), String> {
    fctp::LAST_ACK.store(false, Ordering::Relaxed);
    fctp::REQUEST_ERROR.store(false, Ordering::Relaxed);

    fctp::set_current_recipient(recipient.trim())
        .map_err(|e| format!("Failed to set recipient: {}", e))?;

    let pk = protocol_utils::fctp_secure::get_pk_from_e2ee_key_table(recipient.trim())
        .ok_or("Failed to retrieve recipient's public key")?;

    // Encrypt and send
    let encrypted = crypt::asymmetric::encrypt(&pk, message.trim().as_bytes())
        .map_err(|e| format!("Encryption error: {:?}", e))?;

    let _server_id =
        fctp::get_server_id().map_err(|e| format!("Failed to get server ID: {}", e))?;

    let packet = fctp::encapsulate_to_fctp(
        &fctp::FctpMessage::new(
            fctp::FctpCode::Message,
            fctp_me::get_id(),
            crypt::utils::base64_encode(&encrypted).trim(),
            recipient.trim(),
        ),
        &get_session_key(),
    )
    .map_err(|e| format!("FctpError: {:?}", e))?;

    send_packet(packet).await?;

    // Show message in UI if no error occurred
    if !fctp::REQUEST_ERROR.load(Ordering::Relaxed) {
        let formatted_msg = format!("[You] {}", message.trim());
        fctp::ui_emit_fctp_message(app, formatted_msg);
    }

    Ok(())
}

async fn send_packet(packet: Vec<u8>) -> Result<(), String> {
    let tx_guard = TX.lock().await;
    let tx = tx_guard.as_ref().ok_or("Channel sender not initialized")?;

    tx.send(packet)
        .await
        .map_err(|e| format!("Failed to send packet: {}", e))
}

#[tauri::command]
fn ui_command_request_connection(address: &str, app: AppHandle) {
    let address = address.to_owned();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = init_connection(app.clone(), address).await {
            ui_emit_status(app, format!("E::Connection failed: {}", e));
        }
    });
}

#[tauri::command]
async fn ui_command_status(input: String) {
    if matches!(
        input.as_str(),
        "USER::FP_MATCH" | "USER::DISCONNECT" | "USER::FP_MISMATCH"
    ) {
        if let Some(tx) = &*TX.lock().await {
            let _ = tx.send(input.into_bytes()).await;
        } else {
            eprintln!("Channel sender not initialized");
        }
    }
}

#[tauri::command]
async fn ui_command_select_recipient(recipient: String) {
    // Ensure the backend knows the currently selected recipient so
    // any incoming E2EE key response is stored under the correct name.
    if let Err(e) = fctp::set_current_recipient(recipient.trim()) {
        eprintln!("Failed to set current recipient: {}", e);
    }

    // Request public key if not cached
    if !protocol_utils::fctp_secure::has_pk_in_e2ee_key_table(recipient.trim())
        && recipient != "server"
        && !recipient.is_empty()
    {
        let _ = handle_non_existent_e2ee_key(recipient.trim()).await;
    }
}
#[tauri::command]
async fn ui_command_remove_recipient(recipient: String) {
    remove_pk_from_e2ee_key_table(recipient.trim());
}
pub fn ui_emit_status(app: AppHandle, msg: String) {
    if let Err(e) = app.emit("status", msg) {
        eprintln!("Failed to emit status: {:?}", e);
    }
}

async fn init_connection(app: AppHandle, server_address: String) -> Result<(), String> {
    let stream = timeout(CONNECTION_TIMEOUT, TcpStream::connect(&server_address))
        .await
        .map_err(|_| "Connection timeout".to_string())?
        .map_err(|e| format!("Connection error: {}", e))?;

    stream_handler(app, stream).await;
    Ok(())
}

async fn abort_all_tasks() {
    let mut tasks = TASKS.lock().await;
    for task in tasks.drain(..) {
        task.abort();
    }
}

async fn cleanup_connection(app: AppHandle) {
    set_session_key(Key::<Aes256Gcm>::default());
    set_exchange(PublicKey::from([0u8; 32]), StaticSecret::from([0u8; 32]));

    if let Ok(mut table) = E2EE_KEY_TABLE.write() {
        table.clear();
    }

    *TX.lock().await = None;
    abort_all_tasks().await;
    ui_emit_status(app, "USER::DISCONNECT".to_string());
}

async fn handle_server_message(
    data: &[u8],
    app: &AppHandle,
    tx: &mpsc::Sender<Vec<u8>>,
    fp_verified: &Arc<AtomicBool>,
    last_pong: &Arc<Mutex<Instant>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if data.is_empty() {
        return Ok(());
    }

    // During handshake (no session key), handle raw data
    // After handshake (session key set), handle FCTP messages
    if get_session_key() == Key::<Aes256Gcm>::default() {
        protocol_utils::fctp_secure::handle_handshake_message(data, app, tx, fp_verified).await
    } else {
        // This is an encrypted FCTP message
        fctp::process_fctp_stream(app.clone(), data, last_pong).await;
        Ok(())
    }
}

async fn stream_handler(app: AppHandle, stream: TcpStream) {
    let (mut reader, writer) = stream.into_split();
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);
    let last_pong = Arc::new(Mutex::new(Instant::now()));
    let last_pong_clone = last_pong.clone();
    let fp_verified = Arc::new(AtomicBool::new(false));

    TX.lock().await.replace(tx.clone());

    // Reader task
    {
        let tx_clone = tx.clone();
        let app_clone = app.clone();
        let fp_verified_clone = fp_verified.clone();

        let handle = tokio::spawn(async move {
            let mut buffer = [0u8; BUFFER_SIZE];
            let mut message_buffer = Vec::with_capacity(BUFFER_SIZE * 2);
            let mut expecting_length: Option<usize> = None;

            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Server disconnected");
                        break;
                    }
                    Ok(n) => {
                        if get_session_key() == Key::<Aes256Gcm>::default() {
                            // Handshake phase
                            let data = &buffer[..n];
                            let text_data = String::from_utf8_lossy(data);

                            if text_data.contains("\r\n\r\n") {
                                for line in text_data.lines() {
                                    let trimmed = line.trim();
                                    if !trimmed.is_empty() {
                                        if let Err(e) = handle_server_message(
                                            trimmed.as_bytes(),
                                            &app_clone,
                                            &tx_clone,
                                            &fp_verified_clone,
                                            &last_pong_clone,
                                        )
                                        .await
                                        {
                                            eprintln!("Error handling handshake: {}", e);
                                        }
                                        break;
                                    }
                                }
                            }
                        } else {
                            // Encrypted message phase
                            message_buffer.extend_from_slice(&buffer[..n]);

                            // Prevent buffer overflow
                            if message_buffer.len() > BUFFER_SIZE * 4 {
                                eprintln!("Message buffer overflow, clearing");
                                message_buffer.clear();
                                expecting_length = None;
                                continue;
                            }

                            while message_buffer.len() >= 4 {
                                if expecting_length.is_none() {
                                    let length_bytes: [u8; 4] =
                                        message_buffer[..4].try_into().unwrap();
                                    let message_length = u32::from_be_bytes(length_bytes) as usize;

                                    // Sanity check
                                    if message_length > BUFFER_SIZE * 4 {
                                        eprintln!("Invalid message length: {}", message_length);
                                        message_buffer.clear();
                                        expecting_length = None;
                                        break;
                                    }

                                    expecting_length = Some(message_length);
                                }

                                if let Some(msg_len) = expecting_length {
                                    let total_length = 4 + msg_len + 12; // length + message + nonce

                                    if message_buffer.len() >= total_length {
                                        let message_data = message_buffer
                                            .drain(..total_length)
                                            .collect::<Vec<u8>>();
                                        expecting_length = None;

                                        if let Err(e) = handle_server_message(
                                            &message_data,
                                            &app_clone,
                                            &tx_clone,
                                            &fp_verified_clone,
                                            &last_pong_clone,
                                        )
                                        .await
                                        {
                                            eprintln!("Error handling encrypted message: {}", e);
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading from server: {}", e);
                        break;
                    }
                }
            }

            // Trigger disconnect
            if let Some(tx) = &*TX.lock().await {
                let _ = tx.send("USER::DISCONNECT".as_bytes().to_vec()).await;
            }
        });
        TASKS.lock().await.push(handle);
    }

    // Writer task
    {
        let app_clone = app.clone();
        let fp_verified_clone = fp_verified.clone();

        let handle = tokio::spawn(async move {
            let mut writer = writer;
            while let Some(msg) = rx.recv().await {
                // Handle control messages
                if msg == b"USER::FP_MATCH" {
                    fp_verified_clone.store(true, Ordering::Relaxed);
                    continue;
                } else if msg == b"USER::DISCONNECT" || msg == b"USER::FP_MISMATCH" {
                    cleanup_connection(app_clone.clone()).await;
                    return;
                }

                if let Err(e) = writer.write_all(&msg).await {
                    eprintln!("Failed to send message: {:?}", e);
                    break;
                }
            }
        });
        TASKS.lock().await.push(handle);
    }

    // Pong monitor task
    {
        let last_pong_clone = last_pong.clone();
        let handle = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(3)).await;
                let elapsed = last_pong_clone.lock().await.elapsed();

                if get_session_key() == Key::<Aes256Gcm>::default() {
                    *last_pong_clone.lock().await = Instant::now();
                } else if elapsed.as_secs() > PONG_TIMEOUT_SECS {
                    println!(
                        "Server not responding, last pong: {} ms ago",
                        elapsed.as_millis()
                    );
                }
            }
        });
        TASKS.lock().await.push(handle);
    }

    // Ping task
    {
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(PING_INTERVAL_SECS)).await;

                if get_session_key() != Key::<Aes256Gcm>::default() {
                    let server_id = fctp::get_server_id().unwrap_or_default();
                    let ping_packet = match fctp::encapsulate_to_fctp(
                        &fctp::FctpMessage::ping(server_id),
                        &get_session_key(),
                    ) {
                        Ok(packet) => packet,
                        Err(e) => {
                            eprintln!("FctpError: {:?}", e);
                            continue;
                        }
                    };

                    if tx_clone.send(ping_packet).await.is_err() {
                        break;
                    }
                }
            }
        });
        TASKS.lock().await.push(handle);
    }

    // Keep connection alive
    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypt::symmetric;
    use x25519_dalek::{PublicKey, StaticSecret};

    #[test]
    fn test_symmetric_encryption() {
        let key = symmetric::keygen();
        let plaintext = "test message";

        let encrypted =
            symmetric::encrypt_binary(plaintext, &key).expect("Encryption should succeed");
        let decrypted =
            symmetric::decrypt_binary(&encrypted, &key).expect("Decryption should succeed");

        assert_eq!(String::from_utf8_lossy(&decrypted), plaintext);
    }

    #[test]
    fn test_asymmetric_encryption() {
        let (rec_sec_bytes, rec_pub_bytes) = asymmetric::keypairgen();
        let rec_sec = StaticSecret::from(rec_sec_bytes);
        let rec_pub = PublicKey::from(rec_pub_bytes);
        let plaintext = b"test message";

        let encrypted =
            asymmetric::encrypt(&rec_pub, plaintext).expect("Encryption should succeed");
        let decrypted =
            asymmetric::decrypt(&rec_sec, &encrypted).expect("Decryption should succeed");

        assert_eq!(&decrypted[..], plaintext);
    }

    #[test]
    fn test_id_management() {
        fctp_me::set_id("test_client_id");
        assert_eq!(fctp_me::get_id(), "test_client_id");

        fctp::set_server_id("test_server_id").expect("Should set server ID");
        assert_eq!(fctp::get_server_id().unwrap(), "test_server_id");
    }
}
