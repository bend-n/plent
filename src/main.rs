#![allow(incomplete_features)]
#![feature(
    let_chains,
    generic_const_exprs,
    effects,
    lazy_cell_consume,
    iter_intersperse,
    if_let_guard,
    const_mut_refs,
    backtrace_frames
)]
emojib::the_crate! {}

use std::{net::SocketAddr, sync::OnceLock, time::Instant};
#[cfg(feature = "server")]
mod expose;
#[macro_use]
mod bot;
static START: OnceLock<Instant> = OnceLock::new();
#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("check clones");
    bot::clone();
    START.get_or_init(Instant::now);
    #[cfg(feature = "server")]
    expose::Server::spawn(<SocketAddr as std::str::FromStr>::from_str("0.0.0.0:2000").unwrap())
        .await;
    #[cfg(not(feature = "server"))]
    bot::Bot::spawn().await;
}
