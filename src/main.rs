#![feature(lazy_cell, let_chains)]
#[macro_use]
mod bot;
mod conv;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    bot::Bot::spawn().await;
}
