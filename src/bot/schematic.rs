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
        let name = emoji::mindustry::to_discord(&strip_colors(v.tags.get("name").unwrap()));
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
                            let mut e = CreateEmbed::new()
                                .attachment("image.png")
                                .author(CreateEmbedAuthor::new(author).icon_url(m.avatar));
                            if let Some(tags) = v.tags.get("labels") {
                                // yes, this is incorrect. no, i dont care if your tag is `\u{208} useful tag`.
                                static RE: LazyLock<Regex> =
                                    LazyLock::new(|| Regex::new(r"\\u\{([a-f0-9]+)\}").unwrap());
                                let mut yes = tags.clone();
                                for c in RE.captures_iter(&tags) {
                                    if let Ok(Some(y)) =
                                        u32::from_str_radix(c.get(1).unwrap().as_str(), 16)
                                            .map(char::from_u32)
                                    {
                                        yes = yes.replace(
                                            c.get(0).unwrap().as_str(),
                                            y.encode_utf8(&mut [0; 4]),
                                        );
                                    }
                                }
                                let mut s = yes[1..].as_bytes();
                                let mut o = vec![];
                                let mut t = Vec::new();

                                while s.len() > 0 {
                                    loop {
                                        let b = s[0];
                                        s = &s[1..];
                                        match b {
                                            b',' | b']' => {
                                                o.push(emoji::mindustry::to_discord(
                                                    std::str::from_utf8(&t)
                                                        .unwrap()
                                                        .trim()
                                                        .trim_start_matches('"')
                                                        .trim_end_matches('"'),
                                                ));
                                                break;
                                            }
                                            b => t.push(b),
                                        }
                                    }
                                    t.clear();
                                }
                                e = e.field(
                                    "tags",
                                    o.iter()
                                        .map(String::as_str)
                                        .intersperse(" | ")
                                        .fold(String::new(), |acc, x| acc + x),
                                    true,
                                );
                            }
                            if let Some(v) = v
                                .tags
                                .get("description")
                                .map(|t| emoji::mindustry::to_discord(&strip_colors(t)))
                            {
                                e = e.description(v);
                            }
                            let mut s = String::new();
                            for (i, n) in v.compute_total_cost().0.iter() {
                                if n == 0 {
                                    continue;
                                }
                                write!(s, "{} {n} ", emoji::mindustry::item(i)).unwrap();
                            }
                            e.field("req", s, true).title(name.clone()).color(SUCCESS)
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
