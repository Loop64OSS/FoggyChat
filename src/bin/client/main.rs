#![allow(unused_imports)] //ZAMKNIĘCIE RYJA RUST ANALYZER
mod crypt;
mod protocol_utils;
use crypt::asymmetric;
use crypt::symmetric;
use lazy_static::lazy_static;
use protocol_utils::fctp;
use protocol_utils::fctp_me;
use regex::Regex;
use std::process::exit;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::sleep;

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
#[tokio::main]
async fn main() {
    /*
       Main code Logic
    */

    let stream = TcpStream::connect("127.0.0.1:8081").await.unwrap(); // start stream //TODO: SOCKS5stream support and address selection in user input
    let (reader, writer) = stream.into_split();
    let reader = BufReader::new(reader);
    let (tx, mut rx) = mpsc::channel::<String>(100);
    let last_pong = Arc::new(tokio::sync::Mutex::new(Instant::now()));
    let last_pong_clone = last_pong.clone();

    tokio::spawn(async move {
        //receive messages from server and process them
        let mut lines = reader.lines();
        let mut message = String::new();
        // Glue the lines together
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() && !message.is_empty() {
                if message.ends_with('\n') {
                    message.truncate(message.len() - 1);
                    message.push_str("\r\n\r\n");
                }
                fctp::process_fctp_stream(message.clone(), &last_pong_clone).await;
                message.clear();
            } else {
                message.push_str(&line);
                message.push('\n');
            }
        }
        println!("CON CLOSED - Disconnected");
        exit(0)
    });
    tokio::spawn(async move {
        //receive messages from async channel and send them to server - universal sender
        let mut writer = writer;
        while let Some(msg) = rx.recv().await {
            if let Err(e) = writer.write_all(msg.as_bytes()).await {
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
                    );
                    let _ = tx.send(packet + "\n").await;
                } else {
                    match parse_input(&line) {
                        Ok((id, msg)) => {
                            let packet = fctp::encapsulate_to_fctp(
                                200,
                                &fctp_me::get_id(),
                                msg.trim(),
                                id.trim(),
                            );
                            let _ = tx.send(packet + "\n").await;
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
                sleep(Duration::from_secs(2)).await;
                if tx
                    .send(fctp::encapsulate_to_fctp(
                        10,
                        &fctp_me::get_id(), /* TODO: Replace me with user id later*/
                        "ping",
                        &get_server_id(),
                    ))
                    .await
                    .is_err()
                {
                    break;
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
    fn test_id_management() {
        fctp_me::set_id("test_id");
        assert_eq!(fctp_me::get_id(), "test_id");

        set_server_id("srv_id");
        assert_eq!(get_server_id(), "srv_id");
    }
}
