use crate::emoji;
use anyhow::Result;
use base64::Engine;
use logos::Logos;
use mindus::data::DataRead;
use mindus::*;
use poise::{serenity_prelude::*, CreateReply};
use regex::Regex;
use std::ops::ControlFlow;
use std::sync::LazyLock;
use std::{fmt::Write, ops::Deref};

use super::{strip_colors, Msg, SUCCESS};

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(```)?(\n)?([^`]+)(\n)?(```)?").unwrap());

pub struct Schem {
    pub schem: Schematic,
}

impl Deref for Schem {
    type Target = Schematic;

    fn deref(&self) -> &Self::Target {
        &self.schem
    }
}

pub async fn from_attachments(attchments: &[Attachment]) -> Result<Option<Schem>> {
    for a in attchments {
        if a.filename.ends_with("msch") {
            let sd = a.download().await?;
            let mut s = DataRead::new(&sd);
            let Ok(s) = Schematic::deserialize(&mut s) else {
                println!("failed to read {}", a.filename);
                continue;
            };
            return Ok(Some(Schem { schem: s }));
        // discord uploads base64 as a file when its too long
        } else if a.filename == "message.txt" {
            let Ok(s) = String::from_utf8(a.download().await?) else {
                continue;
            };
            let mut buff = vec![0; s.len() / 4 * 3 + 1];
            let Ok(s) = base64::engine::general_purpose::STANDARD
                .decode_slice(s.as_bytes(), &mut buff)
                .map_err(|_| ())
                .and_then(|n_out| {
                    buff.truncate(n_out);
                    Schematic::deserialize(&mut DataRead::new(&buff)).map_err(|_| ())
                })
            else {
                println!("failed to read msg.txt");
                continue;
            };
            return Ok(Some(Schem { schem: s }));
        }
    }
    Ok(None)
}

pub async fn reply(v: Schem, author: &str, avatar: &str) -> Result<CreateReply> {
    let name = emoji::mindustry::to_discord(&strip_colors(v.tags.get("name").unwrap()));
    let vclone = v.clone();
    let p = tokio::task::spawn_blocking(move || to_png(&vclone)).await?;
    println!("rend {name}");
    Ok(CreateReply::default()
        .attachment(CreateAttachment::bytes(p, "image.png"))
        .embed({
            let mut e = CreateEmbed::new()
                .attachment("image.png")
                .author(CreateEmbedAuthor::new(author).icon_url(avatar));
            if let Some(tags) = tags(&v) {
                e = e.field("tags", tags, true);
            }
            if let Some(v) = v
                .tags
                .get("description")
                .map(|t| emoji::mindustry::to_discord(&strip_colors(t)))
            {
                e = e.description(v);
            }
            e.field("req", cost(&v), true)
                .title(name.clone())
                .color(SUCCESS)
        }))
}

fn tags(v: &Schem) -> Option<String> {
    v.tags.get("labels").map(|tags| {
        decode_tags(tags)
            .iter()
            .map(String::as_str)
            .intersperse(" | ")
            .fold(String::new(), |acc, x| acc + x)
    })
}

fn cost(v: &Schem) -> String {
    let mut s = String::new();
    for (i, n) in v.compute_total_cost().0.iter() {
        if n == 0 {
            continue;
        }
        write!(s, "{} {n} ", emoji::mindustry::item(i)).unwrap();
    }
    s
}

pub async fn send(
    m: Msg,
    c: &serenity::client::Context,
    v: Schem,
) -> Result<(poise::serenity_prelude::Message, std::string::String, Schem)> {
    let name = emoji::mindustry::to_discord(&strip_colors(v.tags.get("name").unwrap()));
    println!("deser {name}");
    let vclone = v.clone();
    let p = tokio::task::spawn_blocking(move || to_png(&vclone)).await?;
    println!("rend {name}");
    let msg = CreateMessage::new()
        .add_file(CreateAttachment::bytes(p, "image.png"))
        .embed({
            let mut e = CreateEmbed::new()
                .attachment("image.png")
                .author(CreateEmbedAuthor::new(m.author).icon_url(m.avatar));
            if let Some(tags) = tags(&v) {
                e = e.field("tags", tags, true);
            }
            if let Some(v) = v
                .tags
                .get("description")
                .map(|t| emoji::mindustry::to_discord(&strip_colors(t)))
            {
                e = e.description(v);
            }
            e.field("req", cost(&v), true)
                .title(name.clone())
                .color(SUCCESS)
        });
    let h = m.channel.send_message(c, msg).await?;
    Ok((h, name, v))
}

pub async fn with(
    m: Msg,
    c: &serenity::client::Context,
    labels: Option<String>,
) -> Result<ControlFlow<(Message, String, Schem), ()>> {
    if let Ok(Some(mut v)) = from((&m.content, &m.attachments)).await {
        if let Some(x) = labels {
            v.schem.tags.insert("labels".into(), x);
        };
        return Ok(ControlFlow::Break(send(m, c, v).await?));
    }

    Ok(ControlFlow::Continue(()))
}

pub fn to_png(s: &Schematic) -> Vec<u8> {
    super::png(s.render())
}

pub async fn from(m: (&str, &[Attachment])) -> Result<Option<Schem>> {
    match from_msg(m.0) {
        x @ Ok(Some(_)) => x,
        _ => from_attachments(m.1).await,
    }
}

pub fn from_msg(msg: &str) -> Result<Option<Schem>> {
    let schem_text = match RE.captures(msg) {
        None => return Ok(None),
        Some(x) => x,
    }
    .get(3)
    .unwrap()
    .as_str()
    .trim();
    Ok(Some(from_b64(schem_text)?))
}

pub fn from_b64(schem_text: &str) -> Result<Schem> {
    let mut buff = vec![0; schem_text.len() / 4 * 3 + 1];
    let s = base64::engine::general_purpose::STANDARD
        .decode_slice(schem_text.as_bytes(), &mut buff)
        .map_err(anyhow::Error::from)
        .and_then(|n_out| {
            buff.truncate(n_out);
            Schematic::deserialize(&mut DataRead::new(&buff)).map_err(anyhow::Error::from)
        })?;
    Ok(Schem { schem: s })
}

fn decode_tags(tags: &str) -> Vec<String> {
    #[derive(logos::Logos, PartialEq, Debug)]
    #[logos(skip r"[\s\n,]+")]
    enum Tokens<'s> {
        #[token("[", priority = 8)]
        Open,
        #[token("]", priority = 8)]
        Close,
        #[regex(r#""[^"]+""#, priority = 7, callback = |x| &x.slice()[1..x.slice().len()-1])]
        #[regex(r"[^,\]\[]+", priority = 6)]
        String(&'s str),
    }
    let mut lexer = Tokens::lexer(tags);
    let mut t = Vec::new();
    let mut next = || lexer.find_map(|x| x.ok());
    assert_eq!(next().unwrap(), Tokens::Open);
    while let Some(Tokens::String(x)) = next() {
        t.push(emoji::mindustry::to_discord(x));
    }
    assert_eq!(lexer.next(), None);
    t
}
