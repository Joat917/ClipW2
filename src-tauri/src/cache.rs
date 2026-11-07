use std::collections::HashMap;
use std::sync::Mutex;

pub struct ClipboardCache {
    pub data: Mutex<HashMap<u32, Vec<u8>>>,
    pub hashes: Mutex<HashMap<u32, String>>,
}

impl ClipboardCache {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
            hashes: Mutex::new(HashMap::new()),
        }
    }
}
