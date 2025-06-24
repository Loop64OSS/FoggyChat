use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, OsRng},
};
use rand::RngCore;
use x25519_dalek::{PublicKey, StaticSecret, x25519};

pub fn generate_keypair() -> ([u8; 32], [u8; 32]) {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (secret.to_bytes(), *public.as_bytes())
}

/// Encrypt with recipient's public key
pub fn encrypt(recipient_pub: &PublicKey, plaintext: &[u8]) -> Vec<u8> {
    let ephemeral = StaticSecret::random_from_rng(OsRng);
    let eph_pub = PublicKey::from(&ephemeral);

    let shared = x25519(ephemeral.to_bytes(), recipient_pub.to_bytes());
    let cipher = XChaCha20Poly1305::new_from_slice(&shared).unwrap();

    let mut nonce = XNonce::default();
    OsRng.fill_bytes(nonce.as_mut());

    let ciphertext = cipher.encrypt(&nonce, plaintext).unwrap();

    [eph_pub.as_bytes(), nonce.as_slice(), &ciphertext].concat()
}

/// Decrypt with recipient's secret key
pub fn decrypt(recipient_secret: &StaticSecret, message: &[u8]) -> Option<Vec<u8>> {
    // ephem(32) | nonce(24) | ciphertext
    if message.len() < 56 {
        return None;
    }

    // Poprawne wyciągnięcie ephemera
    let eph_bytes: [u8; 32] = message[0..32].try_into().unwrap();
    let eph_pub = PublicKey::from(eph_bytes);

    let nonce = XNonce::from_slice(&message[32..56]);
    let ciphertext = &message[56..];

    let shared = x25519(recipient_secret.to_bytes(), eph_pub.to_bytes());
    let cipher = XChaCha20Poly1305::new_from_slice(&shared).unwrap();

    cipher.decrypt(nonce, ciphertext).ok()
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

        let encrypted = encrypt(&rec_pub, msg);
        let decrypted = decrypt(&rec_sec, &encrypted).unwrap();

        assert_eq!(&decrypted, msg);
    }
}
