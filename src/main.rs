#![feature(let_chains, iter_intersperse, if_let_guard, const_mut_refs)]

use std::{net::SocketAddr, sync::OnceLock, time::Instant};
mod expose;
#[macro_use]
mod bot;
static START: OnceLock<Instant> = OnceLock::new();
#[tokio::main(flavor = "current_thread")]
async fn main() {
    START.get_or_init(|| Instant::now());
    expose::Server::spawn(<SocketAddr as std::str::FromStr>::from_str("0.0.0.0:2000").unwrap())
        .await;
    bot::Bot::spawn().await;
}
