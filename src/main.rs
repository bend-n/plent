#![feature(lazy_cell, let_chains, iter_intersperse, if_let_guard)]
#[macro_use]
mod bot;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    bot::Bot::spawn().await;
}
