#![allow(unused_imports)] //UTKANIE JEBANEGO RUST ANALYZERA

mod crypt;
mod protocol_utils;
use aes_gcm::Aes256Gcm;
use aes_gcm::Key;
use aes_gcm::KeyInit;
use crypt::asymmetric;
use crypt::symmetric;
use lazy_static::lazy_static;
use protocol_utils::fctp;
use protocol_utils::fctp_me;
use regex::Regex;
use sha2::digest::generic_array::GenericArray;
use std::process::exit;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::time::Instant;
use x25519_dalek::PublicKey;
use x25519_dalek::StaticSecret;

use crate::protocol_utils::fctp_secure::get_exchange_pub;
use crate::protocol_utils::fctp_secure::get_exchange_sec;
use crate::protocol_utils::fctp_secure::get_session_key;
use crate::protocol_utils::fctp_secure::set_exchange;
use crate::protocol_utils::fctp_secure::set_session_key;
use once_cell::sync::OnceCell;
static TX: OnceCell<tokio::sync::Mutex<mpsc::Sender<Vec<u8>>>> = OnceCell::new();
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

//
//User input parser
fn parse_input(input: &str) -> Result<(&str, &str), &'static str> {
    let re = Regex::new(r"^([^:]+):([^:]+)$").unwrap();
    if let Some(caps) = re.captures(input) {
        let left = caps.get(1).unwrap().as_str();
        let right = caps.get(2).unwrap().as_str();
        Ok((left, right))
    } else {
        Err("Wrong input format, expected 'key:value'")
    }
}
#[tauri::command]
async fn greet(line: &str) -> Result<String, String> {
    match parse_input(&line) {
        Ok((id, msg)) => {
            let packet = fctp::encapsulate_to_fctp(
                200,
                &fctp_me::get_id(),
                msg.trim(),
                id.trim(),
                get_session_key(),
            );
            if let Some(tx) = TX.get() {
                tx.lock()
                    .await
                    .send(packet.into_bytes())
                    .await
                    .map_err(|e| format!("Failed to send packet: {}", e))?;
            } else {
                return Err("Channel sender not initialized".into());
            }
            Ok("ok".into())
        }
        Err(e) => Err(format!("Parse error: {}", e)),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            tauri::async_runtime::spawn(prot());
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn prot() {
    /*
       Main code Logic
    */

    let stream = TcpStream::connect("127.0.0.1:8081").await.unwrap(); // start stream //TODO: SOCKS5stream support and address selection in user input
    let (reader, writer) = stream.into_split();
    let reader = BufReader::new(reader);
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);
    let last_pong = Arc::new(tokio::sync::Mutex::new(Instant::now()));
    let last_pong_clone = last_pong.clone();
    TX.set(Mutex::new(tx.clone())).unwrap();
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            //receive messages from server and process them
            let mut lines = reader.lines();
            let mut message = String::new();
            // Glue the lines together
            while let Ok(Some(line)) = lines.next_line().await {
                if !message.ends_with("\r\n\r\n") {
                    while message.ends_with('\n') || message.ends_with('\r') {
                        message.pop();
                    }
                    message.push_str("\r\n\r\n");
                }
                if line.trim().is_empty() && !message.is_empty() {
                    if get_session_key() == Key::<Aes256Gcm>::default() {
                        if get_exchange_pub() == PublicKey::from([0u8; 32]) {
                            match crypt::utils::base64_decode(&message.trim()) {
                                Ok(decoded) => {
                                    if let Ok(decoded_bytes) =
                                        TryInto::<[u8; 32]>::try_into(decoded)
                                    {
                                        //TODO: implement tofu verification here
                                        println!(
                                            "Received public key: BLAKE3:{}",
                                            crypt::utils::blake3_hash(&decoded_bytes)
                                        );
                                        let cert_pub = PublicKey::from(decoded_bytes);
                                        let (rec_sec_bytes, rec_pub_bytes) =
                                            asymmetric::keypairgen();
                                        let rec_sec = StaticSecret::from(rec_sec_bytes);
                                        let rec_pub = PublicKey::from(rec_pub_bytes);
                                        set_exchange(rec_pub, rec_sec);
                                        match asymmetric::encrypt(&cert_pub, rec_pub.as_bytes()) {
                                            Ok(encrypted) => {
                                                let _ = tx
                                                    .send(
                                                        crypt::utils::base64_encode(&encrypted)
                                                            .into(),
                                                    )
                                                    .await;
                                            }
                                            Err(e) => eprintln!("Encryption error: {}", e),
                                        }
                                    } else {
                                        eprintln!("Error: Key must be 32 bytes long");
                                    }
                                }
                                Err(e) => eprintln!("Decoding error: {}", e),
                            }
                        } else {
                            match crypt::utils::base64_decode(&message.trim()) {
                                Ok(decoded) => {
                                    match asymmetric::decrypt(&get_exchange_sec(), &decoded) {
                                        Ok(decrypted) => {
                                            if decrypted.len() != 32 {
                                                eprintln!("Session key must be exactly 32 bytes");
                                            } else {
                                                let key_array: &[u8; 32] = decrypted
                                                    .as_slice()
                                                    .try_into()
                                                    .expect("Decrypted key is not 32 bytes");

                                                let session_key: Key<Aes256Gcm> =
                                                    GenericArray::from_slice(key_array).clone();

                                                set_session_key(session_key);
                                                println!(
                                                    "Session key set: {}",
                                                    crypt::utils::base64_encode(
                                                        get_session_key().as_slice()
                                                    )
                                                );
                                                let packet = fctp::encapsulate_to_fctp(
                                                    900,
                                                    &fctp_me::get_id(),
                                                    "id",
                                                    &get_server_id(),
                                                    get_session_key(),
                                                );
                                                let _ = tx.send(packet.into_bytes()).await;
                                            }
                                        }
                                        Err(_) => eprintln!("Decryption error"),
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Session key decoding error  {} / {}",
                                        e,
                                        &message.trim()
                                    )
                                }
                            }
                        }
                    } else {
                        fctp::process_fctp_stream(message.clone(), &last_pong_clone).await;
                    }
                    message.clear();
                } else {
                    message.clear();
                    message.push_str(&line);
                    message.push('\n');
                }
            }
            println!("CON CLOSED - Disconnected");
            exit(0)
        });
    }
    tokio::spawn(async move {
        //receive messages from async channel and send them to server - universal sender
        let mut writer = writer;
        while let Some(msg) = rx.recv().await {
            if let Err(e) = writer.write_all(&msg).await {
                eprintln!("[!Couldn't send the message!]: {:?}", e);
                break;
            }
        }
    });

    let last_pong_clone = last_pong.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let elapsed = last_pong_clone.lock().await.elapsed();
            if elapsed.as_millis() > 5000 {
                println!(
                    "server is not responding, last pong was sent {} ms ago",
                    elapsed.as_millis()
                );
            }
        }
    });

    {
        // stdin handle messaging,commands etc. and send it to server
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut stdin = BufReader::new(tokio::io::stdin()).lines();
            while let Ok(Some(line)) = stdin.next_line().await {
                // Parse user input and send it to mpsc channel
                if line.starts_with("/") {
                    let command = line.trim_start_matches('/').trim();
                    let packet = fctp::encapsulate_to_fctp(
                        201,
                        &fctp_me::get_id(),
                        command,
                        &get_server_id(),
                        get_session_key(),
                    );
                    let _ = tx.send(packet.into_bytes()).await;
                } else {
                    match parse_input(&line) {
                        Ok((id, msg)) => {
                            let packet = fctp::encapsulate_to_fctp(
                                200,
                                &fctp_me::get_id(),
                                msg.trim(),
                                id.trim(),
                                get_session_key(),
                            );
                            let _ = tx.send(packet.into_bytes()).await;
                        }
                        Err(e) => {
                            eprintln!("Parse error: {}", e);
                        }
                    }
                }
            }
        });
    }

    {
        // Ping server every 2 seconds with code 10
        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                if get_session_key() != Key::<Aes256Gcm>::default() {
                    sleep(Duration::from_secs(2)).await;
                    if tx
                        .send(
                            fctp::encapsulate_to_fctp(
                                10,
                                &fctp_me::get_id(),
                                "ping",
                                &get_server_id(),
                                get_session_key(),
                            )
                            .into_bytes(),
                        )
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
    }

    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

#[cfg(test)]
mod tests {
    use x25519_dalek::{PublicKey, StaticSecret};

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
    fn regex_userinput_parser() {
        let input = "user:input";
        match parse_input(input) {
            Ok((left, right)) => {
                assert_eq!(left, "user");
                assert_eq!(right, "input");
            }
            Err(e) => panic!("Parsing failed: {}", e),
        }

        let invalid_input = "invalid_input";
        assert!(parse_input(invalid_input).is_err());
    }
    #[test]
    fn test_parse_input_edge_cases() {
        assert!(parse_input("a:b:c").is_err());
        assert!(parse_input("").is_err());
        assert!(parse_input(" : ").is_ok());
    }
    #[test]
    fn test_fctp_encapsulation_and_decapsulation() {
        let code = 200;
        let from = "user123";
        let body = "Hello, world!";
        let to = "user456";

        let message = fctp::encapsulate_to_fctp(code, from, body, to, Key::<Aes256Gcm>::default());
        let decoded = fctp::decapsulate_fctp_message(&message, Key::<Aes256Gcm>::default())
            .expect("Failed to parse FCTP message");

        assert_eq!(decoded.code, code);
        assert_eq!(decoded.from, from);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.to, to);
    }

    #[test]
    fn test_fctp_decapsulation_invalid() {
        let bad_message = "This is not a valid FCTP message";
        assert!(fctp::decapsulate_fctp_message(bad_message, Key::<Aes256Gcm>::default()).is_none());

        let bad_code =
            "FoggyChat Transfer Protocol 0.1\r\nabc\r\nFrom: a\r\nBody: b\r\nTo: c\r\n\r\n";
        assert!(fctp::decapsulate_fctp_message(bad_code, Key::<Aes256Gcm>::default()).is_none());

        let missing_lines =
            "FoggyChat Transfer Protocol 0.1\r\n200\r\nFrom: user\r\nBody: test\r\n\r\n";
        assert!(
            fctp::decapsulate_fctp_message(missing_lines, Key::<Aes256Gcm>::default()).is_none()
        );
    }
    #[test]
    fn test_id_management() {
        fctp_me::set_id("test_id");
        assert_eq!(fctp_me::get_id(), "test_id");

        set_server_id("srv_id");
        assert_eq!(get_server_id(), "srv_id");
    }
}
