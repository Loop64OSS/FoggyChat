#![allow(unused_imports)]
mod crypt;
mod protocol_utils;

use aes_gcm::{Aes256Gcm, Key, KeyInit};
use crypt::asymmetric;
use crypt::symmetric;
use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use protocol_utils::fctp;
use protocol_utils::fctp::get_server_id;
use protocol_utils::fctp::ui_emit_fctp_message;
use protocol_utils::fctp_me;
use protocol_utils::fctp_secure::get_e2ee_pub;
use protocol_utils::fctp_secure::set_e2ee;
use regex::Regex;
use sha2::digest::generic_array::GenericArray;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Instant};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::protocol_utils::fctp::get_current_recipient;
use crate::protocol_utils::fctp::set_current_recipient;
use crate::protocol_utils::fctp_secure::get_pk_from_e2ee_key_table;
use crate::protocol_utils::fctp_secure::has_pk_in_e2ee_key_table;
use crate::protocol_utils::fctp_secure::E2EE_KEY_TABLE;
use crate::protocol_utils::fctp_secure::{
    get_exchange_pub, get_exchange_sec, get_session_key, set_exchange, set_session_key,
};

const BUFFER_SIZE: usize = 8192;

static TASKS: Lazy<Arc<Mutex<Vec<JoinHandle<()>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));
static TX: Lazy<Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

//Tauri app starter
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
            ui_command_status
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
    if message.starts_with("/") {
        //Command handling

        let command = message.trim_start_matches('/').trim();
        let packet = fctp::encapsulate_to_fctp(
            201,
            &fctp_me::get_id(),
            command,
            &get_server_id(),
            get_session_key(),
        );

        if let Some(tx) = &*TX.lock().await {
            tx.send(packet)
                .await
                .map_err(|e| format!("Failed to send packet: {}", e))?;
        } else {
            return Err("Channel sender not initialized".into());
        }
        Ok(())
    } else {
        //Message handling
        protocol_utils::fctp::LAST_ACK.store(false, Ordering::Relaxed);
        protocol_utils::fctp::REQUEST_ERROR.store(false, Ordering::Relaxed);

        set_current_recipient(recipient.trim());
        if !has_pk_in_e2ee_key_table(&get_current_recipient().trim()) {
            let packet = fctp::encapsulate_to_fctp(
                902,
                &fctp_me::get_id(),
                recipient.trim(),
                &get_server_id(),
                get_session_key(),
            );
            if let Some(tx) = &*TX.lock().await {
                tx.send(packet)
                    .await
                    .map_err(|e| format!("Failed to send packet: {}", e))?;
            } else {
                return Err("Channel sender not initialized".into());
            }
            while !fctp::LAST_ACK.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(100)).await;
            }
        }
        let Some(pk) = get_pk_from_e2ee_key_table(recipient.trim()) else {
            return Ok(());
        };
        match crypt::asymmetric::encrypt(&pk, &message.trim().as_bytes()) {
            Ok(msg) => {
                let packet = fctp::encapsulate_to_fctp(
                    200,
                    &fctp_me::get_id(),
                    &crypt::utils::base64_encode(&msg).trim(),
                    recipient.trim(),
                    get_session_key(),
                );

                if let Some(tx) = &*TX.lock().await {
                    tx.send(packet)
                        .await
                        .map_err(|e| format!("Failed to send packet: {}", e))?;
                    if !fctp::REQUEST_ERROR.load(Ordering::Relaxed) {
                        let formatted_msg = format!("[You] {}", message.trim());
                        ui_emit_fctp_message(&app, formatted_msg);
                    }
                } else {
                    return Err("Channel sender not initialized".into());
                }
            }
            Err(err) => {
                return Err(format!("Encryption error: {:?}", err));
            }
        }
        Ok(())
    }
}

#[tauri::command]
fn ui_command_request_connection(address: &str, app: AppHandle) {
    tauri::async_runtime::spawn(init_connection(app.clone(), address.to_owned()));
}

#[tauri::command]
async fn ui_command_status(input: String) {
    if input == "USER::FP_MATCH" || input == "USER::DISCONNECT" || input == "USER::FP_MISMATCH" {
        if let Some(tx) = &*TX.lock().await {
            let _ = tx.send(input.into_bytes()).await;
        } else {
            eprintln!("Channel sender not initialized");
        }
    }
}

pub fn ui_emit_status(app: AppHandle, msg: String) {
    if let Err(e) = app.emit("status", msg) {
        eprintln!("Failed to emit status: {:?}", e);
    }
}

async fn init_connection(app: AppHandle, server_address: String) {
    match tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(server_address)).await {
        Ok(Ok(stream)) => {
            stream_handler(app, stream).await;
        }
        Ok(Err(e)) => {
            ui_emit_status(app, format!("E::Connection error: {}", e));
            eprintln!("E::Connection error: {}", e);
        }
        Err(timeout_err) => {
            ui_emit_status(app, "E::Timeout occurred".to_string());
            eprintln!("E::Timeout: {}", timeout_err);
        }
    }
}

async fn abort_all_tasks() {
    let mut tasks = TASKS.lock().await;
    for task in tasks.drain(..) {
        task.abort();
    }
}

async fn handle_server_message(
    data: &[u8],
    app: &AppHandle,
    tx: &mpsc::Sender<Vec<u8>>,
    fp_verified: &Arc<AtomicBool>,
    last_pong: &Arc<Mutex<Instant>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if get_session_key() == Key::<Aes256Gcm>::default() {
        if get_exchange_pub() == PublicKey::from([0u8; 32]) {
            //Certificate Public key
            let message_str = String::from_utf8_lossy(data).trim().to_string();
            match crypt::utils::base64_decode(&message_str) {
                Ok(decoded) => {
                    if decoded.len() == 32 {
                        let cert_pub = PublicKey::from(<[u8; 32]>::try_from(decoded.as_slice())?);
                        //Ask user to verify certificate fingerprint
                        ui_emit_status(
                            app.clone(),
                            format!("USER::VERIFY_FP::{}", crypt::utils::blake3_hash(&decoded)),
                        );
                        //wait for response
                        while !fp_verified.load(Ordering::Relaxed) {
                            sleep(Duration::from_millis(100)).await;
                        }
                        //generate exchange key
                        let (rec_sec_bytes, rec_pub_bytes) = asymmetric::keypairgen();
                        let rec_sec = StaticSecret::from(rec_sec_bytes);
                        let rec_pub = PublicKey::from(rec_pub_bytes);
                        set_exchange(rec_pub, rec_sec);

                        //encrypt exchange key with certificate
                        match asymmetric::encrypt(&cert_pub, rec_pub.as_bytes()) {
                            Ok(encrypted) => {
                                let encoded = crypt::utils::base64_encode(&encrypted);
                                tx.send(format!("{}\r\n\r\n", encoded).into_bytes()).await?;
                                ui_emit_status(app.clone(), "OK::CON_ESTABLISHED".to_string());
                            }
                            Err(e) => eprintln!("Encryption error: {}", e),
                        }
                    }
                }
                Err(e) => eprintln!("Decoding error: {}", e),
            }
        } else {
            //session key
            let message_str = String::from_utf8_lossy(data).trim().to_string();
            match crypt::utils::base64_decode(&message_str) {
                Ok(decoded) => match asymmetric::decrypt(&get_exchange_sec(), &decoded) {
                    Ok(decrypted) => {
                        if decrypted.len() == 32 {
                            let key_array: [u8; 32] = decrypted
                                .try_into()
                                .map_err(|e| format!("An Error Occurred: {:?}", e))?;
                            let session_key = Key::<Aes256Gcm>::from_slice(&key_array).clone();
                            //saving session key
                            set_session_key(session_key);

                            println!(
                                "Session key established: {}",
                                crypt::utils::base64_encode(get_session_key().as_slice())
                            );
                            //Wait for ui
                            sleep(Duration::from_millis(100)).await;
                            //requesting ID

                            let packet = fctp::encapsulate_to_fctp(
                                900,
                                &fctp_me::get_id(),
                                "id",
                                &get_server_id(),
                                get_session_key(),
                            );
                            tx.send(packet).await?;
                            //generating e2ee key pair and sending e2ee public key
                            let (rec_sec_bytes, rec_pub_bytes) = asymmetric::keypairgen();
                            let rec_sec = StaticSecret::from(rec_sec_bytes);
                            let rec_pub = PublicKey::from(rec_pub_bytes);
                            set_e2ee(rec_pub, rec_sec);
                            let e2ee_pub = get_e2ee_pub();
                            let e2ee_pk_bytes = e2ee_pub.as_bytes();
                            let packet = fctp::encapsulate_to_fctp(
                                901,
                                &fctp_me::get_id(),
                                &crypt::utils::base64_encode(e2ee_pk_bytes),
                                &get_server_id(),
                                get_session_key(),
                            );
                            tx.send(packet).await?;
                        }
                    }
                    Err(e) => eprintln!("Session key decryption error: {:?}", e),
                },
                Err(e) => eprintln!("Session key decoding error: {}", e),
            }
        }
    } else {
        fctp::process_fctp_stream(app.clone(), data, last_pong).await;
    }

    Ok(())
}

async fn stream_handler(app: AppHandle, stream: TcpStream) {
    let (mut reader, writer) = stream.into_split();
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);
    let last_pong = Arc::new(Mutex::new(Instant::now()));
    let last_pong_clone = last_pong.clone();
    let fp_verified = Arc::new(AtomicBool::new(false));

    TX.lock().await.replace(tx.clone());

    {
        let tx_clone = tx.clone();
        let app_clone = app.clone();
        let fp_verified_clone = fp_verified.clone();

        let handle = tokio::spawn(async move {
            let mut buffer = [0u8; BUFFER_SIZE];
            let mut message_buffer = Vec::new();
            let mut expecting_length = None;

            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Server disconnected");
                        break;
                    }
                    Ok(n) => {
                        if get_session_key() == Key::<Aes256Gcm>::default() {
                            let data = &buffer[..n];
                            let text_data = String::from_utf8_lossy(data);

                            if text_data.contains("\r\n\r\n") {
                                for line in text_data.lines() {
                                    if !line.trim().is_empty() {
                                        if let Err(e) = handle_server_message(
                                            line.trim().as_bytes(),
                                            &app_clone,
                                            &tx_clone,
                                            &fp_verified_clone,
                                            &last_pong_clone,
                                        )
                                        .await
                                        {
                                            eprintln!("Error handling handshake message: {}", e);
                                        }
                                        break;
                                    }
                                }
                            }
                        } else {
                            message_buffer.extend_from_slice(&buffer[..n]);

                            while message_buffer.len() >= 4 {
                                if expecting_length.is_none() {
                                    let length_bytes: [u8; 4] =
                                        message_buffer[..4].try_into().unwrap();
                                    let message_length = u32::from_be_bytes(length_bytes) as usize;
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
            if let Some(tx) = &*TX.lock().await {
                let _ = tx.send("USER::DISCONNECT".as_bytes().to_vec()).await;
            } else {
                eprintln!("Channel sender not initialized");
            }
        });
        TASKS.lock().await.push(handle);
    }

    {
        let fp_verified_clone = fp_verified.clone();

        let handle = tokio::spawn(async move {
            let mut writer = writer;
            while let Some(msg) = rx.recv().await {
                if msg == "USER::FP_MATCH".as_bytes() {
                    fp_verified_clone.store(true, Ordering::Relaxed);
                    continue;
                } else if msg == "USER::DISCONNECT".as_bytes()
                    || msg == "USER::FP_MISMATCH".as_bytes()
                {
                    set_session_key(Key::<Aes256Gcm>::default());
                    set_exchange(PublicKey::from([0u8; 32]), StaticSecret::from([0u8; 32]));
                    E2EE_KEY_TABLE.write().expect("Lock Poisoned").clear();
                    *TX.lock().await = None;
                    abort_all_tasks().await;
                    ui_emit_status(app, format!("USER::DISCONNECT"));
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

    {
        let last_pong_clone = last_pong.clone();
        let handle = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(3)).await;
                let elapsed = last_pong_clone.lock().await.elapsed();

                if get_session_key() == Key::<Aes256Gcm>::default() {
                    *last_pong_clone.lock().await = Instant::now();
                } else if elapsed.as_secs() > 240 {
                    println!(
                        "Server not responding, last pong: {} ms ago",
                        elapsed.as_millis()
                    );
                }
            }
        });
        TASKS.lock().await.push(handle);
    }

    {
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(120)).await;

                if get_session_key() != Key::<Aes256Gcm>::default() {
                    let ping_packet = fctp::encapsulate_to_fctp(
                        10,
                        &fctp_me::get_id(),
                        "ping",
                        &get_server_id(),
                        get_session_key(),
                    );

                    if tx_clone.send(ping_packet).await.is_err() {
                        break;
                    }
                }
            }
        });
        TASKS.lock().await.push(handle);
    }

    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::{PublicKey, StaticSecret};

    #[test]
    fn test_symmetric_encryption() {
        let key = symmetric::keygen();

        match symmetric::encrypt_binary("test", &key) {
            Ok(encrypted) => match symmetric::decrypt_binary(&encrypted, &key) {
                Ok(decrypted) => {
                    assert_eq!(String::from_utf8_lossy(&decrypted), "test");
                }
                Err(e) => panic!("Decryption error: {}", e),
            },
            Err(e) => panic!("Encryption error: {}", e),
        }
    }

    #[test]
    fn test_asymmetric_encryption() {
        let (rec_sec_bytes, rec_pub_bytes) = asymmetric::keypairgen();
        let rec_sec = StaticSecret::from(rec_sec_bytes);
        let rec_pub = PublicKey::from(rec_pub_bytes);

        match asymmetric::encrypt(&rec_pub, "test".as_bytes()) {
            Ok(encrypted) => match asymmetric::decrypt(&rec_sec, &encrypted) {
                Ok(decrypted) => {
                    let result = String::from_utf8_lossy(&decrypted);
                    assert_eq!(result, "test");
                }
                Err(e) => panic!("Decryption error: {}", e),
            },
            Err(e) => panic!("Encryption error: {}", e),
        }
    }

    #[test]
    fn test_fctp_binary_roundtrip() {
        let code = 200;
        let from = "user123";
        let body = "Hello, world!";
        let to = "user456";

        let key = symmetric::keygen();
        let message = fctp::encapsulate_to_fctp(code, from, body, to, key);

        assert!(!message.is_empty());

        let decoded =
            fctp::decapsulate_fctp_message(&message, key).expect("Failed to parse FCTP message");

        assert_eq!(decoded.code, code);
        assert_eq!(decoded.from, from);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.to, to);
    }

    #[test]
    fn test_fctp_unencrypted_during_handshake() {
        let code = 900;
        let from = "server";
        let body = "id";
        let to = "client";

        let default_key = Key::<Aes256Gcm>::default();
        let message = fctp::encapsulate_to_fctp(code, from, body, to, default_key);

        let decoded = fctp::decapsulate_fctp_message(&message, default_key)
            .expect("Failed to parse unencrypted FCTP message");

        assert_eq!(decoded.code, code);
        assert_eq!(decoded.from, from);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.to, to);
    }

    #[test]
    fn test_fctp_decapsulation_invalid() {
        let real_key = symmetric::keygen();

        let bad_message = b"This is not a valid FCTP message";
        assert!(fctp::decapsulate_fctp_message(bad_message, real_key).is_none());

        let empty_message = b"";
        assert!(fctp::decapsulate_fctp_message(empty_message, real_key).is_none());

        let bad_binary = b"invalid binary data that cannot be decrypted";
        assert!(fctp::decapsulate_fctp_message(bad_binary, real_key).is_none());
    }

    #[test]
    fn test_id_management() {
        fctp_me::set_id("test_client_id");
        assert_eq!(fctp_me::get_id(), "test_client_id");

        fctp::set_server_id("test_server_id");
        assert_eq!(fctp::get_server_id(), "test_server_id");
    }
}
