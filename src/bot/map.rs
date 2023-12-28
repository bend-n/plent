use anyhow::Result;
use mindus::{data::map::ReadError, *};
use poise::serenity_prelude::*;
use std::borrow::Cow;
use std::time::Instant;

use super::{strip_colors, SUCCESS};

pub async fn with(msg: &Message, c: &serenity::client::Context) -> Result<()> {
    let auth = msg.author_nick(c).await.unwrap_or(msg.author.name.clone());
    for a in &msg.attachments {
        if a.filename.ends_with("msav") {
            let s = a.download().await?;
            let then = Instant::now();

            macro_rules! dang {
                ($fmt:literal $(, $args:expr)+) => {{
                    msg.reply_ping(c, format!($fmt $(, $args)+)).await?;
                    return Ok(());
                }};
            }
            // could ignore, but i think if you have a msav, you dont want to ignore failures.
            let m = match Map::deserialize(&mut mindus::data::DataRead::new(&s)) {
                Ok(m) => m,
                Err(ReadError::Decompress(_) | ReadError::Header(_)) => {
                    dang!("`{}` is not a map.", a.filename)
                }
                Err(ReadError::NoSuchBlock(b)) => dang!(
                    "couldnt find block `{b}`. error originates from `{}`",
                    a.filename
                ),
                Err(ReadError::Version(v)) => {
                    dang!(
                        "unsupported version: `{v}`. supported versions: `7`. error originates from `{}`",
                        a.filename
                    )
                }
                Err(ReadError::Read(r)) => {
                    dang!(
                        "failed to read map. error: `{r}`. originates from `{}`",
                        a.filename
                    )
                }
                Err(ReadError::ReadState(r)) => {
                    dang!(
                        "failed to read dyn data in map. error: `{r}`. originates from `{}`",
                        a.filename
                    )
                }
            };
            let t = msg.channel_id.start_typing(&c.http)?;
            let deser_took = then.elapsed();
            let name = strip_colors(m.tags.get("name").unwrap());
            let (render_took, compression_took, total, png) =
                tokio::task::spawn_blocking(move || {
                    let render_took = Instant::now();
                    let i = m.render();
                    let render_took = render_took.elapsed();
                    let compression_took = Instant::now();
                    let png = super::png(i);
                    let compression_took = compression_took.elapsed();
                    let total = then.elapsed();
                    (render_took, compression_took, total, png)
                })
                .await?;
            t.stop();
            msg.channel_id.send_message(c, |m| { m.add_file(AttachmentType::Bytes { data: Cow::from(png), filename: "map.png".to_string() }).embed(|e| e.title(&name).footer(|f| f.text(format!("render of {name} (requested by {auth}) took: {:.3}s (deser: {}ms, render: {:.3}s, compression: {:.3}s)", total.as_secs_f32(), deser_took.as_millis(), render_took.as_secs_f32(), compression_took.as_secs_f32()))).attachment("map.png").color(SUCCESS)) }).await?;
            return Ok(());
        }
    }

    Ok(())
}
