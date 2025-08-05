use aes_gcm::{Aes256Gcm, Key};
use lazy_static::lazy_static;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::{sync::Mutex, time::Instant};

use crate::{
    crypt::symmetric,
    protocol_utils::{self, fctp_secure::get_session_key},
};

pub struct FctpMessage {
    pub code: i32,
    pub from: String,
    pub body: String,
    #[allow(dead_code)] //pieprzony rust analyzer \/
    pub to: String,
}

/*
    Set global user ID using OnceLock
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
    FCTP message processing
*/
pub fn encapsulate_to_fctp(
    code: i32,
    from: &str,
    body: &str,
    to: &str,
    session_key: Key<Aes256Gcm>,
) -> String {
    match symmetric::encrypt(
        &format!(
            "FoggyChat Transfer Protocol 0.1\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n",
            code, from, body, to
        ),
        &session_key,
    ) {
        Ok(encrypted) => format!("{}", encrypted),
        Err(_) => {
            println!("Failed to encrypt FCTP message");
            String::new()
        }
    }
}

pub fn decapsulate_fctp_message(msg: &str, session_key: Key<Aes256Gcm>) -> Option<FctpMessage> {
    match symmetric::decrypt(&msg.trim(), &session_key) {
        Ok(decrypted) => {
            let mut lines = decrypted.lines();

            if lines.next()? != "FoggyChat Transfer Protocol 0.1" {
                return None;
            }
            //TODO: base64 encoding
            let code = lines.next()?.trim().parse::<i32>().ok()?; // ABSOLUTELY REQUIRED, MUST BE A NUMBER IN INT FORMAT 32 BIT SIZE
            let from = lines.next()?.strip_prefix("From: ")?.trim().to_string();
            let body = lines.next()?.strip_prefix("Body: ")?.trim().to_string();
            let to = lines.next()?.strip_prefix("To: ")?.trim().to_string(); // To: is optional, but we keep it for consistency (may be used to remind the client about its id). TL/DR: ignored

            Some(FctpMessage {
                code,
                from,
                body,
                to,
            })
        }
        Err(_) => {
            println!("Failed to decrypt FCTP message: {}", msg);
            None
        }
    }
}
pub fn pass_message(app: AppHandle, msg: String) {
    app.emit("pass-message", msg).unwrap();
}
pub async fn process_fctp_stream(
    app: tauri::AppHandle,
    message: String,
    last_pong: &Arc<Mutex<Instant>>,
) {
    if let Some(msg) = decapsulate_fctp_message(&message, get_session_key()) {
        //pool 4xx - client-side errors
        //pool 5xx - server-side errors
        //pool 2xx - message handling
        //pool 1x - //TODO: connection handling (ping/pong)
        //pool 8xx - //TODO: encryption handshake
        //pool 9xx - //TODO: session information ex. logged in, id, etc.
        match msg.code {
            200 => pass_message(app , format!("[Message received] <{}> {}", msg.from, msg.body)), //User-user direct message
            201 => pass_message(app , format!("SERVER: {}", msg.body)), //From-server general direct message
            405 => pass_message(app , format!("[!cl!] {}", msg.body)),  //Client-side error
            505 => pass_message(app , format!("[!sv!] {}", msg.body)),  //Server-side error
            900 => {
                if msg.body == "id" {
                    let id_clone = msg.to.clone();
                    let server_id_clone = msg.from.clone();
                    protocol_utils::fctp_me::set_id(&id_clone);
                    set_server_id(&server_id_clone);
                }
                std::thread::sleep(Duration::from_secs(1));
                pass_message(app , format!("DEBUG: ID: {}", protocol_utils::fctp_me::get_id()));
            }
            11 => {
                let mut pong_time = last_pong.lock().await;
                *pong_time = Instant::now();
            } //Pong handling (ping code 10, pong code 11)
            _ => pass_message(app , format!(
                "[!Unsupported code!]: {}\n !Update your client software or ask server administrator to update his server software!",
                msg.code
            )), //Unknown code handling
        }
    } else {
        pass_message(app, format!("[!Malformed header!]: {}", message));
    }
}
