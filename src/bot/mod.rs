mod data;
mod db;
mod logic;
mod map;
pub mod ownership;
pub mod repos;
mod schematic;
pub mod search;
mod sorter;
use charts_rs::{Series, THEME_GRAFANA};
pub use data::log;

use crate::emoji;
use anyhow::Result;
use dashmap::DashMap;
use mindus::Serializable;
use mindus::data::DataWrite;
use poise::{CreateReply, serenity_prelude::*};
use repos::{FORUMS, Repo, SPECIAL, THREADED};
use serenity::futures::StreamExt;
use core::panic;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs::read_to_string;
use std::ops::ControlFlow;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub fn clone() {
    for repos::Repo { auth, name, .. } in repos::ALL {
        if !Path::new(&format!("repos/{name}")).exists() {
            assert_eq!(
                std::process::Command::new("git")
                    .current_dir("repos")
                    .arg("clone")
                    .args(["--depth", "5"])
                    .arg(auth)
                    .arg(format!("{name}"))
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()
                    .unwrap()
                    .code()
                    .unwrap(),
                0
            );
        }
    }
}

#[derive(Debug)]
pub struct Data {
    // message -> resp
    tracker: Arc<DashMap<MessageId, (u64, Message)>>,
}
#[derive(Clone)]
pub struct Msg {
    avatar: String,
    author: String,
    locale: String,
    author_id: u64,
    guild: u64,
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

fn tags<T: std::fmt::Display>(t: &[T]) -> String {
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

#[derive(Clone, Debug)]
pub struct Ch {
    repo: &'static Repo,
    d: &'static str,
    ty: Type,
}
#[derive(Clone, Debug)]
enum Type {
    Basic(&'static [&'static str]),
    Owned(Vec<String>),
    Forum(()), // &'static phf::Map<&'static str, &'static [&'static str]>
}
impl Type {
    fn r(&self) -> Option<String> {
     match self {
            Type::Basic(x) => Some(tags(x)),
            Type::Forum(_) => None,
            Type::Owned(x) => Some(tags(x)),
        }

    }
}

fn sep(x: Option<&Ch>) -> (Option<&'static str>, Option<Type>, Option<&Repo>) {
    (
        x.map(|x| x.d),
        x.map(|x| x.ty.clone()),
        x.map(|x| x.repo),
    )
}

const OWNER: u64 = 696196765564534825;
#[poise::command(slash_command)]
/// This command reads all messages to find the schems.
/// This command will possibly add denied schems.
pub async fn scour(
    c: Context<'_>,
    #[description = "the channel in question"] ch: ChannelId,
) -> Result<()> {
    let repo = repos::chief!(c);
    let mut n = 0;
    let d = SPECIAL[&ch.get()].d;
    let h = c.say(format!("scouring {d}...")).await?;
    _ = std::fs::create_dir(format!("repos/{}/{d}", repo.name));
    let Ch { d, repo, ty, .. } = SPECIAL.get(&ch.get()).unwrap();
    match ty {
        Type::Basic(tg) => {
            let mut msgs = ch.messages_iter(c).boxed();
            while let Some(msg) = msgs.next().await {
                let Ok(msg) = msg else {
                    continue;
                };
                if let Ok(Some(mut x)) = schematic::from((&msg.content, &msg.attachments)).await {
                    use emoji::to_mindustry::named::*;
                    let tags = if tg == &["find unit factory"] {
                        tags(&[x.block_iter().find_map(|x| match x.1.block.name() {
                            "air-factory" => Some(AIR_FACTORY),
                            "ground-factory" => Some(GROUND_FACTORY),
                            "naval-factory" => Some(NAVAL_FACTORY),
                            _ => None,
                        }).unwrap_or(AIR_FACTORY)])
                    } else { tags(tg) };
                    x.schem.tags.insert("labels".into(), tags.clone());
                    let who = msg.author_nick(c).await.unwrap_or(msg.author.name.clone());
                    ownership::get(repo)
                        .await
                        .insert(msg.id.get(), (msg.author.name.clone(), msg.author.id.get()));
                    repo.write(d, msg.id, x);
                    repo.commit(&who, msg.author.id, &format!("add {:x}.msch", msg.id.get()));
                    msg.react(c, emojis::get!(MERGE)).await?;
                    n += 1;
                }
            }
        }
        _ => {
            unreachable!()
        }
    }
    repo.push();
    h.edit(
        c,
        poise::CreateReply::default().content(format!(
            "done scouring <#{ch}>! <:merge:1192387272046284800> {n} schems"
        )),
    )
    .await?;
    Ok(())
}

async fn del(
    c: &serenity::prelude::Context,
    &Ch {
        d: dir, repo: git, ..
    }: &Ch,
    deleted_message_id: u64,
) {
    use crate::emoji::named::*;
    if let Ok(s) = git.schem(dir, deleted_message_id.into()) {
        let own = git.own().await.erase(deleted_message_id).unwrap();
        git.remove(dir, deleted_message_id.into());
        git.commit("plent", 0u64, &format!("remove {deleted_message_id:x}"));
        git.push();
        if git == &repos::DESIGN_IT && !cfg!(debug_assertions) {
            send(c, |x| {
                x.username("plent").embed(
                    CreateEmbed::new()
                        .color(RM)
                        .description(format!(
                            "{CANCEL} remove {} (added by {own}) (`{:x}.msch`)",
                            emoji::mindustry::to_discord(&strip_colors(
                                s.tags.get("name").unwrap()
                            )),
                            deleted_message_id
                        ))
                        .footer(CreateEmbedFooter::new("message was deleted.")),
                )
            })
            .await;
        };
    }
}

static HOOK: OnceLock<Webhook> = OnceLock::new();

pub async fn hookup(c: &impl AsRef<Http>) {
    let v = Webhook::from_url(
        c,
        &std::env::var("WEBHOOK")
            .unwrap_or_else(|_| read_to_string("webhook").expect("wher webhook")),
    )
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

const RM: (u8, u8, u8) = (242, 121, 131);
const AD: (u8, u8, u8) = (128, 191, 255);

async fn handle_message(
    c: &poise::serenity_prelude::Context,
    new_message: &Message,
    d: &Data,
) -> Result<()> {
    let who = new_message
        .author_nick(c)
        .await
        .unwrap_or(new_message.author.name.clone());
    let post = EXTRA.get(&new_message.channel_id.get()).map(|x| x.clone());
    let (dir, l, repo) = sep(SPECIAL.get(&new_message.channel_id.get()).or(post.as_ref()));
    let m = Msg {
        author: who.clone(),
        locale: new_message
            .author
            .locale
            .clone()
            .unwrap_or("unknown locale".to_string()),
        author_id: new_message.author.id.get(),
        guild: new_message.guild_id.map_or(0, Into::into),
        avatar: new_message.author.face(),
        attachments: new_message.attachments.clone(),
        content: new_message.content.clone(),
        channel: new_message.channel_id,
    };
    let mut x = ControlFlow::Continue(());
    for m_ in &new_message.message_snapshots {
        let m = Msg {
            content: m_.content.clone(),
            attachments: m_.attachments.clone(),
            ..m.clone()
        };
        if x.is_continue() {
            x = schematic::with(m, l.clone()).await?;
        };
    }
    if x.is_continue() {
        x = schematic::with(m, l).await?;
    }
    match x {
        ControlFlow::Continue(())
            if THREADED.contains(&new_message.channel_id.get())
                || SPECIAL.contains_key(&new_message.channel_id.get()) =>
        {
            new_message.delete(c).await?;
            return Ok(());
        }
        ControlFlow::Break((ha, m, s)) => {
            let (m, n, s) = schematic::send(m,c,s).await?;
            if SPECIAL.contains_key(&m.channel_id.get()) || THREADED.contains(&m.channel_id.get()) {
                m.channel_id
                    .create_thread_from_message(
                        c,
                        m.id,
                        CreateThread::new(n)
                            .audit_log_reason("because yes")
                            .auto_archive_duration(AutoArchiveDuration::OneDay),
                    )
                    .await
                    .unwrap();
            }
            if let Some(dir) = dir
                && let Some(repo) = repo
            {
                println!("adding {dir}");
                // add :)
                repo.own().await.insert(
                    new_message.id.get(),
                    (new_message.author.name.clone(), new_message.author.id.get()),
                );
                use emoji::named::*;
                if repo.name == "DESIGN_IT" && !cfg!(debug_assertions) {
                    send(c,|x| x
                    .avatar_url(new_message.author.face())
                    .username(&who)
                    .embed(CreateEmbed::new().color(AD)
                        .description(format!("https://discord.com/channels/925674713429184564/{}/{} {ADD} add {} (`{:x}.msch`)", m.channel_id,m.id, emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), new_message.id.get())))
                ).await;
                }
                if post.is_some() {
                    EXTRA.remove(&new_message.channel_id.get());
                    db::set(new_message.channel_id.get(), new_message.id.get());
                }
                repo.write(dir, new_message.id, s);
                repo.add();
                repo.commit(&who, new_message.author.id, &format!("add {:x}.msch", new_message.id.get()));
                repo.push();
                new_message.react(c, emojis::get!(MERGE)).await?;
            }
            d.tracker.insert(new_message.id, (ha, m));
            return Ok(());
        }
        _ => (),
    };

    // not tracked, as you cant add a attachment afterwwards.
    map::with(new_message, c).await?;
    Ok(())
}
static SEEN: LazyLock<Mutex<HashSet<(GuildId, u64, String, UserId)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
static EXTRA: LazyLock<DashMap<u64, Ch>> = LazyLock::new(DashMap::new);
pub struct Bot;
impl Bot {
    pub async fn spawn() {
        use emoji::named::*;
        println!("bot startup");
        let tok =
            std::env::var("TOKEN").unwrap_or_else(|_| read_to_string("token").expect("wher token"));
        let f = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    logic::run(),
                    lb(),
                    logic::run_file(),
                    sorter::sorter(),
                    sorter::mapper(),
                    schembrowser_instructions(),
                    lb_no_vds(),
                    ping(),
                    help(),
                    scour(),
                    search::search(),
                    search::file(),
                    rename(),
                    rename_file(),
                    render(),
                    render_file(),
                    render_message(),
                    map::render_message(),
                    stats(),
                    retag()
                ],
                event_handler: |c, e, _, d| {
                    Box::pin(async move {
                        match e {
                            FullEvent::Ready { .. } => {
                                println!("bot ready");
                                while SEEN.lock().await.len() < 5 {

                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                }
                                let mut x = SEEN.lock().await.clone().into_iter().collect::<Vec<_>>();
                                x.sort_by_key(|(_, x, _,_)|*x);
                                for (g, _, _ ,_ ) in x.iter().take_while(|(_, x, _,_)| *x <= 10).filter(|(_, _, _,x)| *x != OWNER) {
                                    // println!()
                                    // g.leave(&c).await.unwrap();
                                };
                                for (i, member_count, name, _) in x {
                                    println!(
                                            "{name} has {member_count:?} members {i:?}"
                                        );
                                }
                                SEEN.lock().await.clear();
                                emojis::load(c.http()).await;
                                hookup(c.http()).await;
                            }
                            FullEvent::GuildCreate { guild , ..} => {
                                SEEN.lock().await.insert((guild.id, guild.member_count, guild.name.clone(), guild.owner_id));
                                // let User{id,name:owner_name,..} = c.http().get_user(*owner_id).await.unwrap();
                            }
                            // :deny:, @vd
                            FullEvent::ReactionAdd { add_reaction: Reaction {
                                message_id,
                                emoji: ReactionType::Custom { id, .. },
                                channel_id,
                                member: Some(m @ Member {
                                    nick,
                                    user,
                                .. }), .. }
                            }
                            if let Some(Ch {
                                d: dir,
                                repo: git,
                            .. }) = SPECIAL.get(&channel_id.get()).or(
                                channel_id.to_channel(c.http()).await
                                    .ok().and_then(|x| x.guild())
                                        .and_then(|x| x.parent_id)
                                        .and_then(|x| FORUMS.get(&x.get())),
                                )
                                && *id == git.deny_emoji
                                && git.auth(m)
                            => {
                                // repos::ALL.into_iter().filter(|x|x.own().await.get(k))
                                let m = c.http().get_message(*channel_id,* message_id).await?;
                                if let Ok(s) = git.schem(dir,*message_id) {
                                    _ = db::remove(channel_id.get());
                                    let who = nick.as_deref().unwrap_or(&user.name);
                                    let own = ownership::get(git).await.erase(*message_id).unwrap();
                                    git.remove(dir, *message_id);
                                    git.commit(who,  m.author.id, &format!("remove {:x}.msch", message_id.get()));
                                    git.push();
                                    _ = m.delete_reaction(c,Some(1174262682573082644.into()), emojis::get!(MERGE)).await;
                                    m.delete_reaction(c,Some(1174262682573082644.into()), ReactionType::Custom { animated: false, id: 1192316518395039864.into(), name: Some("merge".into()) }).await.unwrap();
                                    m.react(c,emojis::get!(DENY)).await?;
                                    // only design-it has a webhook (possibly subject to future change)
                                    if git.name == "DESIGN_IT" && !cfg!(debug_assertions) {
                                    send(c,|x| x
                                        .avatar_url(user.face())
                                        .username(who)
                                        .embed(CreateEmbed::new().color(RM)
                                            .description(format!("https://discord.com/channels/925674713429184564/{channel_id}/{message_id} {} {} (added by {own}) (`{:x}`)", emojis::get!(DENY), emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), message_id.get())))
                                    ).await;
                                }
                                };
                            }
                            FullEvent::Message { new_message } => {
                                if new_message.content.starts_with('!')
                                || new_message.content.starts_with(PFX)
                                || new_message.author.bot
                                {
                                    return Ok(());
                                }
                                handle_message(c, new_message, d).await?;
                            },
                            FullEvent::ThreadCreate { thread } if let Some(Ch{repo, d, ty: Type::Forum(_)}) = repos::FORUMS.get(&thread.parent_id.unwrap().get()) => {
                                let tg = thread.guild(c).unwrap().channels[&thread.parent_id.unwrap()].available_tags.iter()
                                .filter(|x| {
                                    thread.applied_tags.contains(&x.id)
                                }).map(|x| x.name.clone()).collect::<Vec<_>>();
                                EXTRA.insert(thread.id.get(), Ch { repo, d, ty: Type::Owned(tg) });   
                            }
                            FullEvent::MessageUpdate {event:MessageUpdateEvent {
                                author: Some(author),
                                guild_id: Some(guild_id),
                                content: Some(content),
                                attachments: Some(attachments),
                                id,
                                channel_id,
                                ..
                            }, ..} => {
                                if let Some((_, (hash, r))) = d.tracker.remove(id) {
                                    let who = author
                                    .nick_in(c, guild_id)
                                    .await
                                    .unwrap_or(author.name.clone());
                                    let (dir, l, repo) = sep(SPECIAL.get(&r.channel_id.get()));
                                    if let ControlFlow::Break((ha, m, v)) = schematic::with(
                                        Msg {
                                            locale:author.locale.clone().unwrap_or("unknown locale".to_string()),
                                            author_id: author.id.get(),
                                            avatar: author.face(),
                                            author: who.clone(),
                                            content:content.clone(),
                                            guild: r.guild_id.map_or(0,Into::into),
                                            attachments:attachments.clone(),
                                            channel: *channel_id,
                                        },
                                        l
                                    )
                                    .await? && ha != hash {
                                        _ = r.delete(c).await;
                                        let (m, _, s) = schematic::send(m,c, v).await?;
                                        d.tracker.insert(*id, (ha, m));
                                        if let Some(dir) = dir && let Some(git) = repo && git.has(dir, *id) {
                                            // update :)
                                            if *guild_id == 925674713429184564 && !cfg!(debug_assertions) {
                                            send(c,|x| x
                                                .avatar_url(author.face())
                                                .username(&who)
                                                .embed(CreateEmbed::new().color(AD)
                                                    .description(format!("https://discord.com/channels/925674713429184564/{channel_id}/{id} {ROTATE} update {} (`{:x}.msch`)", emoji::mindustry::to_discord(&strip_colors(s.tags.get("name").unwrap())), id.get())))
                                            ).await;
                                        }
                                            git.write(dir, *id, s);
                                            git.commit(&who, author.id, &format!("update {:x}.msch", id.get()));
                                            git.push();
                                        }
                                    }
                                }
                            }
                            FullEvent::ThreadDelete { thread, .. } if let Some(ch) = FORUMS.get(&thread.parent_id.get()) && let Some(deleted_message_id) = db::remove(thread.id.get()) => del(&c, ch, deleted_message_id).await,
                            FullEvent::MessageDelete {
                                deleted_message_id, channel_id, ..
                            } => {
                                if let Some(ch) = SPECIAL.get(&channel_id.get()).or(
                                    channel_id.to_channel(c.http()).await
                                    .ok().and_then(|x| x.guild())
                                        .and_then(|x| x.parent_id)
                                        .and_then(|x| FORUMS.get(&x.get()))
                                ) {
                                    _ = db::remove(channel_id.get());
                                    del(&c, ch, deleted_message_id.get()).await;
                                }
                                if let Some((_, (_, r))) = d.tracker.remove(deleted_message_id) {
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
                    poise::builtins::register_globally(
                        ctx,
                        &[
                            logic::run(),
                            help(),
                            ping(),
                            render(),
                            schembrowser_instructions(),
                            render_file(),
                            render_message(),
                            rename(),
                            rename_file(),
                            stats(),
                            map::render_message(),
                            logic::run_file(),
                            sorter::sorter(),
                            sorter::mapper(),
                        ],
                    )
                    .await?;
                    poise::builtins::register_in_guild(
                        ctx,
                        &[search::search(), lb(), lb_no_vds(), search::file(), retag()],
                        925674713429184564.into(),
                    )
                    .await?;
                    poise::builtins::register_in_guild(ctx, &[scour()], 1388427745066750045.into()).await?;
                    println!("registered");
                    let tracker = Arc::new(DashMap::new());
                    let tc = Arc::clone(&tracker);
                    tokio::spawn(async move {
                        loop {
                            // every 10 minutes
                            tokio::time::sleep(Duration::from_secs(60 * 10)).await;
                            tc.retain(|_, (_, v): &mut (_, Message)| {
                                // prune messagees older than 3 hours
                                Timestamp::now().unix_timestamp() - v.timestamp.unix_timestamp()
                                    < 60 * 60 * 3
                            });
                        }
                    });
                    Ok(Data { tracker })
                })
            })
            .build();
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

// pub async fn missing(r: &'static Repo) -> impl Iterator<Item = (MessageId, ChannelId)> {
//     let lock = r.own().await;
//     search::files()
//         .map(move |(x, ch)| {
//             let f = search::flake(x.file_name().unwrap().to_str().unwrap());
//             (lock.map.contains_key(&f), f, ch)
//         })
//         .filter_map(|(x, m, c)| (!x).then(|| (m.into(), c.into())))
// }

// #[poise::command(slash_command)]
// pub async fn bust_ghosts(c: Context<'_>) -> Result<()> {
//     if c.author().id != OWNER {
//         poise::say_reply(c, "access denied. this incident will be reported").await?;
//         return Ok(());
//     }
//     let h = c.reply(emoji::named::LOCK_OPEN).await?;
//     for (m, ch) in missing().await.collect::<Vec<_>>() {
//         let ch = c.guild().unwrap().channels[&ch].clone();
//         let User { id, name, .. } = match ch.message(c, m).await {
//             Ok(x) => x.author,
//             Err(_) => {
//                 // removes ghosts
//                 std::fs::remove_file(git::path(&SPECIAL[&ch.id.get()].d, m)).unwrap();
//                 continue;
//             }
//         };
//         ownership::insert(m.into(), (name, id.get())).await;
//     }
//     h.edit(c, poise::CreateReply::default().content(emoji::named::LOCK))
//         .await?;
//     Ok(())
// }

#[poise::command(slash_command)]
pub async fn retag(c: Context<'_>) -> Result<()> {
    if c.author().id != OWNER {
        poise::say_reply(c, "access denied. this incident will be reported").await?;
        return Ok(());
    }
    c.defer().await?;
    for (&channel, x) in repos::SPECIAL.into_iter().filter(|x| {
        x.1.repo == &repos::DESIGN_IT
    }) { 
        let (_, Some(tags), _) = sep(Some(x)) else { panic!() };
        let Some(tags) = tags.r() else { panic!() };
    for schem in search::dir(channel).into_iter().flatten() {
        let mut s = search::load(&schem);
        let mut v = DataWrite::default();
        s.tags.insert("labels".into(), tags.clone());
        s.serialize(&mut v)?;
        std::fs::write(schem, v.consume())?;
    }}
    c.reply(emoji::named::OK).await?;
    Ok(())
}

// dbg!(m
//     .iter()
//     .filter(|x| x.roles.contains(&925676016708489227.into()))
//     .map(|x| x.user.id.get())
//     .collect::<Vec<_>>());

const VDS: &[u64] = &[
    1222024015015706668,
    742034952077705317,
    126381304857100288,
    175218107084832768,
    221780012372721664,
    231505175246798851,
    291255752729821185,
    301919226078298114,
    315827169395998720,
    324736330418487317,
    325570201837895680,
    330298929331699713,
    332054403160735765,
    343939197738024961,
    360488990974935040,
    384188568270274581,
    387018214103842818,
    391302959444656128,
    399346439349600256,
    404682730190798858,
    417607639938236427,
    461517080856887297,
    464033296012017674,
    488243005283631106,
    490271325126918154,
    514981385660792852,
    527626094744961053,
    586994631879819266,
    595625721129336868,
    618507912511225876,
    665938033987682350,
    696196765564534825,
    705503407179431937,
    724657758280089701,
    729281676441550898,
    797211831894016012,
    845191508033667096,
];
pub async fn leaderboard(c: Context<'_>, channel: Option<ChannelId>, vds: bool) -> Result<()> {
    use emoji::named::*;
    c.defer().await?;
    let lock = repos::DESIGN_IT.own().await;
    let process = |map: HashMap<u64, u16>| {
        let mut v = map.into_iter().collect::<Vec<_>>();
        v.sort_by_key(|(_, x)| *x);
        use std::fmt::Write;
        let mut out = String::new();
        v.iter()
            .rev()
            .zip(1..)
            .take(5)
            .for_each(|((y, z), x)| writeln!(out, "{x}. **<@{y}>**: {z}").unwrap());

        out
    };
    // match channel {
    //     Some(ch) => {
    //         let Some(x) = SPECIAL.get(&ch.get()) else {
    //             poise::say_reply(c, format!("{CANCEL} not a schem channel")).await?;
    //             return Ok(());
    //         };
    //         let mut map = HashMap::new();
    //         search::dir(ch.get())
    //             .unwrap()
    //             .map(|y| lock.map[&search::flake(y.file_name().unwrap().to_str().unwrap())].1)
    //             .filter(|x| vds || !VDS.contains(x))
    //             .for_each(|x| *map.entry(x).or_default() += 1);
    //         poise::say_reply(
    //             c,
    //             format!(
    //                 "## Leaderboard of {}\n{}",
    //                 x.ls
    //                     .join("")
    //                     .chars()
    //                     .map(|x| emoji::mindustry::TO_DISCORD[&x])
    //                     .collect::<String>(),
    //                 process(map)
    //             ),
    //         )
    //     }
    //     None => {
    let mut map = std::collections::HashMap::new();
    search::files()
        .map(|(y, _)| lock.map[&search::flake(y.file_name().unwrap().to_str().unwrap())].1)
        .filter(|x| vds || !VDS.contains(x))
        .for_each(|x| *map.entry(x).or_default() += 1);
    poise::say_reply(c, format!("## Leaderboard\n{}", process(map))).await?;
    Ok(())
}

#[poise::command(slash_command)]
/// Show the leaderboard for players with the most contributed schems, optionally in a certain channel.
pub async fn lb(
    c: Context<'_>,
    #[description = "optional channel filter"] channel: Option<ChannelId>,
) -> Result<()> {
    leaderboard(c, channel, true).await
}

#[poise::command(slash_command)]
/// Show the leaderboard for players, excepting verified designers, with the most schemes.
pub async fn lb_no_vds(
    c: Context<'_>,
    #[description = "optional channel filter"] channel: Option<ChannelId>,
) -> Result<()> {
    leaderboard(c, channel, false).await
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
            /* let bt = error.backtrace();
            if bt.status() == std::backtrace::BacktraceStatus::Captured {
                let parsed = btparse_stable::deserialize(error.backtrace()).unwrap();
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
            } */
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

#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<()> {
    log(&ctx);
    macro_rules! pick {
        ($e:literal, $u:literal) => {
            if matches!(
                command.as_deref(),
                Some("eval") | Some("exec") | Some("run")
            ) {
                include_str!($e)
            } else {
                include_str!($u)
            }
        };
    }

    ctx.send(
        poise::CreateReply::default()
            .allowed_mentions(CreateAllowedMentions::new())
            .content(match ctx.locale() {
                Some("ru") => pick!("help_eval_ru.md", "usage_ru.md"),
                _ => pick!("help_eval.md", "usage.md"),
            }),
    )
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

#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
/// Pong!
pub async fn ping(c: Context<'_>) -> Result<()> {
    // let p = Timestamp::now()
    //     .signed_duration_since(*c.created_at())
    //     .to_std()?
    //     .as_millis() as _;
    log(&c);
    use emoji::named::*;
    let m = memory_stats::memory_stats().unwrap().physical_mem as f32 / (1 << 20) as f32;

    let start = cpu_monitor::CpuInstant::now()?;
    std::thread::sleep(Duration::from_millis(200));
    let end = cpu_monitor::CpuInstant::now()?;
    let duration = end - start;
    let util = duration.non_idle() * 100.0;

    // let m = (m / 0.1) + 0.5;
    // let m = m.floor() * 0.1;
    c.reply(format!(
        "pong!\n{DISCORD}{RIGHT}: {}ms — {HOST}: {m:.1}MiB - <:stopwatch:1361892467510870167><:world_processor:1307657404128690268> {util:.0}% — <:up:1307658579251167302><:time:1361892343199957022> {}",
        c.ping().await.as_millis(),
        humantime::format_duration(Duration::from_secs(
            Instant::now()
                .duration_since(*super::START.get().unwrap())
                .as_secs()
        ))
    ))
    .await?;
    Ok(())
}

#[poise::command(
    slash_command,
    install_context = "User|Guild",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
/// Renders base64 schematic.
pub async fn render(c: Context<'_>, #[description = "schematic, base64"] s: String) -> Result<()> {
    log(&c);
    poise::send_reply(
        c,
        match schematic::from_b64(&s) {
            Ok(s) => schematic::reply(s, &c.author().name, &c.author().face()).await?,
            Err(e) => CreateReply::default().content(format!("schem broken / not schem: {e}")),
        },
    )
    .await?;
    Ok(())
}

#[poise::command(
    slash_command,
    install_context = "User|Guild",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
/// Renders map/msch schematic.
pub async fn render_file(
    c: Context<'_>,
    #[description = "map / schematic, msch"] s: Attachment,
) -> Result<()> {
    log(&c);
    _ = c.defer().await;

    let Some(s) = schematic::from_attachments(std::slice::from_ref(&s)).await? else {
        match map::reply(c, &s).await? {
            ControlFlow::Break(x) => return Ok(drop(poise::send_reply(c, x).await?)),
            ControlFlow::Continue(e) if e != "not a map." => {
                return Ok(drop(poise::say_reply(c, e).await?));
            }
            ControlFlow::Continue(_) => (),
        };
        poise::send_reply(
            c,
            CreateReply::default()
                .content("no schem found")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };
    poise::send_reply(
        c,
        schematic::reply(s, &c.author().name, &c.author().face()).await?,
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
/// Rename a schematic.
async fn rename_file(
    c: Context<'_>,
    #[description = "schematic, msch"] s: Attachment,
    #[description = "new name"] name: String,
) -> Result<()> {
    log(&c);
    let Some(schematic::Schem { schem: mut s }) =
        schematic::from_attachments(std::slice::from_ref(&s)).await?
    else {
        c.reply("no schem!").await?;
        return Ok(());
    };
    s.tags.insert("name".to_string(), name);
    let mut o = DataWrite::default();
    s.serialize(&mut o)?;
    poise::send_reply(
        c,
        CreateReply::default().attachment(CreateAttachment::bytes(o.consume(), "out.msch")),
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
/// Rename a schematic.
async fn rename(
    c: Context<'_>,
    #[description = "schematic, base64"] s: String,
    #[description = "new name"] name: String,
) -> Result<()> {
    log(&c);
    let Ok(schematic::Schem { schem: mut s }) = schematic::from_b64(&*s) else {
        c.reply("no schem!").await?;
        return Ok(());
    };
    s.tags.insert("name".to_string(), name);
    let mut o = DataWrite::default();
    s.serialize(&mut o)?;
    poise::send_reply(
        c,
        CreateReply::default().attachment(CreateAttachment::bytes(o.consume(), "out.msch")),
    )
    .await?;
    Ok(())
}

#[poise::command(
    context_menu_command = "Render schematic",
    install_context = "User|Guild",
    interaction_context = "Guild|PrivateChannel"
)]
/// Renders schematic inside a message.
pub async fn render_message(c: Context<'_>, m: Message) -> Result<()> {
    log(&c);
    poise::send_reply(
        c,
        match schematic::from((&m.content, &m.attachments)).await {
            Ok(Some(s)) => {
                schematic::reply(
                    s,
                    &m.author_nick(c)
                        .await
                        .unwrap_or_else(|| m.author.name.clone()),
                    &m.author.face(),
                )
                .await?
            }
            Err(e) => CreateReply::default().content(format!("schematic error {e}")),
            Ok(None) => CreateReply::default()
                .content("no schem found")
                .ephemeral(true),
        },
    )
    .await?;
    Ok(())
}

#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|PrivateChannel"
)]
/// Instructions on adding a schematic repository to YOUR server!
pub async fn schembrowser_instructions(c: Context<'_>) -> Result<()> {
    log(&c);
    poise::send_reply(
        c,
        poise::CreateReply::default()
            .content(include_str!("repo.md"))
            .allowed_mentions(CreateAllowedMentions::default().empty_users().empty_roles()),
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
/// Statistics
#[implicit_fn::implicit_fn]
pub async fn stats(c: Context<'_>) -> Result<()> {
    let mut guilds = HashMap::<_, u64>::default();
    let mut schem_calls = 0;
    let mut map_calls = 0;
    let mut eval_calls = 0;
    for x in std::fs::read_to_string("data")
        .unwrap()
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .filter_map(Result::ok)
    {
        *guilds
            .entry(x.get("guild").unwrap().as_u64().unwrap())
            .or_default() += 1;
        let x = x.get("cname").unwrap().as_str().unwrap();
        if x.contains("schematic") {
            schem_calls += 1;
        }
        if x.contains("map") {
            map_calls += 1;
        }
        if x.contains("eval") {
            eval_calls += 1;
        }
    }
    use futures::stream;

    let mut x = stream::iter(guilds.into_iter().filter(_.0 != 0).filter(_.1 > 25))
        .map(async |(k, v)| {
            GuildId::new(k)
                .to_partial_guild(c.http())
                .await
                .map(|x| (x.name, v))
                .unwrap_or(("DM".to_string(), v))
        })
        .buffer_unordered(16)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|(a, b)| Series::new(a, vec![b as f32]))
        .collect::<Vec<_>>();
    x.sort_by_key(|x| x.data[0] as u64);

    let mut ch = charts_rs::PieChart::new_with_theme(x, THEME_GRAFANA);
    ch.title_text = "usage".into();
    // ch.font_family = "Verdana".into();
    ch.width = 800.0;
    ch.rose_type = Some(false);
    ch.inner_radius = 20.0;
    ch.height = 300.0;

    use emoji::named::*;
    let x = charts_rs::svg_to_webp(&ch.svg().unwrap()).unwrap();
    poise::send_reply(c, poise::CreateReply::default().attachment(CreateAttachment::bytes(x, "chart.webp")).content(format!("{EDIT} total schematics rendered: {schem_calls}\n{MAP} total maps rendered: {map_calls}\n{WORLD_PROCESSOR} eval calls: {eval_calls}"))).await?;
    Ok(())
}
