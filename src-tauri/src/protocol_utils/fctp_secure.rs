use std::{collections::HashMap, sync::RwLock};

use aes_gcm::{Aes256Gcm, Key};
use lazy_static::lazy_static;
use x25519_dalek::{PublicKey, StaticSecret};

lazy_static! {
    static ref EXCHANGE_PUB: RwLock<PublicKey> = RwLock::new(PublicKey::from([0u8; 32]));
    static ref EXCHANGE_SEC: RwLock<StaticSecret> = RwLock::new(StaticSecret::from([0u8; 32]));
    static ref SESSION_KEY: RwLock<Key<Aes256Gcm>> = RwLock::new(Key::<Aes256Gcm>::default());
    static ref E2EE_PUB: RwLock<PublicKey> = RwLock::new(PublicKey::from([0u8; 32]));
    static ref E2EE_SEC: RwLock<StaticSecret> = RwLock::new(StaticSecret::from([0u8; 32]));
    pub static ref E2EE_KEY_TABLE: RwLock<HashMap<String, PublicKey>> = RwLock::new(HashMap::new());
}
//EXCHANGE
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

//SESSION
pub fn set_session_key(new_session_key: Key<Aes256Gcm>) {
    let mut key = SESSION_KEY.write().expect("Lock poisoned");
    *key = new_session_key;
}
pub fn get_session_key() -> Key<Aes256Gcm> {
    SESSION_KEY.read().expect("Lock poisoned").clone()
}
//E2EE
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
