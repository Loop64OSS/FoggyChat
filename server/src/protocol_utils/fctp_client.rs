use std::{collections::HashMap, sync::Arc};

use aes_gcm::{Aes256Gcm, Key};
use tokio::sync::Mutex;
use x25519_dalek::PublicKey;

pub type Clients = Arc<Mutex<HashMap<String, ClientInfo>>>;
#[derive(Clone)]
pub struct ClientInfo {
    pub socket: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    pub conn_session_key: Key<Aes256Gcm>,
    pub ext_connected_at: std::time::Instant,
    pub ext_session_username: String,
    pub ext_rate_limit_last_packet: std::time::Instant,
}
