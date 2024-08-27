use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

static MAP: LazyLock<Mutex<HashMap<u64, (String, u64)>>> = LazyLock::new(|| {
    Mutex::new(serde_json::from_slice(&std::fs::read("repo/ownership.json").unwrap()).unwrap())
});

pub fn insert(k: u64, v: (String, u64)) {
    MAP.lock().unwrap().insert(k, v);
    std::fs::write("repo/ownership.json", serde_json::to_string(&*MAP).unwrap()).unwrap();
}
pub fn get(k: u64) -> (String, u64) {
    MAP.lock().unwrap()[&k].clone()
}
pub fn erase(k: u64) -> Option<(String, u64)> {
    let x = MAP.lock().unwrap().remove(&k);
    std::fs::write("repo/ownership.json", serde_json::to_string(&*MAP).unwrap()).unwrap();
    x
}
