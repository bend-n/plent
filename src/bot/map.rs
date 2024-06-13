use anyhow::Result;
use mindus::{data::map::ReadError, *};
use poise::{serenity_prelude::*, CreateReply};
use std::{
    ops::ControlFlow,
    time::{Duration, Instant},
};

use super::{strip_colors, SUCCESS};

fn string((x, f): (ReadError, &str)) -> String {
    match x {
        ReadError::Decompress(_) | ReadError::Header(_) => {
            format!("not a map.")
        }
        ReadError::NoSuchBlock(b) => {
            format!("couldnt find block `{b}`. error originates from `{f}`")
        }
        ReadError::Version(v) => {
            format!(
                "unsupported version: `{v}`. supported versions: `7`. error originates from `{f}`",
            )
        }
        ReadError::Read(r) => {
            format!("failed to read map. error: `{r}`. originates from `{f}`")
        }
        ReadError::ReadState(r) => {
            format!("failed to read dyn data in map. error: `{r}`. originates from `{f}`")
        }
    }
}

pub async fn download(a: &Attachment) -> Result<(Result<Map, (ReadError, &str)>, Duration)> {
    let s = a.download().await?;
    let then = Instant::now();

    // could ignore, but i think if you have a msav, you dont want to ignore failures.
    Ok((
        Map::deserialize(&mut mindus::data::DataRead::new(&s)).map_err(|x| (x, &*a.filename)),
        then.elapsed(),
    ))
}

pub async fn scour(m: &Message) -> Result<Option<(Result<Map, (ReadError, &str)>, Duration)>> {
    for a in &m.attachments {
        if a.filename.ends_with("msav") {
            return Ok(Some(download(a).await?));
        }
    }
    Ok(None)
}

pub async fn reply(a: &Attachment) -> Result<ControlFlow<CreateReply, String>> {
    let (m, deser_took) = match download(a).await? {
        (Err(e), _) => return Ok(ControlFlow::Continue(string(e))),
        (Ok(m), deser_took) => (m, deser_took),
    };
    let name = strip_colors(m.tags.get("name").or(m.tags.get("mapname")).unwrap());
    let (
        Timings {
            deser_took,
            render_took,
            compression_took,
            total,
        },
        png,
    ) = render(m, deser_took).await;
    Ok(ControlFlow::Break(CreateReply::default().attachment(CreateAttachment::bytes(png,"map.png")).embed(CreateEmbed::new().title(&name).footer(CreateEmbedFooter::new(format!("render of {name} took: {:.3}s (deser: {}ms, render: {:.3}s, compression: {:.3}s)", total.as_secs_f32(), deser_took.as_millis(), render_took.as_secs_f32(), compression_took.as_secs_f32()))).attachment("map.png").color(SUCCESS))))
}

struct Timings {
    deser_took: Duration,
    render_took: Duration,
    compression_took: Duration,
    total: Duration,
}
async fn render(m: Map, deser_took: Duration) -> (Timings, Vec<u8>) {
    tokio::task::spawn_blocking(move || {
        let render_took = Instant::now();
        let i = m.render();
        let render_took = render_took.elapsed();
        let compression_took = Instant::now();
        let png = super::png(i);
        let compression_took = compression_took.elapsed();
        let total = deser_took + render_took + compression_took;
        (
            Timings {
                deser_took,
                render_took,
                compression_took,
                total,
            },
            png,
        )
    })
    .await
    .unwrap()
}

pub async fn with(msg: &Message, c: &serenity::client::Context) -> Result<()> {
    let auth = msg.author_nick(c).await.unwrap_or(msg.author.name.clone());
    let (m, deser_took) = match scour(msg).await? {
        None => return Ok(()),
        Some((Err(e), _)) => return Ok(drop(msg.reply(c, string(e)).await?)),
        Some((Ok(m), deser_took)) => (m, deser_took),
    };
    let t = msg.channel_id.start_typing(&c.http);
    let name = strip_colors(m.tags.get("name").or(m.tags.get("mapname")).unwrap());
    let (
        Timings {
            deser_took,
            render_took,
            compression_took,
            total,
        },
        png,
    ) = render(m, deser_took).await;
    t.stop();
    msg.channel_id.send_message(c,CreateMessage::new().add_file(CreateAttachment::bytes(png,"map.png")).embed(CreateEmbed::new().title(&name).footer(CreateEmbedFooter::new(format!("render of {name} (requested by {auth}) took: {:.3}s (deser: {}ms, render: {:.3}s, compression: {:.3}s)", total.as_secs_f32(), deser_took.as_millis(), render_took.as_secs_f32(), compression_took.as_secs_f32()))).attachment("map.png").color(SUCCESS))).await?;
    Ok(())
}
