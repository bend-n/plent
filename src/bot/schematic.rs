use crate::bot::repos;
use crate::emoji;
use anyhow::Result;
use base64::Engine;
use logos::Logos;
use mindus::data::DataRead;
use mindus::data::schematic::R64Error;
use mindus::*;
use poise::{CreateReply, serenity_prelude::*};
use regex::Regex;
use std::ops::ControlFlow;
use std::sync::LazyLock;
use std::{fmt::Write, ops::Deref};

use super::{Msg, SUCCESS, strip_colors};

static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?").unwrap()
});

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
            let schem = Schematic::deserialize_base64(&s)?;
            return Ok(Some(Schem { schem }));
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
        .embed(e(author, avatar, &v).title(name)))
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

fn e(author: &str, avatar: &str, v: &Schem) -> CreateEmbed {
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
    let f = if v.width == v.height {
        format!("{}²={}", v.width, v.width * v.height)
    } else {
        format!("{}×{}={}", v.height, v.width, v.width * v.height)
    };
    e.field("req", cost(&v), true)
        .footer(CreateEmbedFooter::new(f))
        .color(SUCCESS)
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
        .embed(e(&m.author, &m.avatar, &v).title(name.clone()));
    let h = m.channel.send_message(c, msg).await?;
    Ok((h, name, v))
}

pub async fn with(
    m: Msg,
    c: &serenity::client::Context,
    labels: Option<super::Type>,
) -> Result<ControlFlow<(Message, String, Schem), ()>> {
    if let Ok(Some(mut v)) = from((&m.content, &m.attachments)).await {
        super::data::push_j(serde_json::json! {{
        "locale": m.locale,
        "name":  m.author,
        "id": m.author_id,
        "cname": "schematic message input",
        "guild":  m.guild,
        "channel": m.channel.get(),
        }});
        if let Some(super::Type::Basic(x)) = labels {
            use emoji::to_mindustry::named::*;
            let x = if let Some(i) = x.iter().position(|x| x == &repos::L) {
                let mut x = x.to_vec();
                if v.block_iter().any(|x| {
                    x.1.block == &mindus::block::ADVANCED_LAUNCH_PAD
                        || x.1.block == &mindus::block::LAUNCH_PAD
                }) {
                    x[i] = ADVANCED_LAUNCH_PAD;
                } else {
                    x.remove(i);
                }
                super::tags(&x)
            } else if x.contains(&"find unit factory") {
                super::tags(&[v
                    .block_iter()
                    .find_map(|x| match x.1.block.name() {
                        "air-factory" => Some(AIR_FACTORY),
                        "ground-factory" => Some(GROUND_FACTORY),
                        "naval-factory" => Some(NAVAL_FACTORY),
                        _ => None,
                    })
                    .unwrap_or(AIR_FACTORY)])
            } else {
                super::tags(x)
            };
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
        x @ Ok(Some(_)) => Ok(x?),
        // .or
        _ => from_attachments(m.1).await,
    }
}

pub fn from_msg(msg: &str) -> Result<Option<Schem>, R64Error> {
    RE.captures_iter(msg)
        .map(|x| x.get(0).unwrap().as_str())
        .find(|x| x.starts_with("bXNjaA"))
        .map(from_b64)
        .transpose()
}

pub fn from_b64(schem_text: &str) -> std::result::Result<Schem, R64Error> {
    Schematic::deserialize_base64(schem_text).map(|schem| Schem { schem })
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
        let x = match x.trim() {
            super::repos::SRP => "<:serpulo:1395767515950612593>",
            super::repos::ERE => "<:erekir:1395767762957369484>",
            _ => x,
        };
        t.push(emoji::mindustry::to_discord(x));
    }
    assert_eq!(lexer.next(), None);
    t
}
