#![allow(dead_code)]
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::prelude::*;

const DIVIDER: &str = "::";
pub fn keygen() -> Key<Aes256Gcm> {
    Aes256Gcm::generate_key(&mut OsRng)
}

pub fn encrypt(data: &str, key: &Key<Aes256Gcm>) -> Result<String, aes_gcm::Error> {
    // this implementation will always return a optimized string with nonce number

    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bit

    let cipher_text = cipher.encrypt(&nonce, data.as_bytes())?;

    let string_cipher_text = BASE64_STANDARD.encode(&cipher_text);
    let string_nonce = BASE64_STANDARD.encode(&nonce);

    let result = format!("{string_cipher_text}{DIVIDER}{string_nonce}");
    Ok(result)
}
pub fn decrypt(
    data: &str,
    key: &Key<Aes256Gcm>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_encryption_roundtrip() {
        let random_key = keygen();

        let msg = "TEST TEST TEST 123 ąęðąśðąśðąś";

        let encrypted_message = encrypt(&msg, &random_key).unwrap();
        let decrypted_message = decrypt(&encrypted_message, &random_key).unwrap();

        println!("Encrypted message: {}", encrypted_message);
        println!("Decrypted message: {}", decrypted_message);
        println!("Key: {}", BASE64_STANDARD.encode(random_key.as_slice()));
        assert_eq!(decrypted_message, msg);
    }
}
