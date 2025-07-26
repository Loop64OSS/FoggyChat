#![allow(unused_imports)] //ZAMKNIĘCIE RYJA RUST ANALYZER
mod crypt;
use crypt::asymmetric;
use crypt::symmetric;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

type Clients = Arc<Mutex<HashMap<String, tokio::net::tcp::OwnedWriteHalf>>>;

struct FctpMessage {
    code: i32,
    from: String,
    body: String,
    #[allow(dead_code)] //pieprzony rust analyzer \/
    to: String,
}

fn generate_id() -> uuid::Uuid {
    let uuid = Uuid::new_v4();
    uuid
}

/*
    Set global Server ID using OnceLock (USE ONLY WITH SERVER ID FILE IN PRODUCTION!!!!)
*/

static ID: OnceLock<String> = OnceLock::new();

pub fn set_id(new_id: &str) {
    ID.set(new_id.to_owned()).expect("ID already set!");
}

pub fn get_id() -> &'static str {
    ID.get().map(|s| s.as_str()).expect("ID not set yet!")
}
/**/
/*
    FCTP message processing
*/
fn encapsulate_to_fctp(code: i32, from: &str, body: &str, to: &str) -> String {
    format!(
        "FoggyChat Transfer Protocol 0.1\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n\r\n",
        code, from, body, to
    )
}

fn decapsulate_fctp_message(msg: &str) -> Option<FctpMessage> {
    let mut lines = msg.lines();

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

async fn send_broadcast(
    clients: &Clients,
    id: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut map = clients.lock().await;
    for (client_id, writer) in map.iter_mut() {
        if client_id != id {
            let _ = writer
                .write_all(encapsulate_to_fctp(201, get_id(), message, client_id).as_bytes())
                .await;
        }
    }
    Ok(())
}
async fn kick_client(clients: &Clients, id: &str) {
    let mut map = clients.lock().await;
    if let Some(writer) = map.remove(id) {
        drop(writer);
        println!("User {} got kicked", id);
    } else {
        println!("User {} doesn't exist.", id);
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    set_id("b0ba4f3e-3d19-4f5f-bae2-ece5f6464702"); // SERVER ID WILL NOT BE HARD-CODED IN PRODUCTION, IT WILL BE LOADED FROM FILE!!!
    loop {
        let (socket, _) = listener.accept().await?;
        let clients = clients.clone();
        let id = generate_id();
        let id_string = id.to_string();
        let (mut reader, writer) = socket.into_split();
        {
            let mut map = clients.lock().await;
            map.insert(id_string.clone(), writer);
        }

        let id_clone = id_string.clone();

        tokio::spawn(async move {
            {
                let _ = send_broadcast(&clients, &id_clone, &format!("{} joined", &id_clone)).await;
                let mut map = clients.lock().await;
                if let Some(writer) = map.get_mut(&id_clone) {
                    let _ = writer
                        .write_all(encapsulate_to_fctp(900, get_id(), "id", &id_clone).as_bytes())
                        .await;
                    let _ = writer
                        .write_all(
                            encapsulate_to_fctp(
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
                    if let Some(fctp_message) = decapsulate_fctp_message(msg) {
                        if fctp_message.code == 200 {
                            let to_id = fctp_message.to.trim();
                            print!(
                                "[Message received] <{}> {}\n",
                                fctp_message.from, fctp_message.body
                            );
                            let mut map = clients.lock().await;
                            if let Some(writer) = map.get_mut(to_id) {
                                let _ = writer
                                    .write_all(
                                        encapsulate_to_fctp(
                                            200,
                                            &id_clone,
                                            &fctp_message.body.trim(),
                                            &to_id,
                                        )
                                        .as_bytes(),
                                    )
                                    .await;
                            } else {
                                // Recipient not found, send error header to sender
                                if let Some(sender_writer) = map.get_mut(&id_clone) {
                                    let _ = sender_writer
                                        .write_all(
                                            encapsulate_to_fctp(
                                                405,
                                                get_id(),
                                                "Not found",
                                                &id_clone,
                                            )
                                            .as_bytes(),
                                        )
                                        .await;
                                }
                            }
                        } else if fctp_message.code == 10 {
                            // Ping message, send pong back
                            let mut map = clients.lock().await;
                            if let Some(writer) = map.get_mut(&id_clone) {
                                let _ = writer
                                    .write_all(
                                        encapsulate_to_fctp(11, get_id(), "pong", &id_clone)
                                            .as_bytes(),
                                    )
                                    .await;
                            }
                            continue;
                        } else if fctp_message.code == 201 {
                            // Commands
                            let mut map = clients.lock().await;
                            if let Some(writer) = map.get_mut(&id_clone) {
                                if fctp_message.body.trim() == "help" {
                                    let msg = format!(
                                        "Welcome to server! ServerID: {} / Available commands: /help",
                                        get_id()
                                    );
                                    let _ = writer
                                        .write_all(
                                            encapsulate_to_fctp(201, get_id(), &msg, &id_clone)
                                                .as_bytes(),
                                        )
                                        .await;
                                } else {
                                    let msg = format!(
                                        "Welcome to server! ServerID: {} / Unknown command: {}",
                                        get_id(),
                                        fctp_message.body.trim()
                                    );
                                    let _ = writer
                                        .write_all(
                                            encapsulate_to_fctp(201, get_id(), &msg, &id_clone)
                                                .as_bytes(),
                                        )
                                        .await;
                                }
                            }
                            continue;
                        } else {
                            let mut map = clients.lock().await;
                            if let Some(sender_writer) = map.get_mut(&id_clone) {
                                let _ = sender_writer
                                    .write_all(
                                        encapsulate_to_fctp(
                                            505,
                                            get_id(),
                                            "Unsupported header code, error!",
                                            &id_clone,
                                        )
                                        .as_bytes(),
                                    )
                                    .await;
                            }
                        }
                    } else {
                        // Bad format, send error header to sender
                        let mut map = clients.lock().await;
                        if let Some(sender_writer) = map.get_mut(&id_clone) {
                            let _ = sender_writer
                                .write_all(
                                    encapsulate_to_fctp(
                                        505,
                                        get_id(),
                                        "Malformed header, error!",
                                        &id_clone,
                                    )
                                    .as_bytes(),
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
}
