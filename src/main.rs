#![feature(lazy_cell, let_chains, iter_intersperse, if_let_guard, const_mut_refs)]

use std::{sync::OnceLock, time::Instant};
#[macro_use]
mod bot;
static START: OnceLock<Instant> = OnceLock::new();
#[tokio::main(flavor = "current_thread")]
async fn main() {
    START.get_or_init(|| Instant::now());
    bot::Bot::spawn().await;
}
