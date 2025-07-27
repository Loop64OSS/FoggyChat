#![allow(unused_imports)] //ZAMKNIĘCIE RYJA RUST ANALYZER
mod crypt;
mod protocol_utils;
use crypt::asymmetric;
use crypt::symmetric;
use std::collections::HashMap;
use std::process::exit;
use std::sync::{Arc, OnceLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::protocol_utils::fctp;

struct FctpMessage {
    code: i32,
    from: String,
    body: String,
    #[allow(dead_code)] //pieprzony rust analyzer \/
    to: String,
}

/*
    Set global Server ID using OnceLock (USE ONLY WITH SERVER ID FILE IN PRODUCTION!!!!)
*/

static ID: OnceLock<String> = OnceLock::new();

pub fn set_id(new_id: &str) {
    ID.set(new_id.to_owned()).unwrap_or_else(|_| {
        eprintln!("Unauthorized action: Cannot reregister server!");
    })
}

pub fn get_id() -> &'static str {
    ID.get().map(|s| s.as_str()).unwrap_or_else(|| {
        eprintln!("Unauthorized action: Unregistered server! Halting execution.");
        exit(1005);
    })
}
/**/

fn generate_id() -> uuid::Uuid {
    let uuid = Uuid::new_v4();
    uuid
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    let clients: protocol_utils::fctp_client::Clients = Arc::new(Mutex::new(HashMap::new()));
    set_id("b0ba4f3e-3d19-4f5f-bae2-ece5f6464702"); // SERVER ID WILL NOT BE HARD-CODED IN PRODUCTION, IT WILL BE LOADED FROM FILE!!!
    loop {
        let (socket, _) = listener.accept().await?;
        let clients = clients.clone();
        let id = generate_id();
        let id_string = id.to_string();

        // Split the socket into reader and writer
        let (mut reader, writer) = socket.into_split();

        let client_info = protocol_utils::fctp_client::ClientInfo {
            socket: Arc::new(Mutex::new(writer)),
            connected_at: std::time::Instant::now(),
        };

        {
            let mut map = clients.lock().await;
            map.insert(id_string.clone(), client_info);
        }

        let id_clone = id_string.clone();

        tokio::spawn(async move {
            {
                let _ = protocol_utils::fctp_operations::send_broadcast(
                    &clients,
                    &id_clone,
                    &format!("{} joined", &id_clone),
                )
                .await;
                let mut map = clients.lock().await;
                if let Some(client_info) = map.get_mut(&id_clone) {
                    let mut writer = client_info.socket.lock().await;
                    let _ = writer
                        .write_all(
                            fctp::encapsulate_to_fctp(900, get_id(), "id", &id_clone).as_bytes(),
                        )
                        .await;

                    let _ = writer
                        .write_all(
                            fctp::encapsulate_to_fctp(
                                201,
                                get_id(),
                                &format!(
                                    "motd=Welcome to Loop64.com FoggyChat server. id={}",
                                    &id_clone
                                ),
                                &id_clone,
                            )
                            .as_bytes(),
                        )
                        .await;
                }
            }

            let mut buf = [0; 1024];
            loop {
                let n = match reader.read(&mut buf).await {
                    Ok(n) if n == 0 => break,
                    Ok(n) => n,
                    Err(_) => break,
                };

                // Humanize the message
                if let Ok(msg) = std::str::from_utf8(&buf[..n]) {
                    // TEMPORARY DEBUG
                    println!("[D]\n{}", msg);
                    if let Some(fctp_message) = fctp::decapsulate_fctp_message(msg) {
                        if fctp_message.code == 200 {
                            let to_id = fctp_message.to.trim();
                            print!(
                                "[Message received] <{}> {}\n",
                                fctp_message.from, fctp_message.body
                            );
                            let mut map = clients.lock().await;
                            if let Some(client_info) = map.get_mut(to_id) {
                                fctp::send_fctp_message(
                                    client_info,
                                    200,
                                    &id_clone,
                                    &fctp_message.body.trim(),
                                    &to_id,
                                )
                                .await;
                            } else {
                                // Recipient not found, send error header to sender
                                if let Some(client_info) = map.get_mut(&id_clone) {
                                    fctp::send_fctp_message(
                                        client_info,
                                        405,
                                        get_id(),
                                        "Not found",
                                        &id_clone,
                                    )
                                    .await;
                                }
                            }
                        } else if fctp_message.code == 10 {
                            // Ping message, send pong back
                            let mut map = clients.lock().await;
                            if let Some(client_info) = map.get_mut(&id_clone) {
                                fctp::send_fctp_message(
                                    client_info,
                                    11,
                                    get_id(),
                                    "pong",
                                    &id_clone,
                                )
                                .await;
                            }
                            continue;
                        } else if fctp_message.code == 201 {
                            // Commands
                            let mut map = clients.lock().await;
                            if let Some(client_info) = map.get_mut(&id_clone) {
                                fctp::command_handler(fctp_message, id_clone.clone(), client_info)
                                    .await;
                            }
                            continue;
                        } else {
                            let mut map = clients.lock().await;
                            if let Some(client_info) = map.get_mut(&id_clone) {
                                fctp::send_fctp_message(
                                    client_info,
                                    405,
                                    get_id(),
                                    "Unsupported header code, error!",
                                    &id_clone,
                                )
                                .await;
                            }
                        }
                    } else {
                        // Bad format, send error header to sender
                        let mut map = clients.lock().await;
                        if let Some(client_info) = map.get_mut(&id_clone) {
                            fctp::send_fctp_message(
                                client_info,
                                505,
                                get_id(),
                                "Malformed header, error!",
                                &id_clone,
                            )
                            .await;
                        }
                    }
                }
            }
            let mut map = clients.lock().await;
            map.remove(&id_clone);
        });
    }
}

/*
#########################
UNIT TESTS
#########################
*/
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn symmetric() {
        let key = symmetric::keygen();

        match symmetric::encrypt("test", &key) {
            Ok(encrypted) => {
                println!("Encrypted: {}", encrypted);

                match symmetric::decrypt(&encrypted, &key) {
                    Ok(decrypted) => println!("Decrypted: {}", decrypted),
                    Err(e) => eprintln!("Decryption error: {}", e),
                }
            }
            Err(e) => eprintln!("Encryption error: {}", e),
        }
    }
    #[test]
    fn asymmetric() {
        let (rec_sec_bytes, rec_pub_bytes) = asymmetric::keypairgen();
        let rec_sec = StaticSecret::from(rec_sec_bytes);
        let rec_pub = PublicKey::from(rec_pub_bytes);
        match asymmetric::encrypt(&rec_pub, "test".as_bytes()) {
            Ok(encrypted) => match asymmetric::decrypt(&rec_sec, &encrypted) {
                Ok(decrypted) => println!("Decrypted: {}", String::from_utf8_lossy(&decrypted)),
                Err(e) => eprintln!("Decryption error: {}", e),
            },
            Err(e) => eprintln!("Encryption error: {}", e),
        }
    }

    #[test]
    fn test_generate_id() {
        let id = generate_id();
        assert!(!id.is_nil(), "Generated ID should not be nil");
        println!("Generated ID: {}", id);
    }
    #[test]
    fn test_fctp_encapsulation_and_decapsulation() {
        let code = 200;
        let from = "user123";
        let body = "Hello, world!";
        let to = "user456";

        let message = fctp::encapsulate_to_fctp(code, from, body, to);
        let decoded =
            fctp::decapsulate_fctp_message(&message).expect("Failed to parse FCTP message");

        assert_eq!(decoded.code, code);
        assert_eq!(decoded.from, from);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.to, to);
    }
    #[test]
    fn test_fctp_decapsulation_invalid() {
        let bad_message = "This is not a valid FCTP message";
        assert!(fctp::decapsulate_fctp_message(bad_message).is_none());

        let bad_code =
            "FoggyChat Transfer Protocol 0.1\r\nabc\r\nFrom: a\r\nBody: b\r\nTo: c\r\n\r\n";
        assert!(fctp::decapsulate_fctp_message(bad_code).is_none());

        let missing_lines =
            "FoggyChat Transfer Protocol 0.1\r\n200\r\nFrom: user\r\nBody: test\r\n\r\n";
        assert!(fctp::decapsulate_fctp_message(missing_lines).is_none());
    }
    #[test]
    fn test_set_and_get_id() {
        let _ = ID.set("test-server-id".to_string());
        assert_eq!(get_id(), "test-server-id");

        let result = ID.set("should-fail".to_string());
        assert!(result.is_err(), "ID should not be set twice");
    }
}
