use anyhow::Result;
use mindus::{data::map::ReadError, *};
use poise::{CreateReply, serenity_prelude::*};
use std::{
    ops::ControlFlow,
    time::{Duration, Instant},
};
use tokio::task::JoinError;
const BENDN: ChannelId = ChannelId::new(1149866218057117747);
use super::{SUCCESS, strip_colors};

fn string((x, f): (ReadError, &str)) -> String {
    match x {
        ReadError::Decompress(_) | ReadError::Header(_) => {
            format!("not a map.")
        }
        ReadError::NoBlockFound(b) => {
        	format!("couldnt find block `{b}`. mods are not supported")
        }
        ReadError::NoSuchBlock(b) => {
            format!("couldnt find block at index `{b}`. mods are not supported")
        }
        ReadError::Version(v) => {
            format!(
                "unsupported version: `{v}`. supported versions: `7, 8`.",
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

pub async fn download(
    a: &Attachment,
) -> Result<(Result<(Map, Box<[u8]>), (ReadError, &str)>, Duration)> {
    let s = a.download().await?.into_boxed_slice();
    let then = Instant::now();

    // could ignore, but i think if you have a msav, you dont want to ignore failures.
    Ok((
        Map::deserialize(&mut mindus::data::DataRead::new(&s))
            .map_err(|x| (x, &*a.filename))
            .map(|x| (x, s)),
        then.elapsed(),
    ))
}

pub async fn scour(
    m: &Message,
) -> Result<Option<(Result<(Map, Box<[u8]>), (ReadError, &str)>, Duration)>> {
    for a in &m.attachments {
        if a.filename.ends_with("msav") {
            return Ok(Some(download(a).await?));
        }
    }
    Ok(None)
}

pub async fn reply(
    c: super::Context<'_>,
    a: &Attachment,
) -> Result<ControlFlow<CreateReply, String>> {
    let ((m, b), deser_took) = match download(a).await? {
        (Err(e), _) => return Ok(ControlFlow::Continue(string(e))),
        (Ok(m), deser_took) => (m, deser_took),
    };
    let (a, e) = match embed(m, deser_took).await {
        Ok(x) => x,
        Err(e) => {
            BENDN.send_files(
                &c,
                [CreateAttachment::bytes(b, "map.msav")],
                CreateMessage::new().content(format!("<@696196765564534825>　failure: {e}")),
            );
            return Ok(ControlFlow::Break(CreateReply::default().content(
                "there was a problem. i have notified bendn about this issue.",
            )));
        }
    };
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
fn render(m: Map, deser_took: Duration) -> (Timings, Vec<u8>) {
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
}

pub async fn find(
    msg: &Message,
    c: &serenity::client::Context,
) -> Result<Option<(String, (Map, Box<[u8]>), Duration)>> {
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

#[implicit_fn::implicit_fn]
pub async fn with(msg: &Message, c: &serenity::client::Context) -> Result<()> {
    let Some((_auth, (m, b), deser_took)) = find(msg, c).await? else {
        return Ok(());
    };
    let t = msg.channel_id.start_typing(&c.http);
    let (png, embed) = match embed(m, deser_took).await {
        Ok(x) => x,
        Err(e) => {
            use crate::emoji::named::*;

            BENDN
                .send_files(
                    &c,
                    [CreateAttachment::bytes(b, "file.msch")],
                    CreateMessage::new().content(format!(
                        "<@696196765564534825> panic `{e}` in {}\n",
                        msg.guild(&c.cache).map_or(
                            msg.guild_id
                                .map(_.get().to_string())
                                .unwrap_or(format!("dms with {}", msg.author.name)),
                            _.name.clone(),
                        )
                    )),
                )
                .await?;
            msg.reply(c, format!("{CANCEL} there was an error while rendering this map.\nthis issue has been reported to bendn, who will hopefully take a look eventually."))
                .await?;
            return Ok(());
        }
    };
    t.stop();
    super::data::push_j(serde_json::json! {{
    "locale": msg.author.locale.as_deref().unwrap_or("no locale"),
    "name":  msg.author.name,
    "id": msg.author.id,
    "cname": "map message input",
    "guild": msg.guild_id.map_or(0,|x|x.get()),
    "channel": msg.channel_id.get(),
    }});
    msg.channel_id
        .send_message(c, CreateMessage::new().add_file(png).embed(embed))
        .await?;
    Ok(())
}

async fn embed(m: Map, deser_took: Duration) -> Result<(CreateAttachment, CreateEmbed), JoinError> {
    let name = strip_colors(m.tags.get("name").or(m.tags.get("mapname")).unwrap());
    let d = strip_colors(m.tags.get("description").map(|x| &**x).unwrap_or("?"));
    let f = if m.width == m.height {
        format!("{}²", m.width)
    } else {
        format!("{}×{}", m.height, m.width)
    };
    let (timings, png) = tokio::task::spawn_blocking(move || render(m, deser_took)).await?;
    Ok((
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
    ))
}

#[poise::command(
    context_menu_command = "Render map",
    install_context = "User",
    interaction_context = "Guild|PrivateChannel"
)]
/// Renders map inside a message.
pub async fn render_message(c: super::Context<'_>, m: Message) -> Result<()> {
    super::log(&c);
    let Some((_auth, (m, b), deser_took)) = find(&m, c.serenity_context()).await? else {
        poise::say_reply(c, "no map").await?;
        return Ok(());
    };
    let (png, embed) = match embed(m, deser_took).await {
        Ok(x) => x,
        Err(e) => {
            BENDN.send_files(
                &c,
                [CreateAttachment::bytes(b, "map.msav")],
                CreateMessage::new().content(format!("<@696196765564534825>　failure: {e}")),
            );
            c.say("there was a problem. i have notified bendn about this issue.")
                .await?;
            return Ok(());
        }
    };

    poise::send_reply(c, CreateReply::default().attachment(png).embed(embed)).await?;
    Ok(())
}
