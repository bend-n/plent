use serenity::all::MessageId;
use std::collections::HashMap;
use tokio::sync::MutexGuard;

use super::repos::Repo;

#[derive(Debug)]
pub struct Ownership {
    pub map: HashMap<u64, (String, u64)>,
    path: &'static str,
}
impl Ownership {
    pub fn new(id: &'static str) -> Self {
        let path = format!("repos/{id}/ownership.json").leak();
        Self {
            map: serde_json::from_slice(&std::fs::read(&path).unwrap_or_default())
                .unwrap_or_default(),
            path,
        }
    }

    pub fn insert(&mut self, k: u64, v: (String, u64)) {
        self.map.insert(k, v);
        self.flush();
    }
    fn flush(&self) {
        std::fs::write(self.path, serde_json::to_string_pretty(&self.map).unwrap()).unwrap();
    }
    pub fn get(&self, k: u64) -> &(String, u64) {
        self.map.get(&k).unwrap()
    }
    pub fn erase(&mut self, k: impl Into<u64>) -> Option<String> {
        let x = self.map.remove(&k.into()).map(|(x, _)| x);
        self.flush();
        x
    }

    pub fn whos(&self, x: impl Into<MessageId>) -> &str {
        &self.get(x.into().get()).0
    }
}
pub async fn whos(repo: &'static Repo, x: impl Into<u64>) -> String {
    repo.own().await.get(x.into()).0.clone()
}
pub async fn get(g: &'static Repo) -> MutexGuard<'static, Ownership> {
    g.own().await
}
