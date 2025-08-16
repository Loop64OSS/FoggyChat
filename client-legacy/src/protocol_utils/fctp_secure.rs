use std::sync::RwLock;

use aes_gcm::{Aes256Gcm, Key};
use lazy_static::lazy_static;
use x25519_dalek::{PublicKey, StaticSecret};

lazy_static! {
    static ref EXCHANGE_PUB: RwLock<PublicKey> = RwLock::new(PublicKey::from([0u8; 32]));
    static ref EXCHANGE_SEC: RwLock<StaticSecret> = RwLock::new(StaticSecret::from([0u8; 32]));
    static ref SESSION_KEY: RwLock<Key<Aes256Gcm>> = RwLock::new(Key::<Aes256Gcm>::default());
}

pub fn set_exchange(new_pubkey: PublicKey, new_sec: StaticSecret) {
    let mut pubkey = EXCHANGE_PUB.write().expect("Lock poisoned");
    *pubkey = new_pubkey;
    let mut sec = EXCHANGE_SEC.write().expect("Lock poisoned");
    *sec = new_sec;
}

pub fn get_exchange_pub() -> PublicKey {
    EXCHANGE_PUB.read().expect("Lock poisoned").clone()
}
pub fn get_exchange_sec() -> StaticSecret {
    EXCHANGE_SEC.read().expect("Lock poisoned").clone()
}
pub fn set_session_key(new_session_key: Key<Aes256Gcm>) {
    let mut key = SESSION_KEY.write().expect("Lock poisoned");
    *key = new_session_key;
}

pub fn get_session_key() -> Key<Aes256Gcm> {
    SESSION_KEY.read().expect("Lock poisoned").clone()
}
