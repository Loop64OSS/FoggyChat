use std::{collections::HashMap, time::Duration};

use crate::crypt::utils::base64_decode;
use crate::protocol_utils::fctp;
use crate::protocol_utils::fctp::{get_current_recipient, FctpMessage, LAST_ACK};
use crate::protocol_utils::fctp_me;
use crate::send_packet;
use aes_gcm::{Aes256Gcm, Key};
use crypt::asymmetric;
use lazy_static::lazy_static;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tauri::AppHandle;
use tokio::sync::mpsc;
use tokio::time::{sleep, Instant};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::{
    crypt::{self},
    ui_emit_status,
};

/* Timeouts used during handshake and key requests */
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const KEY_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/* Global exchange/session/E2EE state */
lazy_static! {
    static ref EXCHANGE_PUB: RwLock<PublicKey> = RwLock::new(PublicKey::from([0u8; 32]));
    static ref EXCHANGE_SEC: RwLock<StaticSecret> = RwLock::new(StaticSecret::from([0u8; 32]));
    static ref SESSION_KEY: RwLock<Key<Aes256Gcm>> = RwLock::new(Key::<Aes256Gcm>::default());
    static ref E2EE_PUB: RwLock<PublicKey> = RwLock::new(PublicKey::from([0u8; 32]));
    static ref E2EE_SEC: RwLock<StaticSecret> = RwLock::new(StaticSecret::from([0u8; 32]));
    pub static ref E2EE_KEY_TABLE: RwLock<HashMap<String, PublicKey>> = RwLock::new(HashMap::new());
}

/* Exchange key helpers */
pub fn set_exchange(new_pubkey: PublicKey, new_sec: StaticSecret) {
    let mut pubkey = EXCHANGE_PUB.write().expect("Lock poisoned");
    *pubkey = new_pubkey;
    let mut sec = EXCHANGE_SEC.write().expect("Lock poisoned");
    *sec = new_sec;
}

/* Return the current exchange public key */
pub fn get_exchange_pub() -> PublicKey {
    EXCHANGE_PUB.read().expect("Lock poisoned").clone()
}

/* Return the current exchange secret */
pub fn get_exchange_sec() -> StaticSecret {
    EXCHANGE_SEC.read().expect("Lock poisoned").clone()
}

/* Session key helpers */
pub fn set_session_key(new_session_key: Key<Aes256Gcm>) {
    let mut key = SESSION_KEY.write().expect("Lock poisoned");
    *key = new_session_key;
}

/* Return the active session key */
pub fn get_session_key() -> Key<Aes256Gcm> {
    SESSION_KEY.read().expect("Lock poisoned").clone()
}

/* E2EE key helpers */
pub fn set_e2ee(new_pubkey: PublicKey, new_sec: StaticSecret) {
    let mut pubkey = E2EE_PUB.write().expect("Lock poisoned");
    *pubkey = new_pubkey;
    let mut sec = E2EE_SEC.write().expect("Lock poisoned");
    *sec = new_sec;
}

pub fn get_e2ee_pub() -> PublicKey {
    E2EE_PUB.read().expect("Lock poisoned").clone()
}

pub fn get_e2ee_sec() -> StaticSecret {
    E2EE_SEC.read().expect("Lock poisoned").clone()
}

/* E2EE key table helpers */
pub fn get_pk_from_e2ee_key_table(username: &str) -> Option<PublicKey> {
    E2EE_KEY_TABLE
        .read()
        .expect("Failed to acquire read lock")
        .get(username)
        .cloned()
}

pub fn has_pk_in_e2ee_key_table(username: &str) -> bool {
    E2EE_KEY_TABLE
        .read()
        .expect("Failed to acquire read lock")
        .contains_key(username)
}

/* Remove a public key entry by username. Returns true if removed. */
pub fn remove_pk_from_e2ee_key_table(username: &str) -> bool {
    if let Ok(mut table) = E2EE_KEY_TABLE.write() {
        table.remove(username).is_some()
    } else {
        eprintln!("Failed to acquire write lock to remove E2EE key");
        false
    }
}

/* Conditionally remove a public key if it matches the provided key */
#[allow(unused)]
pub fn remove_pk_if_matches(username: &str, key: PublicKey) -> bool {
    if let Ok(mut table) = E2EE_KEY_TABLE.write() {
        if let Some(existing) = table.get(username) {
            if *existing == key {
                table.remove(username);
                return true;
            }
        }
        false
    } else {
        eprintln!("Failed to acquire write lock to conditionally remove E2EE key");
        false
    }
}

/* Clear the E2EE key table */
#[allow(unused)]
pub fn clear_e2ee_key_table() {
    if let Ok(mut table) = E2EE_KEY_TABLE.write() {
        table.clear();
    } else {
        eprintln!("Failed to acquire write lock to clear E2EE key table");
    }
}

/* Handle an incoming handshake-phase message (plaintext base64). */
pub async fn handle_handshake_message(
    data: &[u8],
    app: &AppHandle,
    tx: &mpsc::Sender<Vec<u8>>,
    fp_verified: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let message_str = String::from_utf8_lossy(data).trim().to_string();

    println!("Handshake phase - received {} bytes", message_str.len());

    let decoded = crypt::utils::base64_decode(&message_str)
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    println!("Decoded {} bytes", decoded.len());

    if get_exchange_pub() == PublicKey::from([0u8; 32]) {
        /* Certificate public key phase */
        println!("Processing certificate exchange...");
        handle_certificate_exchange(decoded, app, tx, fp_verified).await
    } else {
        /* Session key exchange phase */
        println!("Processing session key exchange...");
        handle_session_key_exchange(decoded, app, tx).await
    }
}

/* Process certificate exchange: verify fingerprint, generate exchange key */
pub async fn handle_certificate_exchange(
    decoded: Vec<u8>,
    app: &AppHandle,
    tx: &mpsc::Sender<Vec<u8>>,
    fp_verified: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    if decoded.len() != 32 {
        return Err(format!("Invalid certificate key length: {}", decoded.len()).into());
    }

    let cert_pub = PublicKey::from(
        <[u8; 32]>::try_from(decoded.as_slice()).map_err(|_| "Failed to convert to key array")?,
    );
    /* Request fingerprint verification from UI */
    ui_emit_status(
        app.clone(),
        "user::verify_fp".into(),
        &crypt::utils::blake3_hash(&decoded),
    );

    /* Wait for user to verify fingerprint (timeout) */
    let start = Instant::now();
    while !fp_verified.load(Ordering::Relaxed) {
        if start.elapsed() > HANDSHAKE_TIMEOUT {
            return Err("Fingerprint verification timeout".into());
        }
        sleep(Duration::from_millis(100)).await;
    }

    /* Generate exchange keypair and send encrypted public part to server */
    let (rec_sec_bytes, rec_pub_bytes) = asymmetric::keypairgen();
    let rec_sec = StaticSecret::from(rec_sec_bytes);
    let rec_pub = PublicKey::from(rec_pub_bytes);
    set_exchange(rec_pub, rec_sec);

    let encrypted = asymmetric::encrypt(&cert_pub, rec_pub.as_bytes())
        .map_err(|e| format!("Encryption error: {}", e))?;

    let encoded = crypt::utils::base64_encode(&encrypted);
    tx.send(format!("{}\r\n\r\n", encoded).into_bytes()).await?;
    ui_emit_status(app.clone(), "ok", "con_established");

    Ok(())
}

/* Process session key exchange: decrypt session key and finish handshake */
pub async fn handle_session_key_exchange(
    decoded: Vec<u8>,
    _app: &AppHandle,
    tx: &mpsc::Sender<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Decrypting session key...");

    let decrypted = asymmetric::decrypt(&get_exchange_sec(), &decoded)
        .map_err(|e| format!("Session key decryption error: {:?}", e))?;

    if decrypted.len() != 32 {
        return Err(format!("Invalid session key length: {}", decrypted.len()).into());
    }

    let key_array: [u8; 32] = decrypted
        .try_into()
        .map_err(|_| "Failed to convert session key to array")?;

    let session_key = Key::<Aes256Gcm>::from_slice(&key_array).clone();
    set_session_key(session_key);

    println!(
        "OK: Session key established: {}",
        crypt::utils::base64_encode(get_session_key().as_slice())
    );

    sleep(Duration::from_millis(100)).await;

    /* Request assigned ID from server */
    println!("Requesting ID from server...");
    let server_id = fctp::get_server_id().unwrap_or_default();
    let packet = fctp::encapsulate_to_fctp(
        &fctp::FctpMessage::new(fctp::FctpCode::Hello, fctp_me::get_id(), "id", server_id),
        &get_session_key(),
    )
    .map_err(|e| format!("FctpError creating Hello: {:?}", e))?;
    tx.send(packet).await?;

    /* Generate and send E2EE public key to server */
    println!("Generating E2EE keypair...");
    let (rec_sec_bytes, rec_pub_bytes) = asymmetric::keypairgen();
    let rec_sec = StaticSecret::from(rec_sec_bytes);
    let rec_pub = PublicKey::from(rec_pub_bytes);
    set_e2ee(rec_pub, rec_sec);

    println!("Sending E2EE public key to server...");
    let e2ee_pub = get_e2ee_pub();
    let server_id = fctp::get_server_id().unwrap_or_default();
    let packet = fctp::encapsulate_to_fctp(
        &fctp::FctpMessage::new(
            fctp::FctpCode::PublicKeyExchange,
            fctp_me::get_id(),
            crypt::utils::base64_encode(e2ee_pub.as_bytes()),
            server_id,
        ),
        &get_session_key(),
    )
    .map_err(|e| format!("FctpError creating PublicKeyExchange: {:?}", e))?;
    tx.send(packet).await?;

    println!("OK: Handshake complete!");
    Ok(())
}

/* Send a KeyRequest to the server for the given recipient */
async fn request_public_key(recipient: &str) -> Result<(), String> {
    let server_id = fctp::get_server_id().map_err(|e| format!("Failed to get server ID: {}", e))?;

    let packet = fctp::encapsulate_to_fctp(
        &fctp::FctpMessage::new(
            fctp::FctpCode::KeyRequest,
            fctp_me::get_id(),
            recipient,
            server_id,
        ),
        &get_session_key(),
    )
    .map_err(|e| format!("FctpError: {:?}", e))?;

    send_packet(packet).await
}

/* Request a recipient's public key and wait (with timeout) for a response */
pub async fn handle_non_existent_e2ee_key(recipient: &str) -> Result<(), String> {
    request_public_key(recipient.trim()).await?;

    /* wait for LAST_ACK to be set by incoming KeyRequest response */
    let start = Instant::now();
    Ok(while !fctp::LAST_ACK.load(Ordering::Relaxed) {
        if start.elapsed() > KEY_REQUEST_TIMEOUT {
            return Err("Timeout waiting for recipient's public key".into());
        }
        sleep(Duration::from_millis(100)).await;
    })
}

/* Handle incoming KeyRequest responses from server */
pub async fn handle_e2ee_key_request(_app: &AppHandle, msg: FctpMessage) {
    let key_bytes = match base64_decode(&msg.body.trim()) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Base64 decode failed: {}", e);
            return;
        }
    };

    if key_bytes.len() != 32 {
        eprintln!("Invalid key length: expected 32, got {}", key_bytes.len());
        return;
    }

    let key_array: [u8; 32] = match key_bytes.as_slice().try_into() {
        Ok(arr) => arr,
        Err(_) => {
            eprintln!("Failed to convert key bytes to array");
            return;
        }
    };

    let recipient_key = PublicKey::from(key_array);

    let recipient = match get_current_recipient() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to get current recipient: {}", e);
            return;
        }
    };

    if let Ok(mut table) = E2EE_KEY_TABLE.write() {
        table.insert(recipient, recipient_key);
        LAST_ACK.store(true, Ordering::Relaxed);
    } else {
        eprintln!("Failed to acquire E2EE key table lock");
    }
}
