#![allow(unused_imports)] //ZAMKNIĘCIE RYJA RUST ANALYZER
use rand::{Rng, distributions::Alphanumeric};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

type Clients = Arc<Mutex<HashMap<String, tokio::net::tcp::OwnedWriteHalf>>>;

struct FctpMessage {
    code: i32,
    from: String,
    body: String,
    #[allow(dead_code)] //pieprzony rust analyzer \/
    to: String,
}

fn generate_id() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

fn encapsulate_to_fctp(code: i32, from: &str, body: &str, to: &str) -> String {
    format!(
        "FoggyChat Transfer Protocol 0.1\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n\r\n",
        code, from, body, to
    )
}

fn decapsulate_fctp_message(msg: &str) -> Option<FctpMessage> {
    let mut lines = msg.lines();

    if lines.next()? != "FoggyChat Transfer Protocol 0.1" {
        println!(
            "Protocol header does not match expected format. Update your client or contact the administrator of the server."
        );
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
                .write_all(encapsulate_to_fctp(201, "server", message, client_id).as_bytes())
                .await;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, _) = listener.accept().await?;
        let clients = clients.clone();
        let id = generate_id();
        let (mut reader, writer) = socket.into_split();
        {
            let mut map = clients.lock().await;
            map.insert(id.clone(), writer);
        }

        let id_clone = id.clone();

        tokio::spawn(async move {
            {
                let _ = send_broadcast(&clients, &id_clone, &format!("{} joined", &id_clone)).await;
                let mut map = clients.lock().await;
                if let Some(writer) = map.get_mut(&id_clone) {
                    let _ = writer
                        .write_all(
                            encapsulate_to_fctp(
                                201,
                                "server",
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
                                                "server",
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
                                        encapsulate_to_fctp(11, "server", "pong", &id_clone)
                                            .as_bytes(),
                                    )
                                    .await;
                            }
                            continue;
                        } else {
                            let mut map = clients.lock().await;
                            if let Some(sender_writer) = map.get_mut(&id_clone) {
                                let _ = sender_writer
                                    .write_all(
                                        encapsulate_to_fctp(
                                            505,
                                            "server",
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
                                        "server",
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
