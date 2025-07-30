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

pub async fn command_handler(
    fctp_message: FctpMessage,
    id_clone: String,
    client_info: &mut fctp_client::ClientInfo,
) {
    if fctp_message.body.trim() == "help" {
        let msg = format!(
            "Welcome to server! ServerID: {} / Available commands: /help",
            get_id()
        );
        send_fctp_message(client_info, 201, get_id(), &msg, &id_clone).await;
    } else if fctp_message.body.trim() == "whoami" {
        let msg = format!(
            "You are connected as: {} / ServerID: {} / Connected for: {} seconds",
            id_clone,
            get_id(),
            client_info.connected_at.elapsed().as_secs(),
        );
        send_fctp_message(client_info, 201, get_id(), &msg, &id_clone).await;
    } else {
        let msg = format!(
            "Welcome to server! ServerID: {} / Unknown command: {}",
            get_id(),
            fctp_message.body.trim()
        );
        send_fctp_message(client_info, 201, get_id(), &msg, &id_clone).await;
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
