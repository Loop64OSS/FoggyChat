use aes_gcm::{Aes256Gcm, Key};
use lazy_static::lazy_static;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use thiserror::Error;
use tokio::{sync::Mutex, time::Instant};
use uuid::timestamp;
use x25519_dalek::PublicKey;

use crate::{
    crypt::{self, symmetric, utils::base64_decode},
    protocol_utils::{
        self, fctp,
        fctp_me::get_id,
        fctp_secure::{self, get_e2ee_pub, get_session_key, E2EE_KEY_TABLE},
    },
};

//Internal errors
#[derive(Error, Debug)]
pub enum FctpError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Malformed message: {0}")]
    MalformedMessage(String),
    #[error("Invalid protocol version: {0}")]
    InvalidProtocolVersion(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Client not found: {0}")]
    ClientNotFound(String),
    #[error("Invalid nickname: {0}")]
    InvalidNickname(String),
}
//Statics
type Result<T> = std::result::Result<T, FctpError>;

const PROTOCOL_VERSION: &str = "FoggyChat Transfer Protocol 0.1";

// Protocol codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FctpCode {
    Ping = 10,
    Pong = 11,
    Message = 200,
    Command = 201,
    BadRequest = 400,
    MethodNotAllowed = 405,
    InternalServerError = 500,
    ServiceUnavailable = 505,
    Hello = 900,
    PublicKeyExchange = 901,
    KeyRequest = 902,
}

impl FctpCode {
    fn from_i32(code: i32) -> Option<Self> {
        match code {
            10 => Some(Self::Ping),
            11 => Some(Self::Pong),
            200 => Some(Self::Message),
            201 => Some(Self::Command),
            400 => Some(Self::BadRequest),
            405 => Some(Self::MethodNotAllowed),
            500 => Some(Self::InternalServerError),
            505 => Some(Self::ServiceUnavailable),
            900 => Some(Self::Hello),
            901 => Some(Self::PublicKeyExchange),
            902 => Some(Self::KeyRequest),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FctpMessage {
    pub code: FctpCode,
    pub from: String,
    pub body: String,
    pub to: String,
}

impl FctpMessage {
    pub fn new(
        code: FctpCode,
        from: impl Into<String>,
        body: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            code,
            from: from.into(),
            body: body.into(),
            to: to.into(),
        }
    }

    pub fn ping(to: impl Into<String>) -> Self {
        Self::new(FctpCode::Ping, get_id(), "ping", to)
    }

    pub fn pong(to: impl Into<String>) -> Self {
        Self::new(FctpCode::Pong, get_id(), "pong", to)
    }

    pub fn error(code: FctpCode, message: impl Into<String>, to: impl Into<String>) -> Self {
        Self::new(code, get_id(), message, to)
    }
}
/*
    Set global user ID using lazy_static
*/

lazy_static! {
    static ref SERVER_ID: RwLock<String> = RwLock::new(String::new());
    static ref E2EE_SAVED_KEY: RwLock<PublicKey> = RwLock::new(PublicKey::from([0u8; 32]));
    static ref CURRENT_RECIPIENT: RwLock<String> = RwLock::new(String::new());
}
pub static LAST_ACK: AtomicBool = AtomicBool::new(false);
pub static REQUEST_ERROR: AtomicBool = AtomicBool::new(false);

pub fn set_server_id(new_id: &str) {
    let mut id = SERVER_ID.write().expect("Lock poisoned");
    *id = new_id.to_string();
}

pub fn get_server_id() -> String {
    SERVER_ID.read().expect("Lock poisoned").clone()
}
pub fn set_current_recipient(new_username: &str) {
    let mut username = CURRENT_RECIPIENT.write().expect("Lock poisoned");
    *username = new_username.to_string();
}

pub fn get_current_recipient() -> String {
    CURRENT_RECIPIENT.read().expect("Lock poisoned").clone()
}

/*
    FCTP message processing with binary encryption
*/
// Header tools
pub fn encapsulate_to_fctp(message: &FctpMessage, session_key: &Key<Aes256Gcm>) -> Result<Vec<u8>> {
    let formatted_message = format!(
        "{}\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n\r\n",
        PROTOCOL_VERSION, message.code as i32, message.from, message.body, message.to
    );

    symmetric::encrypt_binary(&formatted_message, session_key)
        .map_err(|e| FctpError::EncryptionFailed(format!("{:?}", e)))
}

pub fn decapsulate_fctp_message(msg: &[u8], session_key: &Key<Aes256Gcm>) -> Result<FctpMessage> {
    let decrypted_str = if *session_key == Key::<Aes256Gcm>::default() {
        // Unencrypted message!
        String::from_utf8(msg.to_vec())
            .map_err(|e| FctpError::DecryptionFailed(format!("UTF-8 decode error: {}", e)))?
    } else {
        let decrypted_bytes = symmetric::decrypt_binary(msg, session_key)
            .map_err(|e| FctpError::DecryptionFailed(format!("{:?}", e)))?;
        String::from_utf8(decrypted_bytes)
            .map_err(|e| FctpError::DecryptionFailed(format!("UTF-8 decode error: {}", e)))?
    };

    parse_fctp_message(&decrypted_str)
}

fn parse_fctp_message(message: &str) -> Result<FctpMessage> {
    let mut lines = message.lines();

    // Check protocol
    let protocol_line = lines
        .next()
        .ok_or_else(|| FctpError::MalformedMessage("Missing protocol line".to_string()))?;

    if protocol_line != PROTOCOL_VERSION {
        return Err(FctpError::InvalidProtocolVersion(protocol_line.to_string()));
    }

    // parse status code
    let code_str = lines
        .next()
        .ok_or_else(|| FctpError::MalformedMessage("Missing code line".to_string()))?
        .trim();

    let code_int = code_str
        .parse::<i32>()
        .map_err(|_| FctpError::MalformedMessage(format!("Invalid code: {}", code_str)))?;

    let code = FctpCode::from_i32(code_int)
        .ok_or_else(|| FctpError::MalformedMessage(format!("Unknown code: {}", code_int)))?;

    // Parse other attributes
    let from = parse_header_field(lines.next(), "From")?;
    let body = parse_header_field(lines.next(), "Body")?;
    let to = parse_header_field(lines.next(), "To")?;

    Ok(FctpMessage {
        code,
        from,
        body,
        to,
    })
}

fn parse_header_field(line: Option<&str>, field_name: &str) -> Result<String> {
    let line =
        line.ok_or_else(|| FctpError::MalformedMessage(format!("Missing {} line", field_name)))?;

    let prefix = format!("{}: ", field_name);
    line.strip_prefix(&prefix)
        .map(|s| s.trim().to_string())
        .ok_or_else(|| {
            FctpError::MalformedMessage(format!("Invalid {} format: {}", field_name, line))
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
    if let Ok(fctp_message) = decapsulate_fctp_message(message, &get_session_key()) {
        println!(
            "{:?},{},{},{}",
            fctp_message.code, fctp_message.from, fctp_message.body, fctp_message.to
        );
        match fctp_message.code {
            FctpCode::Message => {
                //default c<-c message
                match base64_decode(&fctp_message.body.trim()) {
                    Ok(encrypted_message_bytes) => {
                        match crypt::asymmetric::decrypt(
                            &protocol_utils::fctp_secure::get_e2ee_sec(),
                            &encrypted_message_bytes,
                        ) {
                            Ok(msg) => {
                                let formatted_msg = format!(
                                    "<{}> {}",
                                    fctp_message.from,
                                    String::from_utf8_lossy(&msg).trim()
                                );
                                ui_emit_fctp_message(&app, formatted_msg);
                            }
                            Err(err) => {
                                eprintln!("Couldn't decrypt: {}", err)
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Couldn't decode: {}", err)
                    }
                }
                LAST_ACK.store(true, Ordering::Relaxed);

                // if let Err(e) = app
                //     .notification()
                //     .builder()
                //     .title("FoggyChat")
                //     .body(format!("{}: {}", msg.from, msg.body))
                //     .show()
                // {
                //     eprintln!("Failed to show notification: {:?}", e);
                // }
            }
            FctpCode::Command => {
                //c<-s message
                ui_emit_fctp_message(&app, format!("|SERVER| {}", fctp_message.body));
                LAST_ACK.store(true, Ordering::Relaxed);
            }

            FctpCode::MethodNotAllowed => {
                //Error by client
                ui_emit_fctp_message(&app, format!("[!Client error!] {}", fctp_message.body));
                LAST_ACK.store(true, Ordering::Relaxed);
                REQUEST_ERROR.store(true, Ordering::Relaxed);
            }
            FctpCode::InternalServerError => {
                //Error by server
                ui_emit_fctp_message(&app, format!("[!Server error!] {}", fctp_message.body));
                LAST_ACK.store(true, Ordering::Relaxed);
                REQUEST_ERROR.store(true, Ordering::Relaxed);
            }
            FctpCode::Hello => {
                //9xx - SYSTEM MESSAGE POOL
                if fctp_message.body == "id" {
                    let id_clone = fctp_message.to.clone();
                    let server_id_clone = fctp_message.from.clone();
                    protocol_utils::fctp_me::set_id(&id_clone);
                    set_server_id(&server_id_clone);
                }
            }
            FctpCode::KeyRequest => {
                let recipient_e2ee_key_bytes =
                    base64_decode(&fctp_message.body.trim()).expect("Base64 decode failed");
                let recipient_e2ee_key_array: [u8; 32] = recipient_e2ee_key_bytes
                    .as_slice()
                    .try_into()
                    .expect("Invalid key length");
                let recipient_e2ee_key = PublicKey::from(recipient_e2ee_key_array);
                fctp_secure::E2EE_KEY_TABLE
                    .write()
                    .expect("Lock poisoned")
                    .insert(get_current_recipient(), recipient_e2ee_key);

                LAST_ACK.store(true, Ordering::Relaxed);
            }
            FctpCode::Pong => {
                // Pong handling
                let mut pong_time = last_pong.lock().await;
                *pong_time = Instant::now();
            }
            _ => {
                //Code not matching
                ui_emit_fctp_message(
                    &app,
                    format!(
                        "[!Unsupported code!]: {:?}\nUpdate your client or contact server admin!",
                        fctp_message.code
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
    fn test_fctp_encapsulation_and_decapsulation() {
        let msg = FctpMessage::new(FctpCode::Message, "alice", "hello world", "bob");
        let key = crate::crypt::symmetric::keygen();

        let encrypted = encapsulate_to_fctp(&msg, &key).expect("Should encrypt");
        assert!(!encrypted.is_empty());

        let decrypted = decapsulate_fctp_message(&encrypted, &key).expect("Should decrypt");
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn test_parse_fctp_message() {
        let message_str = format!(
            "{}\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n\r\n",
            PROTOCOL_VERSION, 200, "alice", "hello", "bob"
        );

        let parsed = parse_fctp_message(&message_str).expect("Should parse");
        assert_eq!(parsed.code, FctpCode::Message);
        assert_eq!(parsed.from, "alice");
        assert_eq!(parsed.body, "hello");
        assert_eq!(parsed.to, "bob");
    }

    #[test]
    fn test_parse_fctp_message_direct() {
        let valid_message = "FoggyChat Transfer Protocol 0.1\r\n200\r\nFrom: test\r\nBody: hello world\r\nTo: user\r\n\r\n";
        let parsed = parse_fctp_message(valid_message).expect("Should parse valid message");

        assert_eq!(parsed.code, FctpCode::Message);
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
}
