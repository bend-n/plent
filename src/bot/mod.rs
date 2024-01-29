mod logic;
mod map;
mod schematic;
mod search;

use anyhow::Result;
use dashmap::DashMap;
use emoji::named::*;
use mindus::Serializable;
use poise::serenity_prelude::*;
use serenity::futures::StreamExt;
use serenity::model::channel::Message;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs::read_to_string;
use std::ops::ControlFlow;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Data {
    // message -> resp
    tracker: Arc<DashMap<MessageId, Message>>,
}

pub struct Msg {
    avatar: String,
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
    1200301847387316317u64,
    925736573037838397u64,
    1141034314163826879u64,
    949529149800865862u64,
    925729855574794311u64,
    1185702384194818048u64,
    1198555991667646464u64,
    1198556531281637506u64,
    1198527267933007893u64,
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
    1200305956689547324u64,
    1200306409036857384u64,
    1200308292744921088u64,
    1200308146460180520u64,

    1129391545418797147u64,
};

const SPECIAL: phf::Map<u64, &str> = phf::phf_map! {
    925721957209636914u64 => "cryofluid",
    925721791475904533u64 => "graphite",
    925721824556359720u64 => "metaglass",
    925721863525646356u64 => "phase-fabric",
    927036346869104693u64 => "plastanium",
    925736419983515688u64 => "pyratite",
    925736573037838397u64 => "blast-compound",
    927793648417009676u64 => "scrap",
    1198556531281637506u64 => "spore-press",
    1200308146460180520u64 => "oil-extractor",
    1200301847387316317u64 => "rtg-gen",
    1200308292744921088u64 => "cultivator",
    1200305956689547324u64 => "graphite-multipress",
    1200306409036857384u64 => "silicon-crucible",
    1198555991667646464u64 => "coal",
    925721763856404520u64 => "silicon",
    925721930814869524u64 => "surge-alloy",
    1141034314163826879u64 => "defensive-outpost",
    949529149800865862u64 => "drills",
    925729855574794311u64 => "logic-schems",
    1185702384194818048u64 => "miscellaneous",
    1018541701431836803u64 => "combustion-gen",
    927480650859184171u64 => "differential-gen",
    925719985987403776u64 => "impact-reactor",
    949740875817287771u64 => "steam-gen",
    926163105694752811u64 => "thorium-reactor",
    973234467357458463u64 => "carbide",
    1198527267933007893u64 => "erekir-defensive-outpost",
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
pub async fn scour(c: Context<'_>, ch: ChannelId) -> Result<()> {
    let mut n = 0;
    let d = SPECIAL[&ch.get()];
    let h = c.say(format!("scouring {d}...")).await?;
    _ = std::fs::create_dir(format!("repo/{d}"));
    let mut msgs = ch.messages_iter(c).boxed();
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
    h.edit(
        c,
        poise::CreateReply::default().content(format!(
            "done scouring <#{ch}>! <:merge:1192387272046284800> {n} schems"
        )),
    )
    .await?;
    Ok(())
}

static HOOK: OnceLock<Webhook> = OnceLock::new();

pub async fn hookup(c: &impl AsRef<Http>) {
    let v = Webhook::from_url(c, {
        &std::env::var("WEBHOOK")
            .unwrap_or_else(|_| read_to_string("webhook").expect("wher webhook"))
    })
    .await
    .unwrap();
    HOOK.get_or_init(|| v);
}

async fn send<F>(c: &impl AsRef<Http>, block: F)
where
    for<'b> F: FnOnce(ExecuteWebhook) -> ExecuteWebhook,
{
    let execute_webhook = ExecuteWebhook::default();
    let execute_webhook = block(execute_webhook);
    if let Err(e) = HOOK
        .get()
        .unwrap()
        .execute(c.as_ref(), false, execute_webhook.clone())
        .await
    {
        println!("sending {execute_webhook:#?} got error {e}.");
    }
}

mod git {
    use super::*;
    pub fn schem(dir: &str, x: MessageId) -> std::io::Result<mindus::Schematic> {
        std::fs::read(path(dir, x))
            .map(|x| mindus::Schematic::deserialize(&mut mindus::data::DataRead::new(&x)).unwrap())
    }

    pub fn path(dir: &str, x: MessageId) -> std::path::PathBuf {
        Path::new("repo")
            .join(dir)
            .join(format!("{:x}.msch", x.get()))
    }

    pub fn gpath(dir: &str, x: MessageId) -> std::path::PathBuf {
        Path::new(dir).join(format!("{:x}.msch", x.get()))
    }

    pub fn has(dir: &str, x: MessageId) -> bool {
        path(dir, x).exists()
    }

    pub fn whos(dir: &str, x: MessageId) -> String {
        let mut dat = std::process::Command::new("git")
            .current_dir("repo")
            .arg("blame")
            .arg("--porcelain")
            .arg(gpath(dir, x))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
            .wait_with_output()
            .unwrap()
            .stdout;
        dat.drain(0..=dat.iter().position(|&x| x == b'\n').unwrap() + "author ".len());
        dat.truncate(dat.iter().position(|&x| x == b'\n').unwrap());
        String::from_utf8(dat).unwrap()
    }

    pub fn remove(dir: &str, x: MessageId) {
        assert!(std::process::Command::new("git")
            .current_dir("repo")
            .arg("rm")
            .arg("-q")
            .arg(gpath(dir, x))
            .status()
            .unwrap()
            .success());
    }

    pub fn commit(by: &str, msg: &str) {
        assert!(std::process::Command::new("git")
            .current_dir("repo")
            .args(["commit", "-q", "--author"])
            .arg(format!("{by} <@designit>",))
            .arg("-m")
            .arg(msg)
            .status()
            .unwrap()
            .success());
    }

    pub fn push() {
        assert!(std::process::Command::new("git")
            .current_dir("repo")
            .arg("push")
            .arg("-q")
            .status()
            .unwrap()
            .success())
    }

    pub fn write(dir: &str, x: MessageId, s: &mindus::Schematic) {
        _ = std::fs::create_dir(format!("repo/{dir}"));
        let mut w = mindus::data::DataWrite::default();
        s.serialize(&mut w).unwrap();
        std::fs::write(path(dir, x), w.consume()).unwrap();
        add();
    }

    pub fn add() {
        assert!(std::process::Command::new("git")
            .current_dir("repo")
            .arg("add")
            .arg(".")
            .status()
            .unwrap()
            .success());
    }
}

const RM: (u8, u8, u8) = (242, 121, 131);
const AD: (u8, u8, u8) = (128, 191, 255);
const CAT: &str =
    "https://cdn.discordapp.com/avatars/696196765564534825/6f3c605329ffb5cfb790343f59ed355d.webp";
pub struct Bot;
impl Bot {
    pub async fn spawn() {
        println!("bot startup");
        let tok =
            std::env::var("TOKEN").unwrap_or_else(|_| read_to_string("token").expect("wher token"));
        let f = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![logic::run(), help(), search::search() ,search::find(), search::file()],
                event_handler: |c, e, _, d| {
                    Box::pin(async move {
                        match e {
                            FullEvent::Ready { .. } => {
                                println!("bot ready");
                                emojis::load(c.http()).await;
                                hookup(c.http()).await;
                            }
                            // :deny:, @vd
                            FullEvent::ReactionAdd { add_reaction: Reaction { message_id, emoji: ReactionType::Custom {  id,.. } ,channel_id,member: Some(Member{roles,nick,user,..}),..}} if *id == 1192388789952319499 && let Some(dir) = SPECIAL.get(&channel_id.get()) && roles.contains(&RoleId::new(925676016708489227)) => {
                                let m = c.http().get_message(*channel_id,* message_id).await?;
                                if let Ok(s) = git::schem(dir,*message_id) {
                                    let who = nick.as_deref().unwrap_or(&user.name);
                                    let own = git::whos(dir,*message_id);
                                    git::remove(dir, *message_id);
                                    git::commit(who, &format!("remove {:x}.msch", message_id.get()));
                                    git::push();
                                    _ = m.delete_reaction(c,Some(1174262682573082644.into()), emojis::get!(MERGE)).await;
                                    _ = m.delete_reaction(c,Some(1174262682573082644.into()), ReactionType::Custom { animated: false, id: 1192316518395039864.into(), name: Some("merge".into()) }).await.unwrap();
                                    m.react(c,emojis::get!(DENY)).await?;
                                    send(c,|x| x
                                        .avatar_url(user.avatar_url().unwrap_or(CAT.to_string()))
                                        .username(who)
                                        .embed(CreateEmbed::new().color(RM)
                                            .description(format!("https://discord.com/channels/925674713429184564/{channel_id}/{message_id} {} {} (added by {own}) (`{:x}`)", emojis::get!(DENY), emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), message_id.get())))
                                    ).await;
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
                                    avatar: new_message. author.avatar_url().unwrap_or(CAT.to_string()),
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
                                        git::write(dir, new_message.id, &s);
                                        git::add();
                                        git::commit(&who, &format!("add {:x}.msch", new_message.id.get()));
                                        git::push();
                                        new_message.react(c, emojis::get!(MERGE)).await?;
                                        send(c,|x| x
                                            .avatar_url(new_message.author.avatar_url().unwrap_or(CAT.to_string()))
                                            .username(who)
                                            .embed(CreateEmbed::new().color(AD)
                                                .description(format!("https://discord.com/channels/925674713429184564/{}/{} {ADD} add {} (`{:x}.msch`)", m.channel_id,m.id, emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), new_message.id.get())))
                                        ).await;
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
                                            avatar: author.avatar_url().unwrap_or(CAT.to_string()),
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
                                        if let Some(dir) = SPECIAL.get(&channel_id.get()) && git::has(dir, *id) {
                                            // update :)
                                            git::write(dir, *id, &s);
                                            git::commit(&who,&format!("update {:x}.msch", id.get()));
                                            git::push();
                                            send(c,|x| x
                                                .avatar_url(author.avatar_url().unwrap_or(CAT.to_string()))
                                                .username(who)
                                                .embed(CreateEmbed::new().color(AD)
                                                    .description(format!("https://discord.com/channels/925674713429184564/{channel_id}/{id} {ROTATE} update {} (`{:x}.msch`)", emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), id.get())))
                                            ).await;
                                        }
                                    }
                                }
                            }
                            FullEvent::MessageDelete {
                                deleted_message_id, channel_id, ..
                            } => {
                                if let Some(dir) = SPECIAL.get(&channel_id.get()) {
                                    if let Ok(s) = git::schem(dir, *deleted_message_id) {
                                        let own = git::whos(dir,*deleted_message_id);
                                        git::remove(dir, *deleted_message_id);
                                        git::commit("plent", &format!("remove {:x}", deleted_message_id.get()));
                                        git::push();
                                        send(c,|x| x
                                            .username("plent")
                                            .embed(CreateEmbed::new().color(RM)
                                                .description(format!("{CANCEL} remove {} (added by {own}) (`{:x}.msch`)", emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), deleted_message_id.get()))
                                                .footer(CreateEmbedFooter::new("message was deleted.")
                                            ))
                                        ).await;
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
            .setup(|ctx, _ready, _| {
                Box::pin(async move {
                    poise::builtins::register_globally(ctx, &[logic::run(), help()]).await?;
                    poise::builtins::register_in_guild(ctx, &[search::search(), search::find(), search::file()], 925674713429184564.into()).await?;
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
