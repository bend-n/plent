// from moderatior
use std::sync::LazyLock;

use kv::*;

fn cfg() -> kv::Config {
    kv::Config {
        path: "./channels".into(),
        temporary: false,
        use_compression: true,
        flush_every_ms: None,
        cache_capacity: None,
        segment_size: None,
    }
}
static DB: LazyLock<Store> = LazyLock::new(|| Store::new(cfg()).unwrap());
static BU: LazyLock<Bucket<Integer, Vec<u8>>> = LazyLock::new(|| DB.bucket(None).unwrap());

pub fn set(k: u64, v: u64) {
    BU.set(&k.into(), &v.to_le_bytes().to_vec()).unwrap();
    BU.flush().unwrap();
}
pub fn remove(k: u64) -> Option<u64> {
    BU.remove(&k.into())
        .ok()
        .flatten()
        .map(|x| u64::from_le_bytes(x.try_into().unwrap()))
        .inspect(|_| {
            BU.flush().unwrap();
        })
}
pub fn sz() -> f32 {
    DB.size_on_disk().unwrap() as f32 / (1 << 20) as f32
}
