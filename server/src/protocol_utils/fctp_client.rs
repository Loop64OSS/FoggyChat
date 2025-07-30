use std::{collections::HashMap, sync::Arc};

use aes_gcm::{Aes256Gcm, Key};
use tokio::sync::Mutex;
use x25519_dalek::PublicKey;

pub type Clients = Arc<Mutex<HashMap<String, ClientInfo>>>;
#[derive(Clone)]
pub struct ClientInfo {
    pub socket: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    pub connected_at: std::time::Instant,
    pub conn_session_key: Key<Aes256Gcm>,
}
