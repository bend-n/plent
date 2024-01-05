use anyhow::Result;
use mindus::data::DataRead;
use mindus::*;
use poise::serenity_prelude::*;
use regex::Regex;
use std::fmt::Write;
use std::ops::ControlFlow;
use std::sync::LazyLock;

use super::{strip_colors, Msg, SUCCESS};

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(```)?(\n)?([^`]+)(\n)?(```)?").unwrap());

pub async fn from_attachments(attchments: &[Attachment]) -> Result<Option<Schematic>> {
    for a in attchments {
        if a.filename.ends_with("msch") {
            let s = a.download().await?;
            let mut s = DataRead::new(&s);
            let Ok(s) = Schematic::deserialize(&mut s) else {
                println!("failed to read {}", a.filename);
                continue;
            };
            return Ok(Some(s));
        // discord uploads base64 as a file when its too long
        } else if a.filename == "message.txt" {
            let Ok(s) = String::from_utf8(a.download().await?) else {
                continue;
            };
            let Ok(s) = Schematic::deserialize_base64(s.trim()) else {
                println!("failed to read msg.txt");
                continue;
            };
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub async fn with(
    m: Msg,
    c: &serenity::client::Context,
) -> Result<ControlFlow<(Message, String, Schematic), ()>> {
    let author = m.author;
    let send = |v: Schematic| async move {
        let d = v
            .tags
            .get("description")
            .map(|t| emoji::mindustry::to_discord(&strip_colors(t)));
        let name = emoji::mindustry::to_discord(&strip_colors(v.tags.get("name").unwrap()));
        let cost = v.compute_total_cost().0;
        println!("deser {name}");
        let vclone = v.clone();
        let p = tokio::task::spawn_blocking(move || to_png(&vclone)).await?;
        println!("rend {name}");
        anyhow::Ok((
            m.channel
                .send_message(
                    c,
                    CreateMessage::new()
                        .add_file(CreateAttachment::bytes(p, "image.png"))
                        .embed({
                            let mut e = CreateEmbed::new().attachment("image.png");
                            if let Some(v) = d {
                                e = e.description(v);
                            }
                            let mut s = String::new();
                            for (i, n) in cost.iter() {
                                if n == 0 {
                                    continue;
                                }
                                write!(s, "{} {n} ", emoji::mindustry::item(i)).unwrap();
                            }
                            e.field("req", s, true)
                                .title(name.clone())
                                .footer(CreateEmbedFooter::new(format!("requested by {author}")))
                                .color(SUCCESS)
                        }),
                )
                .await?,
            name,
            v,
        ))
    };

    if let Ok(Some(v)) = from((&m.content, &m.attachments)).await {
        return Ok(ControlFlow::Break(send(v).await?));
    }

    Ok(ControlFlow::Continue(()))
}

pub fn to_png(s: &Schematic) -> Vec<u8> {
    super::png(s.render())
}

pub async fn from(m: (&str, &[Attachment])) -> Result<Option<Schematic>> {
    match from_msg(m.0) {
        x @ Ok(Some(_)) => x,
        _ => from_attachments(m.1).await,
    }
}

pub fn from_msg(msg: &str) -> Result<Option<Schematic>> {
    let schem_text = match RE.captures(msg) {
        None => return Ok(None),
        Some(x) => x,
    }
    .get(3)
    .unwrap()
    .as_str()
    .trim();
    Ok(Some(Schematic::deserialize_base64(schem_text)?))
}
