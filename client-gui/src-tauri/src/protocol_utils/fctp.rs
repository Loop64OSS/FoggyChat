use aes_gcm::{Aes256Gcm, Key};
use lazy_static::lazy_static;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::{sync::Mutex, time::Instant};

use crate::{
    crypt::symmetric,
    protocol_utils::{self, fctp_secure::get_session_key},
};

pub struct FctpMessage {
    pub code: i32,
    pub from: String,
    pub body: String,
    pub to: String,
}

/*
    Set global user ID using lazy_static
*/

lazy_static! {
    static ref SERVER_ID: RwLock<String> = RwLock::new(String::new());
}

pub fn set_server_id(new_id: &str) {
    let mut id = SERVER_ID.write().expect("Lock poisoned");
    *id = new_id.to_string();
}

pub fn get_server_id() -> String {
    SERVER_ID.read().expect("Lock poisoned").clone()
}

/*
    FCTP message processing with binary encryption
*/
pub fn encapsulate_to_fctp(
    code: i32,
    from: &str,
    body: &str,
    to: &str,
    session_key: Key<Aes256Gcm>,
) -> Vec<u8> {
    let message = format!(
        "FoggyChat Transfer Protocol 0.1\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n\r\n",
        code, from, body, to
    );

    if session_key == Key::<Aes256Gcm>::default() {
        return message.into_bytes();
    }

    match symmetric::encrypt_binary(&message, &session_key) {
        Ok(encrypted) => encrypted,
        Err(e) => {
            eprintln!("Failed to encrypt FCTP message: {:?}", e);
            Vec::new()
        }
    }
}

pub fn decapsulate_fctp_message(msg: &[u8], session_key: Key<Aes256Gcm>) -> Option<FctpMessage> {
    let decrypted_str = if session_key == Key::<Aes256Gcm>::default() {
        String::from_utf8(msg.to_vec()).ok()?
    } else {
        let decrypted_bytes = symmetric::decrypt_binary(msg, &session_key).ok()?;
        String::from_utf8(decrypted_bytes).ok()?
    };

    parse_fctp_message(&decrypted_str)
}

fn parse_fctp_message(message: &str) -> Option<FctpMessage> {
    let mut lines = message.lines();

    if lines.next()? != "FoggyChat Transfer Protocol 0.1" {
        return None;
    }

    let code = lines.next()?.trim().parse::<i32>().ok()?;
    let from = lines.next()?.strip_prefix("From: ")?.trim().to_string();
    let body = lines.next()?.strip_prefix("Body: ")?.trim().to_string();
    let to = lines.next()?.strip_prefix("To: ")?.trim().to_string();

    Some(FctpMessage {
        code,
        from,
        body,
        to,
    })
}

pub fn ui_emit_fctp_message(app: &AppHandle, msg: String) {
    if let Err(e) = app.emit("fctp-message", msg) {
        eprintln!("Failed to emit message: {:?}", e);
    }
}

pub async fn process_fctp_stream(
    app: tauri::AppHandle,
    message: &[u8],
    last_pong: &Arc<Mutex<Instant>>,
) {
    if let Some(msg) = decapsulate_fctp_message(message, get_session_key()) {
        match msg.code {
            200 => {
                let formatted_msg = format!("<{}> {}", msg.from, msg.body);
                ui_emit_fctp_message(&app, formatted_msg);

                if let Err(e) = app
                    .notification()
                    .builder()
                    .title("FoggyChat")
                    .body(format!("{}: {}", msg.from, msg.body))
                    .show()
                {
                    eprintln!("Failed to show notification: {:?}", e);
                }
            }
            201 => {
                ui_emit_fctp_message(&app, format!("|SERVER| {}", msg.body));
            }
            405 => {
                ui_emit_fctp_message(&app, format!("[!Client error!] {}", msg.body));
            }
            505 => {
                ui_emit_fctp_message(&app, format!("[!Server error!] {}", msg.body));
            }
            900 => {
                if msg.body == "id" {
                    let id_clone = msg.to.clone();
                    let server_id_clone = msg.from.clone();
                    protocol_utils::fctp_me::set_id(&id_clone);
                    set_server_id(&server_id_clone);
                }
            }
            11 => {
                // Pong handling
                let mut pong_time = last_pong.lock().await;
                *pong_time = Instant::now();
            }
            _ => {
                ui_emit_fctp_message(
                    &app,
                    format!(
                    "[!Unsupported code!]: {}\nUpdate your client or ask server admin to update!",
                    msg.code
                ),
                );
            }
        }
    } else {
        let msg_preview = String::from_utf8_lossy(message);
        let preview = if msg_preview.len() > 100 {
            format!("{}...", &msg_preview[..100])
        } else {
            msg_preview.to_string()
        };
        ui_emit_fctp_message(&app, format!("[!Malformed message!]: {}", preview));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::symmetric;

    #[test]
    fn test_fctp_binary_encapsulation_and_decapsulation() {
        let code = 200;
        let from = "user123";
        let body = "Hello, world!";
        let to = "user456";

        let real_key = symmetric::keygen();
        let encrypted_message = encapsulate_to_fctp(code, from, body, to, real_key);

        assert!(!encrypted_message.is_empty());

        let decoded = decapsulate_fctp_message(&encrypted_message, real_key)
            .expect("Failed to parse FCTP message");

        assert_eq!(decoded.code, code);
        assert_eq!(decoded.from, from);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.to, to);
    }

    #[test]
    fn test_fctp_default_key() {
        let code = 900;
        let from = "server";
        let body = "id";
        let to = "client";

        let default_key = Key::<Aes256Gcm>::default();
        let message = encapsulate_to_fctp(code, from, body, to, default_key);

        let decoded = decapsulate_fctp_message(&message, default_key)
            .expect("Failed to parse unencrypted FCTP message");

        assert_eq!(decoded.code, code);
        assert_eq!(decoded.from, from);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.to, to);
    }

    #[test]
    fn test_parse_fctp_message_direct() {
        let valid_message = "FoggyChat Transfer Protocol 0.1\r\n200\r\nFrom: test\r\nBody: hello world\r\nTo: user\r\n\r\n";
        let parsed = parse_fctp_message(valid_message).expect("Should parse valid message");

        assert_eq!(parsed.code, 200);
        assert_eq!(parsed.from, "test");
        assert_eq!(parsed.body, "hello world");
        assert_eq!(parsed.to, "user");
    }

    #[test]
    fn test_server_id_management() {
        set_server_id("test_server_123");
        assert_eq!(get_server_id(), "test_server_123");

        set_server_id("another_server");
        assert_eq!(get_server_id(), "another_server");
    }

    #[test]
    fn test_invalid_messages() {
        let real_key = symmetric::keygen();

        let empty_message = b"";
        assert!(decapsulate_fctp_message(empty_message, real_key).is_none());

        let bad_binary = b"This is not valid encrypted data";
        assert!(decapsulate_fctp_message(bad_binary, real_key).is_none());
    }
}
