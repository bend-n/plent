mod logic;
mod map;
mod schematic;
mod search;

use anyhow::Result;
use dashmap::DashMap;
use mindus::data::DataWrite;
use mindus::Serializable;
use poise::serenity_prelude::*;
use serenity::futures::StreamExt;
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

macro_rules! decl {
    ($($ch:literal $( => $item:literal : [$($labels: expr),* $(,)?])?),+ $(,)?) => {
        use emoji::to_mindustry::named::*;
        const THREADED: phf::Set<u64> = phf::phf_set! { $($ch,)+ };
        const SPECIAL: phf::Map<u64, Ch> = phf::phf_map! {
            $($($ch => Ch { d: $item, labels: &[$($labels,)+] })?),+
        };
    };
}

fn tags(t: &[&str]) -> String {
    if let [x, rest @ ..] = t {
        let mut s = format!("[\"{x}\"");
        for elem in rest {
            write!(s, ",\"{elem}\"").unwrap();
        }
        write!(s, "]").unwrap();
        s
    } else {
        String::from("[]")
    }
}

decl! {
    925721957209636914u64 => "cryofluid" : [CRYOFLUID, CRYOFLUID_MIXER],
    925721791475904533u64 => "graphite" : [GRAPHITE, GRAPHITE_PRESS],
    925721824556359720u64 => "metaglass" : [METAGLASS, KILN],
    925721863525646356u64 => "phase-fabric" : [PHASE_FABRIC, PHASE_WEAVER],
    927036346869104693u64 => "plastanium" : [PLASTANIUM, PLASTANIUM_COMPRESSOR],
    925736419983515688u64 => "pyratite" : [PYRATITE, PYRATITE_MIXER],
    925736573037838397u64 => "blast-compound" : [BLAST_COMPOUND, BLAST_MIXER],
    927793648417009676u64 => "scrap" : [SCRAP],
    1198556531281637506u64 => "spore-press" : [OIL, SPORE_PRESS],
    1200308146460180520u64 => "oil-extractor" : [OIL, OIL_EXTRACTOR],
    1200301847387316317u64 => "rtg-gen" : [POWER, RTG_GENERATOR],
    1200308292744921088u64 => "cultivator" : [SPORE_POD, CULTIVATOR],
    1200305956689547324u64 => "graphite-multipress" : [GRAPHITE, MULTI_PRESS],
    1200306409036857384u64 => "silicon-crucible" : [SILICON, SILICON_CRUCIBLE],
    1198555991667646464u64 => "coal" : [COAL, COAL_CENTRIFUGE],
    925721763856404520u64 => "silicon" : [SILICON, SILICON_SMELTER],
    925721930814869524u64 => "surge-alloy" : [SURGE_ALLOY, SURGE_SMELTER],
    1141034314163826879u64 => "defensive-outpost" : [""],
    949529149800865862u64 => "drills" : [PRODUCTION],
    925729855574794311u64 => "logic-schems" : [MICRO_PROCESSOR],
    1185702384194818048u64 => "miscellaneous" : ["…"],
    1018541701431836803u64 => "combustion-gen" : [POWER, COMBUSTION_GENERATOR],
    927480650859184171u64 => "differential-gen" : [POWER, DIFFERENTIAL_GENERATOR],
    925719985987403776u64 => "impact-reactor" : [POWER, IMPACT_REACTOR],
    949740875817287771u64 => "steam-gen" : [POWER, STEAM_GENERATOR],
    926163105694752811u64 => "thorium-reactor" : [POWER, THORIUM_REACTOR],
    973234467357458463u64 => "carbide" : [CARBIDE, ""],
    1198527267933007893u64 => "erekir-defensive-outpost" : [""],
    973236445567410186u64 => "fissile-matter" : [FISSILE_MATTER, ""],
    1147887958351945738u64 => "electrolyzer" : [HYDROGEN, OZONE, ""],
    1202001032503365673u64 => "nitrogen" : [NITROGEN, ""],
    1202001055349477426u64 => "cyanogen" : [CYANOGEN, ""],
    1096157669112418454u64 => "mass-driver" : [""],
    973234248054104115u64 => "oxide" : [OXIDE, ""],
    973422874734002216u64 => "erekir-phase" : [PHASE_FABRIC, ""],
    973369188800413787u64 => "ccc" : ["", POWER],
    1218453338396430406u64 => "neoplasia-reactor": ["", POWER],
    1218453292045172817u64 => "flux-reactor": ["", POWER],
    1218452986788053012u64 => "pyrolisis-gen": ["", POWER],
    1147722735305367572u64 => "silicon-arc" : [SILICON, ""],
    974450769967341568u64 => "erekir-surge" : [SURGE_ALLOY, ""],
    973241041685737532u64 => "erekir-units" : ["[#ff9266][]"],
    1158818171139133490u64 => "unit-core" : [UNITS, CORE_NUCLEUS],
    1158818324210274365u64 => "unit-delivery" : [UNITS, FLARE],
    1158818598568075365u64 => "unit-raw" : [UNITS, PRODUCTION],
    1142181013779398676u64 => "unit-sand" : [UNITS, SAND],
    1222270513045438464u64 => "bore": [PRODUCTION],
    1226407271978766356u64 => "pulveriser": [PULVERIZER, SAND],

    1129391545418797147u64,
}

#[derive(Copy, Clone, Debug)]
struct Ch {
    d: &'static str,
    labels: &'static [&'static str],
}

fn sep(x: Option<&Ch>) -> (Option<&'static str>, Option<String>) {
    (x.map(|x| x.d), x.map(|x| tags(x.labels)))
}

#[poise::command(slash_command)]
pub async fn tag(c: Context<'_>) -> Result<()> {
    if c.author().id != OWNER {
        poise::say_reply(c, "access denied. this incident will be reported").await?;
        return Ok(());
    }
    c.defer().await?;
    for (tags, schem) in SPECIAL.keys().filter_map(|&x| search::dir(x).map(move |y| y.map(move |y| (tags(SPECIAL[&x].labels), y)))).flatten() {
        let mut s = search::load(&schem);
        let mut v = DataWrite::default();
        s.tags.insert("labels".into(), tags);
        s.serialize(&mut v)?;
        std::fs::write(schem, v.consume())?;
    }
    send(&c, |x| {
        x.avatar_url(CAT.to_string()).username("bendn <3").embed(
            CreateEmbed::new()
                .color(RM)
                .description(format!("fixed tags :heart:")),
        )
    })
    .await;
    c.reply("fin").await?;
    Ok(())
}

const OWNER: u64 = 696196765564534825;
#[poise::command(slash_command)]
pub async fn scour(c: Context<'_>, ch: ChannelId) -> Result<()> {
    if c.author().id != OWNER {
        poise::say_reply(c, "access denied. this incident will be reported").await?;
        return Ok(());
    }
    let mut n = 0;
    let d = SPECIAL[&ch.get()].d;
    let h = c.say(format!("scouring {d}...")).await?;
    _ = std::fs::create_dir(format!("repo/{d}"));
    let mut msgs = ch.messages_iter(c).boxed();
    while let Some(msg) = msgs.next().await {
        let Ok(msg) = msg else {
            continue;
        };
        if let Ok(Some(x)) = schematic::from((&msg.content, &msg.attachments)).await {
            let mut v = DataWrite::default();
            x.serialize(&mut v).unwrap();
            _ = std::fs::write(format!("repo/{d}/{:x}.msch", msg.id.get()), v.consume());
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
    use mindus::data::DataWrite;

    use self::schematic::Schem;

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

    pub fn write(dir: &str, x: MessageId, s: Schem) {
        _ = std::fs::create_dir(format!("repo/{dir}"));
        let mut v = DataWrite::default();
        s.serialize(&mut v).unwrap();
        std::fs::write(path(dir, x), v.consume()).unwrap();
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
        use emoji::named::*;
        println!("bot startup");
        let tok =
            std::env::var("TOKEN").unwrap_or_else(|_| read_to_string("token").expect("wher token"));
        let f = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![logic::run(), help(), scour(), search::search(), search::find(), search::file(), tag()],
                event_handler: |c, e, _, d| {
                    Box::pin(async move {
                        match e {
                            FullEvent::Ready { .. } => {
                                println!("bot ready");
                                emojis::load(c.http()).await;
                                hookup(c.http()).await;
                            }
                            // :deny:, @vd
                            FullEvent::ReactionAdd { add_reaction: Reaction { message_id, emoji: ReactionType::Custom {  id,.. } ,channel_id,member: Some(Member{roles,nick,user,..}),..}} if *id == 1192388789952319499 && let Some(Ch {d:dir,..}) = SPECIAL.get(&channel_id.get()) && roles.contains(&RoleId::new(925676016708489227)) => {
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
                                let (dir, l) = sep(SPECIAL.get(&new_message.channel_id.get()));
                                let x = schematic::with(m, c, l).await?;
                                match x {
                                    ControlFlow::Continue(()) if THREADED.contains(&new_message.channel_id.get()) => {
                                        new_message.delete(c).await?;
                                        return Ok(());
                                    },
                                    ControlFlow::Break((m, n, s)) => {
                                        if THREADED.contains(&m.channel_id.get()) {
                                            m.channel_id.create_thread_from_message(c, m.id,CreateThread::new(n).audit_log_reason("because yes").auto_archive_duration(AutoArchiveDuration::OneDay)).await.unwrap();
                                        }
                                        if let Some(dir) = dir {
                                            // add :)
                                            send(c,|x| x
                                                .avatar_url(new_message.author.avatar_url().unwrap_or(CAT.to_string()))
                                                .username(&who)
                                                .embed(CreateEmbed::new().color(AD)
                                                    .description(format!("https://discord.com/channels/925674713429184564/{}/{} {ADD} add {} (`{:x}.msch`)", m.channel_id,m.id, emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), new_message.id.get())))
                                            ).await;
                                            git::write(dir, new_message.id, s);
                                            git::add();
                                            git::commit(&who, &format!("add {:x}.msch", new_message.id.get()));
                                            git::push();
                                            new_message.react(c, emojis::get!(MERGE)).await?;
                                        }
                                        d.tracker.insert(new_message.id, m);
                                        return Ok(());
                                    },
                                    _ => (),
                                };
                                
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
                                    let (dir, l) = sep(SPECIAL.get(&r.channel_id.get()));
                                    if let ControlFlow::Break((m,_,s)) = schematic::with(
                                        Msg {
                                            avatar: author.avatar_url().unwrap_or(CAT.to_string()),
                                            author: who.clone(),
                                            content:content.clone(),
                                            attachments:attachments.clone(),
                                            channel: *channel_id,
                                        },
                                        c,
                                        l
                                    )
                                    .await?
                                    {
                                        d.tracker.insert(*id, m);
                                        if let Some(dir) = dir && git::has(dir, *id) {
                                            // update :)
                                            send(c,|x| x
                                                .avatar_url(author.avatar_url().unwrap_or(CAT.to_string()))
                                                .username(&who)
                                                .embed(CreateEmbed::new().color(AD)
                                                    .description(format!("https://discord.com/channels/925674713429184564/{channel_id}/{id} {ROTATE} update {} (`{:x}.msch`)", emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), id.get())))
                                            ).await;
                                            git::write(dir, *id, s);
                                            git::commit(&who,&format!("update {:x}.msch", id.get()));
                                            git::push();
                                        }
                                    }
                                }
                            }
                            FullEvent::MessageDelete {
                                deleted_message_id, channel_id, ..
                            } => {
                                if let Some(Ch{ d:dir,..}) = SPECIAL.get(&channel_id.get()) {
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
                    poise::builtins::register_in_guild(ctx, &[tag(), search::search(), scour(), search::find(), search::file()], 925674713429184564.into()).await?;
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
