mod crypt;
#[allow(unused_imports)] //ZAMKNIĘCIE RYJA RUST ANALYZER
use crypt::asymmetric;
use crypt::symmetric;
use std::process::exit;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::sleep;

struct FctpMessage {
    code: i32,
    from: String,
    body: String,
    #[allow(dead_code)] //pieprzony rust analyzer \/
    to: String,
}

fn decapsulate_fctp_message(msg: &str) -> Option<FctpMessage> {
    let mut lines = msg.lines();

    if lines.next()? != "FoggyChat Transfer Protocol 0.1" {
        return None;
    }

    let code = lines.next()?.trim().parse::<i32>().ok()?; // ABSOLUTELY REQUIRED, MUST BE A NUMBER IN INT FORMAT 32 BIT SIZE
    let from = lines.next()?.strip_prefix("From: ")?.trim().to_string();
    let body = lines.next()?.strip_prefix("Body: ")?.trim().to_string();
    let to = lines.next()?.strip_prefix("To: ")?.trim().to_string(); // To: is optional, but we keep it for consistency. TL/DR: ignored

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

async fn process_fctp_stream(message: String) {
    if let Some(msg) = decapsulate_fctp_message(&message) {
        //pool 4xx - client-side errors
        //pool 5xx - server-side errors
        //pool 2xx - message handling
        //pool 1x - connection handling (ping/pong) TODO: pong
        //pool 8xx - encryption handshake TODO
        //pool 9xx - logon information ex. logged in, session info TODO
        match msg.code {
            200 => println!("[Message received] <{}> {}", msg.from, msg.body), //User-user direct message
            201 => println!(" {}: {}", msg.from, msg.body), //From-server general direct message
            405 => println!("[!cl!] {}: {}", msg.from, msg.body), //Client-side error
            505 => println!("[!sv!] {}: {}", msg.from, msg.body), //Server-side error
            11 => {} //Pong handling (ping code 10, pong code 11), TODO: connection keep-alive
            _ => println!(
                "[!Unsupported code!]: {}\n !Update your client software or ask server administrator to update his server software!",
                msg.code
            ), //Unknown code handling
        }
    } else {
        eprintln!("[!Malformed header!]: {}", message);
    }
}

#[tokio::main]
async fn main() {
    /*
       This section is dedicated for encryption library testing:
        - AES256
        - ECDHE or ECIES
    */

    let key = symmetric::keygen();

    match symmetric::encrypt_message("test", &key) {
        Ok(encrypted) => {
            println!("Encrypted: {}", encrypted);

            match symmetric::decrypt_message(&encrypted, &key) {
                Ok(decrypted) => println!("Decrypted: {}", decrypted),
                Err(e) => eprintln!("Decryption error: {}", e),
            }
        }
        Err(e) => eprintln!("Encryption error: {}", e),
    }

    /*
       Main code Logic
    */

    let stream = TcpStream::connect("127.0.0.1:8081").await.unwrap(); // start stream //TODO: SOCKS5stream support and address selection in user input
    let (reader, writer) = stream.into_split();
    let reader = BufReader::new(reader);
    let (tx, mut rx) = mpsc::channel::<String>(100);

    print!("\n");

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
                process_fctp_stream(message.clone()).await;
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

    {
        // stdin handle messaging,commands etc. and send it to server
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut stdin = BufReader::new(tokio::io::stdin()).lines();
            while let Ok(Some(line)) = stdin.next_line().await {
                let packet = encapsulate_to_fctp(
                    200,
                    "me",
                    line.split(':').nth(1).map(|s| s.trim()).unwrap_or(""),
                    line.split(':').nth(0).map(|s| s.trim()).unwrap_or(""),
                );

                let _ = tx.send(packet + "\n").await;
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
                    .send(encapsulate_to_fctp(10, "me", "ping", "server"))
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
