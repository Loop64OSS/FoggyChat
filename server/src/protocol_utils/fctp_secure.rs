use std::sync::RwLock;

use lazy_static::lazy_static;
use x25519_dalek::{PublicKey, StaticSecret};

lazy_static! {
    static ref CERT_PUB: RwLock<PublicKey> = RwLock::new(PublicKey::from([0u8; 32]));
    static ref CERT_SEC: RwLock<StaticSecret> = RwLock::new(StaticSecret::from([0u8; 32]));
}

pub fn set_cert(new_pubkey: PublicKey, new_sec: StaticSecret) {
    let mut pubkey = CERT_PUB.write().expect("Lock poisoned");
    *pubkey = new_pubkey;
    let mut sec = CERT_SEC.write().expect("Lock poisoned");
    *sec = new_sec;
}

pub fn get_cert_pub() -> PublicKey {
    CERT_PUB.read().expect("Lock poisoned").clone()
}
pub fn get_cert_sec() -> StaticSecret {
    CERT_SEC.read().expect("Lock poisoned").clone()
}
