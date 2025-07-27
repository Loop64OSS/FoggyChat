use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

pub type Clients = Arc<Mutex<HashMap<String, ClientInfo>>>;
pub struct ClientInfo {
    pub socket: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    pub connected_at: std::time::Instant,
}
