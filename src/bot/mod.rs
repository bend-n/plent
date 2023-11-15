mod logic;
mod schematic;

use anyhow::Result;
use dashmap::DashMap;

use poise::serenity_prelude::*;
use serenity::model::channel::Message;
use std::fmt::Write;
use std::fs::read_to_string;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub struct Data {
    // message -> resp
    tracker: Arc<DashMap<MessageId, Message>>,
}

pub struct Msg {
    author: String,
    content: String,
    channel: ChannelId,
    attachments: Vec<Attachment>,
}

#[macro_export]
macro_rules! send {
    ($e:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        $e.send(format!($fmt $(, $args)*))
    };
}

const SUCCESS: (u8, u8, u8) = (34, 139, 34);

const PFX: char = '}';

pub struct Bot;
impl Bot {
    pub async fn spawn() {
        println!("bot startup");
        let tok = std::env::var("TOKEN").unwrap_or(read_to_string("token").expect("wher token"));
        let f: poise::FrameworkBuilder<Data, anyhow::Error> = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![logic::run(), help()],
                event_handler: |c, e, _, d| {
                    Box::pin(async move {
                        match e {
                            poise::Event::Ready { .. } => {
                                println!("bot ready");
                            }
                            poise::Event::Message { new_message } => {
                                if new_message.content.starts_with('!')
                                    || new_message.content.starts_with(PFX)
                                    || new_message.author.bot
                                {
                                    return Ok(());
                                }
                                if let ControlFlow::Break(m) = schematic::with(
                                    Msg {
                                        author: new_message
                                            .author_nick(c)
                                            .await
                                            .unwrap_or(new_message.author.name.clone()),
                                        attachments: new_message.attachments.clone(),
                                        content: new_message.content.clone(),
                                        channel: new_message.channel_id,
                                    },
                                    c,
                                )
                                .await?
                                {
                                    d.tracker.insert(new_message.id, m);
                                    return Ok(());
                                }
                            }
                            poise::Event::MessageUpdate { event, .. } => {
                                let MessageUpdateEvent {
                                    author: Some(author),
                                    guild_id: Some(guild_id),
                                    content: Some(content),
                                    attachments: Some(attachments),
                                    ..
                                } = event.clone()
                                else {
                                    return Ok(());
                                };
                                if let Some((_, r)) = d.tracker.remove(&event.id) {
                                    r.delete(c).await.unwrap();
                                    if let ControlFlow::Break(m) = schematic::with(
                                        Msg {
                                            author: author
                                                .nick_in(c, guild_id)
                                                .await
                                                .unwrap_or(author.name.clone()),
                                            content,
                                            attachments,
                                            channel: event.channel_id,
                                        },
                                        c,
                                    )
                                    .await?
                                    {
                                        d.tracker.insert(event.id, m);
                                    }
                                }
                            }
                            poise::Event::MessageDelete {
                                deleted_message_id, ..
                            } => {
                                if let Some((_, r)) = d.tracker.remove(deleted_message_id) {
                                    r.delete(c).await.unwrap();
                                }
                            }
                            _ => {}
                        };
                        Ok(())
                    })
                },
                on_error: |e| Box::pin(on_error(e)),
                prefix_options: poise::PrefixFrameworkOptions {
                    edit_tracker: Some(poise::EditTracker::for_timespan(
                        std::time::Duration::from_secs(2 * 60),
                    )),
                    prefix: Some(PFX.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            })
            .token(tok)
            .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
            .setup(|ctx, _ready, framework| {
                Box::pin(async move {
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    println!("registered");
                    let tracker = Arc::new(DashMap::new());
                    let tc = Arc::clone(&tracker);
                    tokio::spawn(async move {
                        loop {
                            // every 10 minutes
                            tokio::time::sleep(Duration::from_secs(60 * 10)).await;
                            tc.retain(|_, v: &mut Message| {
                                // prune messagees older than 3 hours
                                Timestamp::now().unix_timestamp() - v.timestamp.unix_timestamp()
                                    < 60 * 60 * 3
                            });
                        }
                    });
                    Ok(Data { tracker })
                    // todo: voting::fixall() auto
                })
            });
        f.run().await.unwrap();
    }
}

type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, anyhow::Error>) {
    use poise::FrameworkError::Command;
    match error {
        Command { error, ctx } => {
            let mut msg;
            {
                let mut chain = error.chain();
                msg = format!("e: `{}`", chain.next().unwrap());
                for mut source in chain {
                    write!(msg, "from: `{source}`").unwrap();
                    while let Some(next) = source.source() {
                        write!(msg, "from: `{next}`").unwrap();
                        source = next;
                    }
                }
            }
            let bt = error.backtrace();
            if bt.status() == std::backtrace::BacktraceStatus::Captured {
                let parsed = btparse::deserialize(dbg!(error.backtrace())).unwrap();
                let mut s = vec![];
                for frame in parsed.frames {
                    if let Some(line) = frame.line
                        && (frame.function.contains("panel")
                            || frame.function.contains("poise")
                            || frame.function.contains("serenity")
                            || frame.function.contains("mindus")
                            || frame.function.contains("image"))
                    {
                        s.push(format!("l{}@{}", line, frame.function));
                    }
                }
                s.truncate(15);
                write!(msg, "trace: ```rs\n{}\n```", s.join("\n")).unwrap();
            }
            ctx.say(msg).await.unwrap();
        }
        err => poise::builtins::on_error(err).await.unwrap(),
    }
}

pub fn strip_colors(from: &str) -> String {
    let mut result = String::new();
    result.reserve(from.len());
    let mut level: u8 = 0;
    for c in from.chars() {
        if c == '[' {
            level += 1;
        } else if c == ']' {
            level -= 1;
        } else if level == 0 {
            result.push(c);
        }
    }
    result
}

#[poise::command(slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<()> {
    ctx.send(|m| {
        m.ephemeral(true).content(
            if matches!(
                command.as_deref(),
                Some("eval") | Some("exec") | Some("run")
            ) {
                include_str!("help_eval.md")
            } else {
                include_str!("usage.md")
            },
        )
    })
    .await?;
    Ok(())
}

pub fn png(p: fimg::Image<Vec<u8>, 3>) -> Vec<u8> {
    use oxipng::*;
    let p = RawImage::new(
        p.width(),
        p.height(),
        ColorType::RGB {
            transparent_color: None,
        },
        BitDepth::Eight,
        p.take_buffer(),
    )
    .unwrap();
    p.create_optimized_png(&oxipng::Options {
        filter: indexset! { RowFilter::None },
        bit_depth_reduction: false,
        color_type_reduction: false,
        palette_reduction: false,
        grayscale_reduction: false,
        ..oxipng::Options::from_preset(0)
    })
    .unwrap()
}
