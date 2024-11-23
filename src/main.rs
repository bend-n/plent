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
mod expose;
#[macro_use]
mod bot;
static START: OnceLock<Instant> = OnceLock::new();
#[tokio::main(flavor = "current_thread")]
async fn main() {
    START.get_or_init(Instant::now);
    expose::Server::spawn(<SocketAddr as std::str::FromStr>::from_str("0.0.0.0:2000").unwrap())
        .await;
    bot::Bot::spawn().await;
}
