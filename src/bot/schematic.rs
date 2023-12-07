use anyhow::{anyhow, Result};
use mindus::data::DataRead;
use mindus::*;
use poise::serenity_prelude::*;
use regex::Regex;
use std::fmt::Write;
use std::sync::LazyLock;
use std::{borrow::Cow, ops::ControlFlow};

use super::{strip_colors, Msg, SUCCESS};

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(```)?(\n)?([^`]+)(\n)?(```)?").unwrap());

async fn from_attachments(attchments: &[Attachment]) -> Result<Option<Schematic>> {
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
            let Ok(s) = Schematic::deserialize_base64(&s) else {
                println!("failed to read msg.txt");
                continue;
            };
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub async fn with(m: Msg, c: &serenity::client::Context) -> Result<ControlFlow<Message, ()>> {
    let author = m.author;
    let send = |v: Schematic| async move {
        let d = v.tags.get("description").map(|t| emoji::mindustry::to_discord(t));
        let name = emoji::mindustry::to_discord(&strip_colors(v.tags.get("name").unwrap()));
        let cost = v.compute_total_cost().0;
        println!("deser {name}");
        let p = tokio::task::spawn_blocking(move || to_png(&v)).await?;
        println!("rend {name}");
        anyhow::Ok(
            m.channel
                .send_message(c, |m| {
                    m.add_file(AttachmentType::Bytes {
                        data: Cow::Owned(p),
                        filename: "image.png".to_string(),
                    })
                    .embed(|e| {
                        e.attachment("image.png");
                        d.map(|v| e.description(v));
                        let mut s = String::new();
                        for (i, n) in cost.iter() {
                            if n == 0 {
                                continue;
                            }
                            write!(s, "{} {n} ", emoji::mindustry::item(i)).unwrap();
                        }
                        e.field("req", s, true);
                        e.title(name)
                            .footer(|f| f.text(format!("requested by {author}")))
                            .color(SUCCESS)
                    })
                })
                .await?,
        )
    };

    if let Ok(Some(v)) = from_attachments(&m.attachments).await {
        return Ok(ControlFlow::Break(send(v).await?));
    }
    if let Ok(v) = from_msg(&m.content) {
        return Ok(ControlFlow::Break(send(v).await?));
    }
    Ok(ControlFlow::Continue(()))
}

pub fn to_png(s: &Schematic) -> Vec<u8> {
    super::png(s.render())
}

fn from_msg(msg: &str) -> Result<Schematic> {
    let schem_text = RE
        .captures(msg)
        .ok_or(anyhow!("couldnt find schematic"))?
        .get(3)
        .unwrap()
        .as_str();
    Ok(Schematic::deserialize_base64(schem_text)?)
}
