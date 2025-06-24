use aes_gcm::{
    Aes256Gcm,
    Key, // Or `Aes128Gcm`
    Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::prelude::*;

const DIVIDER: &str = "::";
pub fn keygen() -> Key<Aes256Gcm> {
    Aes256Gcm::generate_key(&mut OsRng)
}

pub fn encrypt_message(data: &str, key: &Key<Aes256Gcm>) -> Result<String, aes_gcm::Error> {
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bit

    let cipher_text = cipher.encrypt(&nonce, data.as_bytes())?;

    let string_cipher_text = BASE64_STANDARD.encode(&cipher_text);
    let string_nonce = BASE64_STANDARD.encode(&nonce);

    let result = format!("{string_cipher_text}{DIVIDER}{string_nonce}");
    Ok(result)
}
pub fn decrypt_message(
    data: &str,
    key: &Key<Aes256Gcm>,
) -> Result<String, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = data.split(DIVIDER).collect();
    if parts.len() != 2 {
        return Err("Invalid encrypted format".into());
    }

    let cipher_text = BASE64_STANDARD.decode(parts[0])?;
    let nonce_bytes = BASE64_STANDARD.decode(parts[1])?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let cipher = Aes256Gcm::new(key);
    let plain_text_bytes = cipher
        .decrypt(nonce, cipher_text.as_ref())
        .map_err(|e| format!("Decryption failed: {:?}", e))?;
    let plain_text = String::from_utf8(plain_text_bytes)?;

    Ok(plain_text)
}
