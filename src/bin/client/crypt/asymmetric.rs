use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, OsRng},
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret, x25519};

pub const VERSION: u8 = 1;

pub fn generate_keypair() -> ([u8; 32], [u8; 32]) {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (secret.to_bytes(), *public.as_bytes())
}

fn derive_key(shared_secret: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared_secret);
    let mut okm = [0u8; 32];
    hk.expand(b"chacha20poly1305 key", &mut okm)
        .expect("HKDF expand should never fail with 32 bytes output");
    okm
}

pub fn encrypt(
    recipient_pub: &PublicKey,
    plaintext: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let ephemeral = StaticSecret::random_from_rng(OsRng);
    let eph_pub = PublicKey::from(&ephemeral);

    let shared = x25519(ephemeral.to_bytes(), recipient_pub.to_bytes());
    let key = derive_key(&shared);
    let cipher = XChaCha20Poly1305::new_from_slice(&key)?;

    let mut nonce = XNonce::default();
    OsRng.fill_bytes(nonce.as_mut());

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| Box::<dyn std::error::Error>::from(format!("Encrypt error: {:?}", e)))?;
    // Format: [version (1) | eph_pub (32) | nonce (24) | ciphertext (...)]
    let mut msg = Vec::with_capacity(1 + 32 + 24 + ciphertext.len());
    msg.push(VERSION);
    msg.extend_from_slice(eph_pub.as_bytes());
    msg.extend_from_slice(nonce.as_slice());
    msg.extend_from_slice(&ciphertext);

    Ok(msg)
}

pub fn decrypt(
    recipient_secret: &StaticSecret,
    message: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // minimal length: 1 (ver) + 32 (eph_pub) + 24 (nonce) + 1 (ciphertext)
    if message.len() < 58 {
        return Err("Message too short".into());
    }

    let version = message[0];
    if version != VERSION {
        return Err(format!("Unsupported version: {}", version).into());
    }

    let eph_bytes: [u8; 32] = message[1..33].try_into()?;
    let eph_pub = PublicKey::from(eph_bytes);

    let nonce = XNonce::from_slice(&message[33..57]);
    let ciphertext = &message[57..];

    let shared = x25519(recipient_secret.to_bytes(), eph_pub.to_bytes());
    let key = derive_key(&shared);
    let cipher = XChaCha20Poly1305::new_from_slice(&key)?;

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| Box::<dyn std::error::Error>::from(format!("Decrypt error: {:?}", e)))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_and_crypto_roundtrip() {
        let (rec_sec_bytes, rec_pub_bytes) = generate_keypair();
        let rec_sec = StaticSecret::from(rec_sec_bytes);
        let rec_pub = PublicKey::from(rec_pub_bytes);

        let msg = b"Test wiadomosc";

        let encrypted = encrypt(&rec_pub, msg).expect("Encryption failed");
        let decrypted = decrypt(&rec_sec, &encrypted).expect("Decryption failed");

        assert_eq!(&decrypted, msg);
    }
}
