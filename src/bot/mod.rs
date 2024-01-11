mod logic;
mod map;
mod schematic;

use anyhow::Result;
use dashmap::DashMap;

use mindus::Serializable;
use poise::serenity_prelude::*;
use serenity::futures::StreamExt;
use serenity::model::channel::Message;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs::read_to_string;
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::Mutex;

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

const THREADED: phf::Set<u64> = phf::phf_set! {
    925721957209636914u64,
    925721791475904533u64,
    925721824556359720u64,
    925721863525646356u64,
    927036346869104693u64,
    925736419983515688u64,
    927793648417009676u64,
    925721763856404520u64,
    925721930814869524u64,
    925674713932521483u64,
    1141034314163826879u64,
    949529149800865862u64,
    925729855574794311u64,
    1185702384194818048u64,
    925720008313683969u64,
    1018541701431836803u64,
    927480650859184171u64,
    925719985987403776u64,
    949740875817287771u64,
    926163105694752811u64,
    973234467357458463u64,
    973236445567410186u64,
    1147887958351945738u64,
    1096157669112418454u64,
    973234248054104115u64,
    973422874734002216u64,
    973369188800413787u64,
    1147722735305367572u64,
    974450769967341568u64,
    973241041685737532u64,
    1158818171139133490u64,
    1158818324210274365u64,
    1158818598568075365u64,
    1142181013779398676u64,

    1129391545418797147u64,
};

const SPECIAL: phf::Map<u64, &str> = phf::phf_map! {
    925721957209636914u64 => "cryofluid",
    925721791475904533u64 => "graphite",
    925721824556359720u64 => "metaglass",
    925721863525646356u64 => "phase-fabric",
    927036346869104693u64 => "plastanium",
    925736419983515688u64 => "pyratite",
    927793648417009676u64 => "scrap",
    925721763856404520u64 => "silicon",
    925721930814869524u64 => "surge-alloy",
    1141034314163826879u64 => "defensive-outpost",
    949529149800865862u64 => "drills",
    925729855574794311u64 => "logic-schems",
    1185702384194818048u64 => "miscellaneous",
    925720008313683969u64 => "units",
    1018541701431836803u64 => "combustion-gen",
    927480650859184171u64 => "differential-gen",
    925719985987403776u64 => "impact-reactor",
    949740875817287771u64 => "steam-gen",
    926163105694752811u64 => "thorium-reactor",
    973234467357458463u64 => "carbide",
    973236445567410186u64 => "fissile-matter",
    1147887958351945738u64 => "liquid",
    1096157669112418454u64 => "mass-driver",
    973234248054104115u64 => "oxide",
    973422874734002216u64 => "erekir-phase",
    973369188800413787u64 => "power",
    1147722735305367572u64 => "silicon-arc",
    974450769967341568u64 => "erekir-surge",
    973241041685737532u64 => "erekir-units",
    1158818171139133490u64 => "unit-core",
    1158818324210274365u64 => "unit-delivery",
    1158818598568075365u64 => "unit-raw",
    1142181013779398676u64 => "unit-sand",
};

#[poise::command(slash_command)]
pub async fn scour(c: Context<'_>) -> Result<()> {
    let h = c.say("beginning scour, this may take a bit.").await?;
    let mut n = 0;
    for (&k, &d) in &SPECIAL {
        _ = std::fs::create_dir(format!("repo/{d}"));
        h.edit(
            c,
            poise::CreateReply::default().content(format!("scouring {d}...")),
        )
        .await?;
        let mut msgs = ChannelId::new(k).messages_iter(c).boxed();
        while let Some(msg) = msgs.next().await {
            let Ok(msg) = msg else {
                continue;
            };
            if let Ok(Some(x)) = schematic::from((&msg.content, &msg.attachments)).await {
                let mut w = mindus::data::DataWrite::default();
                x.serialize(&mut w).unwrap();
                _ = std::fs::write(format!("repo/{d}/{:x}.msch", msg.id.get()), w.consume());
                msg.react(c, emojis::get!(MERGE)).await?;
                n += 1;
            }
        }
    }
    h.edit(
        c,
        poise::CreateReply::default()
            .content(format!("done! <:merge:1192387272046284800> {n} schems")),
    )
    .await?;
    Ok(())
}

pub struct Bot;
impl Bot {
    pub async fn spawn() {
        println!("bot startup");
        let tok =
            std::env::var("TOKEN").unwrap_or_else(|_| read_to_string("token").expect("wher token"));
        let f: poise::Framework<Data, anyhow::Error> = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![logic::run(), help()],
                event_handler: |c, e, _, d| {
                    Box::pin(async move {
                        match e {
                            FullEvent::Ready { .. } => {
                                println!("bot ready");
                                emojis::load(c.http())
                                .await;
                            }
                            // :deny:, @vd
                            FullEvent::ReactionAdd { add_reaction: Reaction { message_id, emoji: ReactionType::Custom {  id,.. } ,channel_id,member: Some(Member{roles,nick,user,..}),..}} if *id == 1192388789952319499 && let Some(dir) = SPECIAL.get(&channel_id.get()) && roles.contains(&RoleId::new(925676016708489227)) => {
                                let m = c.http().get_message(*channel_id,* message_id).await?;
                                if Path::new("repo").join(dir).join(format!("{:x}.msch",message_id.get())).exists() {
                                    assert!(std::process::Command::new("git").current_dir("repo").arg("rm").arg("-q").arg(Path::new(dir).join(format!("{:x}.msch",message_id.get()))).status().unwrap().success());
                                    assert!(std::process::Command::new("git").current_dir("repo").args(["commit", "-q", "--author"]).arg(format!("{} <@designit>", nick.as_deref().unwrap_or(&user.name))).arg("-m").arg(format!("remove {:x}.msch", message_id.get())).status().unwrap().success());
                                    assert!(std::process::Command::new("git").current_dir("repo").arg("push").arg("-q").status().unwrap().success());
                                    _ = m.delete_reaction(c,Some(1174262682573082644.into()), emojis::get!(MERGE)).await;
                                    _ = m.delete_reaction(c,Some(1174262682573082644.into()), ReactionType::Custom { animated: false, id: 1192316518395039864.into(), name: Some("merge".into()) }).await.unwrap();
                                    m.react(c,emojis::get!(DENY)).await?;
                                };
                            }
                            FullEvent::GuildCreate { guild ,..} => {
                                static SEEN: LazyLock<Mutex<HashSet<GuildId>>> =
                                    LazyLock::new(|| Mutex::new(HashSet::new()));
                                if SEEN.lock().await.insert(guild.id) {
                                    let Guild {
                                        member_count,
                                        name,
                                        owner_id,
                                        ..
                                    } = guild;
                                    let User{id,name:owner_name,..} = c.http().get_user(*owner_id).await.unwrap();
                                    c.http()
                                        .get_user(696196765564534825.into())
                                        .await
                                        .unwrap()
                                        .dm(c.http(),  CreateMessage::new().allowed_mentions(CreateAllowedMentions::default().empty_users()).content(format!(
                                            "{name} (owned by <@{id}>({owner_name})) has {member_count:?} members"
                                        )))
                                        .await
                                        .unwrap();
                                }
                            }
                            FullEvent::Message { new_message } => {
                                if new_message.content.starts_with('!')
                                    || new_message.content.starts_with(PFX)
                                    || new_message.author.bot
                                {
                                    return Ok(());
                                }
                                let who = new_message
                                .author_nick(c)
                                .await
                                .unwrap_or(new_message.author.name.clone());
                                let m = Msg {
                                    author: who.clone(),
                                    attachments: new_message.attachments.clone(),
                                    content: new_message.content.clone(),
                                    channel: new_message.channel_id,
                                };
                                if let ControlFlow::Break((m,n, s)) = schematic::with(m, c).await? {
                                    if THREADED.contains(&m.channel_id.get()) {
                                        m.channel_id.create_thread_from_message(c, m.id,CreateThread::new(n).audit_log_reason("because yes").auto_archive_duration(AutoArchiveDuration::OneDay)).await.unwrap();
                                    }
                                    if let Some(dir) = SPECIAL.get(&m.channel_id.get()) {
                                        // add :)
                                        let mut w = mindus::data::DataWrite::default();
                                        s.serialize(&mut w).unwrap();
                                        _ = std::fs::create_dir(format!("repo/{dir}"));
                                        std::fs::write(format!("repo/{dir}/{:x}.msch", new_message.id.get()), w.consume()).unwrap();
                                        assert!(std::process::Command::new("git").current_dir("repo").arg("add").arg(".").status().unwrap().success());
                                        assert!(std::process::Command::new("git").current_dir("repo").args(["commit", "-q", "--author"]).arg(format!("{who} <@designit>")).arg("-m").arg(format!("add {:x}.msch", new_message.id.get())).status().unwrap().success());
                                        assert!(std::process::Command::new("git").current_dir("repo").arg("push").arg("-q").status().unwrap().success());
                                        new_message.react(c, emojis::get!(MERGE)).await?;
                                    }
                                    d.tracker.insert(new_message.id, m);
                                    return Ok(());
                                }
                                // not tracked, as you cant add a attachment afterwwards.
                                map::with(new_message, c).await?;
                            }
                            FullEvent::MessageUpdate {event: MessageUpdateEvent {
                                author: Some(author),
                                guild_id: Some(guild_id),
                                content: Some(content),
                                attachments: Some(attachments),
                                id,
                                channel_id,
                                ..
                            }, ..} => {

                                if let Some((_, r)) = d.tracker.remove(id) {
                                    _ = r.delete(c).await;
                                    let who = author
                                    .nick_in(c, guild_id)
                                    .await
                                    .unwrap_or(author.name.clone());
                                    if let ControlFlow::Break((m,_,s)) = schematic::with(
                                        Msg {
                                            author: who.clone(),
                                            content:content.clone(),
                                            attachments:attachments.clone(),
                                            channel: *channel_id,
                                        },
                                        c,
                                    )
                                    .await?
                                    {
                                        d.tracker.insert(*id, m);
                                        if let Some(dir) = SPECIAL.get(&channel_id.get()) {
                                            if Path::new("repo").join(dir).join(format!("{:x}.msch",id.get())).exists() {   
                                                // update :)
                                                let mut w = mindus::data::DataWrite::default();
                                                s.serialize(&mut w).unwrap();
                                                std::fs::write(format!("repo/{dir}/{:x}.msch", id.get()), w.consume()).unwrap();
                                                assert!(std::process::Command::new("git").current_dir("repo").arg("add").arg(".").status().unwrap().success());
                                                assert!(std::process::Command::new("git").current_dir("repo").args(["commit", "-q", "--author"]).arg(format!("{who} <@designit>")).arg("-m").arg(format!("update {:x}.msch", id.get())).status().unwrap().success());
                                                assert!(std::process::Command::new("git").current_dir("repo").arg("push").arg("-q").status().unwrap().success());
                                            }
                                        }
                                    }
                                }
                            }
                            FullEvent::MessageDelete {
                                deleted_message_id, channel_id, ..
                            } => {
                                if let Some(dir) = SPECIAL.get(&channel_id.get()) {
                                    if Path::new("repo").join(dir).join(format!("{:x}.msch",deleted_message_id.get())).exists() {
                                        assert!(std::process::Command::new("git").current_dir("repo").arg("rm").arg("-q").arg(Path::new(dir).join(format!("{:x}.msch", deleted_message_id.get()))).status().unwrap().success());
                                        assert!(std::process::Command::new("git").current_dir("repo").args(["commit", "-q"]).arg("-m").arg(format!("remove {:x}.msch", deleted_message_id.get())).status().unwrap().success());
                                        assert!(std::process::Command::new("git").current_dir("repo").arg("push").arg("-q").status().unwrap().success());
                                    };
                                }

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
                    edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                        std::time::Duration::from_secs(2 * 60),
                    ))),
                    prefix: Some(PFX.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            })
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
                })
            }).build();
        ClientBuilder::new(
            tok,
            GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
        )
        .framework(f)
        .await
        .unwrap()
        .start()
        .await
        .unwrap();
    }
}

pub mod emojis {
    pub const GUILDS: &[u64] = &[1003092764919091282, 925674713429184564];
    use poise::serenity_prelude::*;
    use std::sync::OnceLock;

    macro_rules! create {
        ($($i: ident),+ $(,)?) => { paste::paste! {
            $(pub static $i: OnceLock<Emoji> = OnceLock::new();)+

            pub async fn load(c: &Http) {
                for &g in GUILDS {
                let all = c.get_emojis(g.into()).await.unwrap();
                for e in all {
                    match e.name.as_str() {
                        $(stringify!([< $i:lower >])=>{let _=$i.get_or_init(||e);},)+
                        _ => { /*println!("{n} unused");*/ }
                    }
                }
            }
                $(
                    $i.get().expect(&format!("{} should be loaded", stringify!($i)));
                )+
            }
        } };
    }
    create![MERGE, DENY];

    macro_rules! get {
        ($e: ident) => {
            crate::bot::emojis::$e.get().unwrap().clone()
        };
    }
    pub(crate) use get;
}

type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, anyhow::Error>) {
    use poise::FrameworkError::Command;
    match error {
        Command { error, ctx, .. } => {
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
                        && (frame.function.contains("plent")
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
    ctx.send(poise::CreateReply::default().ephemeral(true).content(
        if matches!(
            command.as_deref(),
            Some("eval") | Some("exec") | Some("run")
        ) {
            include_str!("help_eval.md")
        } else {
            include_str!("usage.md")
        },
    ))
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
