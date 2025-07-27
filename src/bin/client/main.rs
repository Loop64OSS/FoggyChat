#![allow(unused_imports)] //ZAMKNIĘCIE RYJA RUST ANALYZER
mod crypt;
use crypt::asymmetric;
use crypt::symmetric;
use lazy_static::lazy_static;
use regex::Regex;
use std::process::exit;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::sleep;
use x25519_dalek::{PublicKey, StaticSecret};
struct FctpMessage {
    code: i32,
    from: String,
    body: String,
    #[allow(dead_code)] //pieprzony rust analyzer \/
    to: String,
}

/*
    Set global user ID using OnceLock
*/

lazy_static! {
    static ref ID: RwLock<String> = RwLock::new(String::new());
    static ref SERVER_ID: RwLock<String> = RwLock::new(String::new());
}

pub fn set_id(new_id: &str) {
    let mut id = ID.write().expect("Lock poisoned");
    *id = new_id.to_string();
}

pub fn get_id() -> String {
    ID.read().expect("Lock poisoned").clone()
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
fn decapsulate_fctp_message(msg: &str) -> Option<FctpMessage> {
    let mut lines = msg.lines();

    if lines.next()? != "FoggyChat Transfer Protocol 0.1" {
        println!(
            "Protocol header does not match expected format. Update your client or contact the administrator of the server."
        );
        return None;
    }

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

fn encapsulate_to_fctp(code: i32, from: &str, body: &str, to: &str) -> String {
    format!(
        "FoggyChat Transfer Protocol 0.1\r\n{}\r\nFrom: {}\r\nBody: {}\r\nTo: {}\r\n\r\n",
        code, from, body, to
    )
}
async fn process_fctp_stream(message: String, last_pong: &Arc<Mutex<Instant>>) {
    if let Some(msg) = decapsulate_fctp_message(&message) {
        //pool 4xx - client-side errors
        //pool 5xx - server-side errors
        //pool 2xx - message handling
        //pool 1x - //TODO: connection handling (ping/pong)
        //pool 8xx - //TODO: encryption handshake
        //pool 9xx - //TODO: session information ex. logged in, id, etc.
        match msg.code {
            200 => println!("[Message received] <{}> {}", msg.from, msg.body), //User-user direct message
            201 => println!("SERVER: {}", msg.body), //From-server general direct message
            405 => println!("[!cl!] {}", msg.body),  //Client-side error
            505 => println!("[!sv!] {}", msg.body),  //Server-side error
            900 => {
                if msg.body == "id" {
                    let id_clone = msg.to.clone();
                    let server_id_clone = msg.from.clone();
                    set_id(&id_clone);
                    set_server_id(&server_id_clone);
                }
                println!("DEBUG: ID: {}", get_id());
            }
            11 => {
                let mut pong_time = last_pong.lock().await;
                *pong_time = Instant::now();
            } //Pong handling (ping code 10, pong code 11)
            _ => println!(
                "[!Unsupported code!]: {}\n !Update your client software or ask server administrator to update his server software!",
                msg.code
            ), //Unknown code handling
        }
    } else {
        eprintln!("[!Malformed header!]: {}", message);
    }
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
                process_fctp_stream(message.clone(), &last_pong_clone).await;
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
                match parse_input(&line) {
                    Ok((id, msg)) => {
                        let packet = encapsulate_to_fctp(200, &get_id(), msg.trim(), id.trim());
                        let _ = tx.send(packet + "\n").await;
                    }
                    Err(e) => {
                        eprintln!("Parse error: {}", e);
                    }
                }
                match &line.starts_with("/") {
                    true => {
                        let command = line.trim_start_matches('/').trim();
                        let packet = encapsulate_to_fctp(201, &get_id(), command, &get_server_id());
                        let _ = tx.send(packet + "\n").await;
                    }
                    false => {
                        eprintln!("Commands must start with '/'");
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
                    .send(encapsulate_to_fctp(
                        10,
                        &get_id(), /* TODO: Replace me with user id later*/
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

        let message = encapsulate_to_fctp(code, from, body, to);
        let decoded = decapsulate_fctp_message(&message).expect("Failed to parse FCTP message");

        assert_eq!(decoded.code, code);
        assert_eq!(decoded.from, from);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.to, to);
    }

    #[test]
    fn test_fctp_decapsulation_invalid() {
        let bad_message = "This is not a valid FCTP message";
        assert!(decapsulate_fctp_message(bad_message).is_none());

        let bad_code =
            "FoggyChat Transfer Protocol 0.1\r\nabc\r\nFrom: a\r\nBody: b\r\nTo: c\r\n\r\n";
        assert!(decapsulate_fctp_message(bad_code).is_none());

        let missing_lines =
            "FoggyChat Transfer Protocol 0.1\r\n200\r\nFrom: user\r\nBody: test\r\n\r\n";
        assert!(decapsulate_fctp_message(missing_lines).is_none());
    }
    #[test]
    fn test_id_management() {
        set_id("test_id");
        assert_eq!(get_id(), "test_id");

        set_server_id("srv_id");
        assert_eq!(get_server_id(), "srv_id");
    }
}
