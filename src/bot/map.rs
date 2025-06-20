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
    let (a, e) = embed(m, deser_took).await;
    Ok(ControlFlow::Break(
        CreateReply::default().attachment(a).embed(e),
    ))
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

pub async fn find(
    msg: &Message,
    c: &serenity::client::Context,
) -> Result<Option<(String, Map, Duration)>> {
    match scour(msg).await? {
        None => Ok(None),
        Some((Err(e), _)) => {
            msg.reply(c, string(e)).await?;
            Ok(None)
        }
        Some((Ok(m), deser_took)) => Ok(Some((
            msg.author_nick(c).await.unwrap_or(msg.author.name.clone()),
            m,
            deser_took,
        ))),
    }
}

pub async fn with(msg: &Message, c: &serenity::client::Context) -> Result<()> {
    let Some((_auth, m, deser_took)) = find(msg, c).await? else {
        return Ok(());
    };
    let t = msg.channel_id.start_typing(&c.http);
    let (png, embed) = embed(m, deser_took).await;
    t.stop();
    msg.channel_id
        .send_message(c, CreateMessage::new().add_file(png).embed(embed))
        .await?;
    Ok(())
}

async fn embed(m: Map, deser_took: Duration) -> (CreateAttachment, CreateEmbed) {
    let name = strip_colors(m.tags.get("name").or(m.tags.get("mapname")).unwrap());
    let d = strip_colors(m.tags.get("description").map(|x| &**x).unwrap_or("?"));
    let f = if m.width == m.height {
        format!("{}²", m.width)
    } else {
        format!("{}×{}", m.height, m.width)
    };
    let (timings, png) = render(m, deser_took).await;
    (
        CreateAttachment::bytes(png, "map.png"),
        CreateEmbed::new()
            .title(&name)
            .description(d)
            .footer(CreateEmbedFooter::new(format!(
                "render of {name} ({f}) took: {:.3}s",
                timings.total.as_secs_f64()
            )))
            .attachment("map.png")
            .color(SUCCESS),
    )
}

#[poise::command(
    context_menu_command = "Render map",
    install_context = "User",
    interaction_context = "Guild|PrivateChannel"
)]
/// Renders map inside a message.
pub async fn render_message(c: super::Context<'_>, m: Message) -> Result<()> {
    let Some((_auth, m, deser_took)) = find(&m, c.serenity_context()).await? else {
        poise::say_reply(c, "no map").await?;
        return Ok(());
    };
    let (png, embed) = embed(m, deser_took).await;
    poise::send_reply(c, CreateReply::default().attachment(png).embed(embed)).await?;
    Ok(())
}
