use anyhow::Result;
use emoji::named::*;
use mindus::data::DataRead;
use mindus::{Schematic, Serializable};
use poise::serenity_prelude::*;
use std::mem::MaybeUninit;
use std::path::Path;

struct Dq<T, const N: usize> {
    arr: [MaybeUninit<T>; N],
    front: u8,
    len: u8,
}

impl<T: Copy, const N: usize> Dq<T, N> {
    pub fn new(first: T) -> Self {
        let mut dq = Dq {
            arr: unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() },
            front: 0,
            len: 1,
        };
        dq.arr[0].write(first);
        dq
    }

    pub fn first(&mut self) -> T {
        unsafe { self.arr.get_unchecked(self.front as usize).assume_init() }
    }

    pub fn push_front(&mut self, elem: T) {
        // sub 1
        match self.front {
            0 => self.front = N as u8 - 1,
            n => self.front = n - 1,
        }
        self.len += 1;
        unsafe { self.arr.get_unchecked_mut(self.front as usize).write(elem) };
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.arr
            .iter()
            .cycle()
            .skip(self.front as _)
            .take((self.len as usize).min(N))
            .map(|x| unsafe { x.assume_init() })
    }
}

#[poise::command(slash_command)]
/// Find a schematic in the repo
pub async fn find(
    c: super::Context<'_>,
    #[description = "schematic name"] name: String,
) -> Result<()> {
    let mut stack = Dq::<_, 5>::new((
        0.0,
        Data {
            channel: 0,
            message: 0,
        },
    ));
    c.defer().await?;
    for (elem, data) in schems() {
        let cmp = rust_fuzzy_search::fuzzy_compare(elem.tags.get("name").unwrap(), &name);
        if stack.first().0 < cmp {
            stack.push_front((cmp, data));
        }
    }
    if stack.iter().filter(|&(n, _)| n > 0.5).count() == 0 {
        return c
            .say(format!("{CANCEL} not found"))
            .await
            .map(|_| ())
            .map_err(Into::into);
    }
    c.say(
        stack
            .iter()
            .filter(|&(n, _)| n > 0.5)
            .map(|(_, Data { channel, message })| {
                format!(
                    "{RIGHT} https://discord.com/channels/925674713429184564/{channel}/{message}"
                )
            })
            .intersperse("\n".to_string())
            .fold(String::new(), |acc, x| acc + &x),
    )
    .await
    .map(|_| ())
    .map_err(Into::into)
}

#[derive(Copy, Clone)]
pub struct Data {
    channel: u64,
    message: u64,
}

pub fn schems() -> impl Iterator<Item = (Schematic, Data)> {
    super::SPECIAL
        .entries()
        .filter_map(|(&ch, &dir)| {
            std::fs::read_dir(Path::new("repo").join(dir))
                .ok()
                .map(|x| (x, ch))
        })
        .map(|(fs, channel)| {
            fs.filter_map(Result::ok).map(move |f| {
                let dat = std::fs::read(f.path()).unwrap();
                let mut dat = DataRead::new(&dat);
                let ts = Schematic::deserialize(&mut dat).unwrap();
                let p = f.path();
                let x = p.file_name().unwrap().to_string_lossy();
                (
                    ts,
                    Data {
                        channel,
                        message: (u64::from_str_radix(&x[..x.len() - 5], 16).unwrap()),
                    },
                )
            })
        })
        .flatten()
}

#[poise::command(slash_command)]
/// Search for a schematic in the repo
pub async fn search(
    c: super::Context<'_>,
    #[description = "base64 of the schematic"] base64: Option<String>,
    #[description = "msch of the schematic"] msch: Option<Attachment>,
) -> Result<()> {
    let s = match base64
        .and_then(|s| Schematic::deserialize_base64(&s).ok())
        .or(match msch {
            Some(x) => x.download().await.ok().and_then(|x| {
                let mut s = DataRead::new(&x);
                Schematic::deserialize(&mut s).ok()
            }),
            None => None,
        }) {
        Some(x) => x,
        None => return c.say("no schematic").await.map(|_| ()).map_err(Into::into),
    };
    c.defer().await?;

    if let Some((_, Data { channel, message })) = schems().find(|(ts, _)| &s == ts) {
        return c
            .say(format!(
                "{RIGHT} https://discord.com/channels/925674713429184564/{channel}/{message}",
            ))
            .await
            .map(|_| ())
            .map_err(Into::into);
    }
    c.say(format!("{CANCEL} not found"))
        .await
        .map(|_| ())
        .map_err(Into::into)
}
