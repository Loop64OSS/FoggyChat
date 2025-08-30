use lazy_static::lazy_static;
use std::sync::RwLock;

lazy_static! {
    static ref ID: RwLock<String> = RwLock::new(String::new());
}
//SETTING CLIENT ID
pub fn set_id(new_id: &str) {
    let mut id = ID.write().expect("Lock poisoned");
    *id = new_id.to_string();
}

pub fn get_id() -> String {
    ID.read().expect("Lock poisoned").clone()
}
