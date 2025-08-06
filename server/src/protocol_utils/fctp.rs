use aes_gcm::{Aes256Gcm, Key};
use tokio::io::AsyncWriteExt;

use crate::{
    crypt::{self, symmetric},
    get_id,
    protocol_utils::fctp_client,
};
pub struct FctpMessage {
    pub code: i32,
    pub from: String,
    pub body: String,
    #[allow(dead_code)] //pieprzony rust analyzer \/
    pub to: String,
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
            "FoggyChat Transfer Protocol 0.1\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}",
            code, from, body, to
        ),
        &session_key,
    ) {
        Ok(encrypted) => format!("{}\r\n\r\n", encrypted),
        Err(_) => {
            println!("Failed to encrypt FCTP message");
            String::new()
        }
    }
}

pub fn decapsulate_fctp_message(msg: &str, session_key: Key<Aes256Gcm>) -> Option<FctpMessage> {
    match symmetric::decrypt(&msg, &session_key) {
        Ok(decrypted) => {
            println!("Decrypted message: {}", decrypted);

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
            println!("Failed to decrypt FCTP message");
            None
        }
    }
}
async fn is_nick_taken(clients: &fctp_client::Clients, nick: &str, current_id: &str) -> bool {
    let map = clients.lock().await;
    map.iter()
        .any(|(id, client)| id != current_id && client.ext_session_username == nick)
}

pub async fn command_handler(
    fctp_message: FctpMessage,
    id_clone: &mut String,
    client_info: &mut fctp_client::ClientInfo,
    clients: &fctp_client::Clients,
) {
    let body = fctp_message.body.trim();
    let mut parts = body.split_whitespace();
    let command = parts.next().unwrap_or(""); // Pierwszy element to komenda

    match command {
        "help" => {
            let msg = format!(
                "Welcome to server! ServerID: {} / Available commands: help, whoami, setnick <name>",
                get_id()
            );
            send_fctp_message(client_info, 201, get_id(), &msg, id_clone).await;
        }

        "whoami" => {
            let msg = format!(
                "You are connected as: {} / Your session nick: {} / ServerID: {} / Connected for: {} seconds",
                id_clone,
                client_info.ext_session_username,
                get_id(),
                client_info.connected_at.elapsed().as_secs(),
            );
            send_fctp_message(client_info, 201, get_id(), &msg, id_clone).await;
        }

        "setnick" => {
            if let Some(new_name) = parts.next() {
                if is_nick_taken(clients, new_name, id_clone).await {
                    let msg = format!(
                        "Nick '{}' is already taken. Choose a different one.",
                        new_name
                    );
                    send_fctp_message(client_info, 405, get_id(), &msg, id_clone).await; // 409 Conflict
                } else {
                    if new_name.len() < 3 || new_name.len() > 20 {
                        let msg = "Nick must be between 3 and 20 characters long.".to_string();
                        send_fctp_message(client_info, 405, get_id(), &msg, id_clone).await;
                    } else if !new_name
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                    {
                        let msg =
                            "Nick can only contain letters, numbers, underscores and hyphens."
                                .to_string();
                        send_fctp_message(client_info, 405, get_id(), &msg, id_clone).await;
                    } else {
                        client_info.ext_session_username = new_name.to_owned();
                        let msg = format!(
                            "Nick changed to: {} / ServerID: {}",
                            client_info.ext_session_username,
                            get_id()
                        );
                        send_fctp_message(client_info, 201, get_id(), &msg, id_clone).await;
                    }
                }
            } else {
                let msg = "Usage: setnick <new_name>".to_string();
                send_fctp_message(client_info, 400, get_id(), &msg, id_clone).await;
            }
        }

        _ => {
            let msg = format!(
                "Unknown command: {}. Type 'help' for available commands.",
                body
            );
            send_fctp_message(client_info, 405, get_id(), &msg, id_clone).await;
        }
    }
}
pub async fn send_fctp_message(
    client_info: &mut fctp_client::ClientInfo,
    code: i32,
    from: &str,
    body: &str,
    to: &str,
) {
    let mut writer = client_info.socket.lock().await;

    let _ = writer
        .write_all(
            encapsulate_to_fctp(code, from, body, to, client_info.conn_session_key).as_bytes(),
        )
        .await;
}
