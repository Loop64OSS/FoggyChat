use aes_gcm::{Aes256Gcm, Key};
use lazy_static::lazy_static;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::{sync::Mutex, time::Instant};

use crate::{
    crypt::{self, symmetric, utils::base64_decode},
    protocol_utils::{
        self,
        fctp_me::get_id,
        fctp_secure::{get_e2ee_sec, get_session_key},
    },
};

/*  The version of the FCTP protocol being used. */
const PROTOCOL_VERSION: &str = "FoggyChat Transfer Protocol 0.1";
/*  The maximum allowed size for a single FCTP message in bytes (1MB). */
const MAX_MESSAGE_SIZE: usize = 1_048_576; /*  1MB limit */

/*  Defines the possible errors that can occur during FCTP message processing. */
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
    #[error("Message too large: {0} bytes (max: {1})")]
    MessageTooLarge(usize, usize),
}

/*  A type alias for `Result` with the FCTP-specific error type. */
type Result<T> = std::result::Result<T, FctpError>;

/*  Represents the various codes used in the FCTP protocol to signify message types. */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FctpCode {
    /*  System message for checking connectivity. */
    Ping = 10,
    /*  System message response to a Ping. */
    Pong = 11,
    /*  A standard, end-to-end encrypted user-to-user message. */
    Message = 200,
    /*  A command sent from the server to the client. */
    Command = 201,
    /*  Error response when the server cannot understand the client's request. */
    BadRequest = 400,
    /*  Error response when a requested method is not allowed. */
    MethodNotAllowed = 405,
    /*  Error response for a critical, unrecoverable server-side error. */
    InternalServerError = 500,
    /*  Error response when a required service or resource is unavailable. */
    ServiceUnavailable = 505,
    /*  System message for initial handshake and identity assignment. */
    Hello = 900,
    /*  System message for exchanging public keys for E2EE. */
    PublicKeyExchange = 901,
    /*  System message for requesting another user's public key. */
    KeyRequest = 902,
}

impl FctpCode {
    /*  Converts an `i32` code into an `FctpCode` enum variant.
     # Arguments
    code - The integer representation of the FCTP code.
     # Returns
    An Option<Self> which is Some(FctpCode) if the code is valid, or None otherwise. */
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

/*  Represents a single FCTP message. */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FctpMessage {
    /*  The type of the message. */
    pub code: FctpCode,
    /*  The ID of the message sender. */
    pub from: String,
    /*  The main content of the message. */
    pub body: String,
    /*  The ID of the message recipient. */
    pub to: String,
}

impl FctpMessage {
    /*  Creates a new `FctpMessage`. */

    /*  # Arguments */
    /*  * `code` - The `FctpCode` for the message. */
    /*  * `from` - The sender's ID. */
    /*  * `body` - The message content. */
    /*  * `to` - The recipient's ID. */
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
    #[allow(dead_code)]
    /*  Creates a new `Ping` message. */
    pub fn ping(to: impl Into<String>) -> Self {
        Self::new(FctpCode::Ping, get_id(), "ping", to)
    }
    #[allow(dead_code)]
    /*  Creates a new `Pong` message. */
    pub fn pong(to: impl Into<String>) -> Self {
        Self::new(FctpCode::Pong, get_id(), "pong", to)
    }
    #[allow(dead_code)]
    /*  Creates a new error message. */
    pub fn error(code: FctpCode, message: impl Into<String>, to: impl Into<String>) -> Self {
        Self::new(code, get_id(), message, to)
    }

    /*  Validates message fields for regular FCTP messages. */
    /*  Note: Some system messages (like handshake) may have empty fields */
    fn validate(&self) -> Result<()> {
        /*  Only validate user-facing messages, not system messages */
        match self.code {
            FctpCode::Message | FctpCode::Command => {
                if self.from.is_empty() {
                    return Err(FctpError::MalformedMessage("Empty 'from' field".into()));
                }
                if self.to.is_empty() {
                    return Err(FctpError::MalformedMessage("Empty 'to' field".into()));
                }
            }
            _ => {
                /*  System messages (Hello, Ping, Pong, etc.) are less strict */
                /*  They may have empty fields during handshake */
            }
        }
        Ok(())
    }
}

/*  --- Global State --- */
/*  `lazy_static` is used to create global, thread-safe variables that are initialized at runtime. */
lazy_static! {
    /*  Stores the ID of the server we are connected to. */
    static ref SERVER_ID: RwLock<String> = RwLock::new(String::new());
    /*  Stores the username of the current chat recipient. */
    static ref CURRENT_RECIPIENT: RwLock<String> = RwLock::new(String::new());
}

/*  A flag to indicate if the last sent message was acknowledged by the recipient. */
pub static LAST_ACK: AtomicBool = AtomicBool::new(false);
/*  A flag to indicate if the last request resulted in an error. */
pub static REQUEST_ERROR: AtomicBool = AtomicBool::new(false);

/*  Sets the global server ID. */
pub fn set_server_id(new_id: &str) -> std::result::Result<(), String> {
    let mut id = SERVER_ID
        .write()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    *id = new_id.to_string();
    Ok(())
}

/*  Retrieves the global server ID. */
pub fn get_server_id() -> std::result::Result<String, String> {
    SERVER_ID
        .read()
        .map(|id| id.clone())
        .map_err(|e| format!("Lock poisoned: {}", e))
}

/*  Sets the current recipient's username. */
pub fn set_current_recipient(new_username: &str) -> std::result::Result<(), String> {
    let mut username = CURRENT_RECIPIENT
        .write()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    *username = new_username.to_string();
    Ok(())
}

/*  Retrieves the current recipient's username. */
pub fn get_current_recipient() -> std::result::Result<String, String> {
    CURRENT_RECIPIENT
        .read()
        .map(|name| name.clone())
        .map_err(|e| format!("Lock poisoned: {}", e))
}

/* Message Processing */

/*  Encapsulates an `FctpMessage` into a byte vector for network transmission. */
/*  This involves formatting the message, validating its size, and encrypting it. */

/*  # Arguments */
/*  * `message` - The `FctpMessage` to encapsulate. */
/*  * `session_key` - The symmetric session key for encryption. */
pub fn encapsulate_to_fctp(message: &FctpMessage, session_key: &Key<Aes256Gcm>) -> Result<Vec<u8>> {
    /*  Validate message */
    message.validate()?;

    let formatted_message = format!(
        "{}\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n\r\n",
        PROTOCOL_VERSION, message.code as i32, message.from, message.body, message.to
    );

    /*  Check message size */
    if formatted_message.len() > MAX_MESSAGE_SIZE {
        return Err(FctpError::MessageTooLarge(
            formatted_message.len(),
            MAX_MESSAGE_SIZE,
        ));
    }

    symmetric::encrypt_binary(&formatted_message, session_key)
        .map_err(|e| FctpError::EncryptionFailed(format!("{:?}", e)))
}

/*  Decapsulates a byte slice received from the network into an `FctpMessage`. */
/*  This involves decrypting the message and parsing its contents. */

/*  Arguments */
/*  msg - The raw byte slice received from the network. */
/*  session_key - The symmetric session key for decryption. */
pub fn decapsulate_fctp_message(msg: &[u8], session_key: &Key<Aes256Gcm>) -> Result<FctpMessage> {
    /*  Validate input size */
    if msg.len() > MAX_MESSAGE_SIZE {
        return Err(FctpError::MessageTooLarge(msg.len(), MAX_MESSAGE_SIZE));
    }

    /*  If the session key is the default, it means we are in the */
    /*  initial handshake phase, and the message is not encrypted. */
    /*  Otherwise, decrypt the message. */
    let decrypted_str = if *session_key == Key::<Aes256Gcm>::default() {
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

/*  Parses a string representation of an FCTP message into an `FctpMessage` struct. */
fn parse_fctp_message(message: &str) -> Result<FctpMessage> {
    let mut lines = message.lines();

    /*  Validate protocol */
    let protocol_line = lines
        .next()
        .ok_or_else(|| FctpError::MalformedMessage("Missing protocol line".to_string()))?;

    if protocol_line != PROTOCOL_VERSION {
        return Err(FctpError::InvalidProtocolVersion(protocol_line.to_string()));
    }

    /*  Parse status code */
    let code_str = lines
        .next()
        .ok_or_else(|| FctpError::MalformedMessage("Missing code line".to_string()))?
        .trim();

    let code_int = code_str
        .parse::<i32>()
        .map_err(|_| FctpError::MalformedMessage(format!("Invalid code: {}", code_str)))?;

    let code = FctpCode::from_i32(code_int)
        .ok_or_else(|| FctpError::MalformedMessage(format!("Unknown code: {}", code_int)))?;

    /*  Parse fields */
    let from = parse_header_field(lines.next(), "From")?;
    let body = parse_header_field(lines.next(), "Body")?;
    let to = parse_header_field(lines.next(), "To")?;

    let message = FctpMessage {
        code,
        from,
        body,
        to,
    };

    message.validate()?;
    Ok(message)
}

/*  A helper function to parse a single header field (ex. From:) from a line. */
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

/*  Emits a message to the Tauri frontend. */
/*  This is the primary way the backend communicates information to be displayed in the UI. */
pub fn ui_emit_fctp_message(app: &AppHandle, msg: String) {
    if let Err(e) = app.emit("fctp-message", msg) {
        eprintln!("Failed to emit message: {:?}", e);
    }
}

/*  The main entry point for processing incoming FCTP messages from the network stream. */
/*  It decapsulates the message and dispatches it to the appropriate handler based on its code. */
pub async fn process_fctp_stream(
    app: tauri::AppHandle,
    message: &[u8],
    last_pong: &Arc<Mutex<Instant>>,
) {
    /*  Validate message size */
    if message.len() > MAX_MESSAGE_SIZE {
        ui_emit_fctp_message(
            &app,
            format!("[!Error!] Message too large: {} bytes", message.len()),
        );
        return;
    }

    let fctp_message = match decapsulate_fctp_message(message, &get_session_key()) {
        Ok(msg) => msg,
        Err(e) => {
            let msg_preview = String::from_utf8_lossy(message);
            let preview = if msg_preview.len() > 100 {
                format!("{}...", &msg_preview[..100])
            } else {
                msg_preview.to_string()
            };
            ui_emit_fctp_message(&app, format!("[!Malformed message!]: {} - {}", preview, e));
            return;
        }
    };

    println!(
        "{:?},{},{},{}",
        fctp_message.code, fctp_message.from, fctp_message.body, fctp_message.to
    );

    match fctp_message.code {
        /*  Message: Standard user to user message, end to end encrypted, code 200. */
        FctpCode::Message => handle_message(&app, fctp_message).await,
        /*  Command: User to server message (server command), code 201. */
        FctpCode::Command => handle_command(&app, fctp_message).await,
        /*  MethodNotAllowed: User error, sent only to user from server, code 405. */
        FctpCode::MethodNotAllowed => handle_error(&app, fctp_message, "Client error").await,
        /*  InternalServerError: Server error, sent to user from server when critical error occurs (ex. database connection error), code 500. */
        FctpCode::InternalServerError => handle_error(&app, fctp_message, "Server error").await,
        /*  BadRequest: When user sends FCTP Message with unknown code, the server returns this error, code 400. */
        FctpCode::BadRequest => handle_error(&app, fctp_message, "Bad Request").await,
        /*  ServiceUnavailable: Server error, sent to user from server, occurs when server cant access some resource (ex. critical microservice unavailability), code 505. */
        FctpCode::ServiceUnavailable => {
            handle_error(&app, fctp_message, "Service Unavailable").await
        }
        /*  Hello: System message, is sent when server wants to assign something critical to user (ex. id). */
        FctpCode::Hello => handle_hello(&app, fctp_message).await,
        /*  KeyRequest: System message, is sent when client wants to acquire another clients end to end encryption key. */
        FctpCode::KeyRequest => {
            protocol_utils::fctp_secure::handle_e2ee_key_request(&app, fctp_message).await
        }
        /*  Pong: System message, pong */
        FctpCode::Pong => handle_pong(last_pong).await,
        _ => {
            ui_emit_fctp_message(
                &app,
                format!(
                    "[!Unsupported code!]: {:?}\nUpdate your client or contact server admin!",
                    fctp_message.code
                ),
            );
        }
    }
}

/*  Handles a standard user-to-user message (FctpCode::Message). */
/*  It decodes the Base64 body, decrypts it using the E2EE private key, */
/*  and emits the formatted message to the UI. */
async fn handle_message(app: &AppHandle, msg: FctpMessage) {
    let encrypted_bytes = match base64_decode(&msg.body.trim()) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("Base64 decode error: {}", err);
            return;
        }
    };

    let decrypted = match crypt::asymmetric::decrypt(&get_e2ee_sec(), &encrypted_bytes) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Decryption error: {}", err);
            return;
        }
    };

    let formatted_msg = format!(
        "<{}> {}",
        msg.from,
        String::from_utf8_lossy(&decrypted).trim()
    );
    ui_emit_fctp_message(app, formatted_msg);
    LAST_ACK.store(true, Ordering::Relaxed);
}

/*  Handles a command from the server (FctpCode::Command).
It displays the command body directly in the UI, prefixed with `|SERVER|`. */
async fn handle_command(app: &AppHandle, msg: FctpMessage) {
    ui_emit_fctp_message(app, format!("|SERVER| {}", msg.body));
    LAST_ACK.store(true, Ordering::Relaxed);
}

/*  Handles various error messages from the server.
It displays the error in the UI and sets a global flag indicating a request error. */
async fn handle_error(app: &AppHandle, msg: FctpMessage, error_type: &str) {
    ui_emit_fctp_message(app, format!("[!{}!] {}", error_type, msg.body));
    LAST_ACK.store(true, Ordering::Relaxed);
    REQUEST_ERROR.store(true, Ordering::Relaxed);
}

/*  Handles the initial `Hello` message from the server.
This is used to set the client's own ID and the server's ID. */
async fn handle_hello(_app: &AppHandle, msg: FctpMessage) {
    if msg.body == "id" {
        protocol_utils::fctp_me::set_id(&msg.to);
        if let Err(e) = set_server_id(&msg.from) {
            eprintln!("Failed to set server ID: {}", e);
        }
    }
}

/*  Handles a `Pong` message from the server.
It updates the timestamp of the last received pong, which is used for keep-alive checks. */
async fn handle_pong(last_pong: &Arc<Mutex<Instant>>) {
    let mut pong_time = last_pong.lock().await;
    *pong_time = Instant::now();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fctp_encapsulation_and_decapsulation() {
        let msg = FctpMessage::new(FctpCode::Message, "alice", "hello world", "bob");
        let key = crate::crypt::symmetric::keygen();

        let encrypted = encapsulate_to_fctp(&msg, &key).expect("Should encrypt");
        assert!(!encrypted.is_empty());
        assert!(encrypted.len() < MAX_MESSAGE_SIZE);

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
    fn test_message_validation() {
        let invalid_msg = FctpMessage::new(FctpCode::Message, "", "body", "to");
        assert!(invalid_msg.validate().is_err());

        let valid_msg = FctpMessage::new(FctpCode::Message, "from", "body", "to");
        assert!(valid_msg.validate().is_ok());
    }

    #[test]
    fn test_message_size_limit() {
        let large_body = "x".repeat(MAX_MESSAGE_SIZE + 1);
        let msg = FctpMessage::new(FctpCode::Message, "alice", large_body, "bob");
        let key = crate::crypt::symmetric::keygen();

        let result = encapsulate_to_fctp(&msg, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_server_id_management() {
        set_server_id("test_server_123").expect("Should set ID");
        assert_eq!(get_server_id().unwrap(), "test_server_123");

        set_server_id("another_server").expect("Should update ID");
        assert_eq!(get_server_id().unwrap(), "another_server");
    }
}
