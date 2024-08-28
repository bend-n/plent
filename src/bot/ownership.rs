use serenity::all::MessageId;
use std::{collections::HashMap, sync::LazyLock};
use tokio::sync::Mutex;

pub static MAP: LazyLock<Mutex<HashMap<u64, (String, u64)>>> = LazyLock::new(|| {
    Mutex::new(serde_json::from_slice(&std::fs::read("repo/ownership.json").unwrap()).unwrap())
});

pub async fn insert(k: u64, v: (String, u64)) {
    let mut lock = MAP.lock().await;
    lock.insert(k, v);
    std::fs::write(
        "repo/ownership.json",
        serde_json::to_string_pretty(&*lock).unwrap(),
    )
    .unwrap();
}
pub async fn get(k: u64) -> (String, u64) {
    MAP.lock().await[&k].clone()
}
pub async fn erase(k: u64) -> Option<String> {
    let mut lock = MAP.lock().await;
    let x = lock.remove(&k).map(|(x, _)| x);
    std::fs::write(
        "repo/ownership.json",
        serde_json::to_string_pretty(&*lock).unwrap(),
    )
    .unwrap();
    x
}
pub async fn whos(x: impl Into<MessageId>) -> String {
    get(x.into().get()).await.0
}
