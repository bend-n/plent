#![allow(incomplete_features)]
#![feature(
    try_blocks,
    try_blocks_heterogeneous,
    const_array,
    const_closures,
    generic_const_exprs,
    iter_intersperse,
    const_trait_impl
)]
emojib::the_crate! {}

use std::{sync::OnceLock, time::Instant};

// use crate::bot::stats_;
#[cfg(feature = "server")]
mod expose;
#[macro_use]
mod bot;
static START: OnceLock<Instant> = OnceLock::new();
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // crate::bot::stats_(None).await.3.save("what.png");
    // return;
    println!("check clones");
    bot::clone();
    START.get_or_init(Instant::now);
    #[cfg(feature = "server")]
    expose::Server::spawn(<SocketAddr as std::str::FromStr>::from_str("0.0.0.0:2000").unwrap())
        .await;
    #[cfg(not(feature = "server"))]
    bot::Bot::spawn().await;
}
