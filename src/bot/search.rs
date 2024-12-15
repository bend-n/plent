use crate::emoji::named::*;
use anyhow::Result;
use mindus::data::DataRead;
use mindus::{Schematic, Serializable};
use poise::serenity_prelude::*;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};

pub struct Dq<T, const N: usize> {
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
/// Find a schematic by file
pub async fn file(
    c: super::Context<'_>,
    #[description = "schematic file name"] file: String,
) -> Result<()> {
    let Some((file, ch)) = files().find(|(x, _)| x.file_name().unwrap().to_string_lossy() == file)
    else {
        return c
            .say(format!("{CANCEL} not found"))
            .await
            .map(|_| ())
            .map_err(Into::into);
    };
    c.say(format!(
        "{RIGHT} https://discord.com/channels/925674713429184564/{ch}/{}",
        flake(&file.file_name().unwrap().to_string_lossy())
    ))
    .await
    .map(|_| ())
    .map_err(Into::into)
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
    pub channel: u64,
    pub message: u64,
}

pub fn dir(x: u64) -> Option<impl Iterator<Item = PathBuf>> {
    std::fs::read_dir(
        Path::new("repos")
            .join("cd8a83f57821034")
            .join(super::SPECIAL[&x].d),
    )
    .ok()
    .map(|x| x.filter_map(Result::ok).map(move |f| f.path()))
}

pub fn files() -> impl Iterator<Item = (PathBuf, u64)> {
    super::SPECIAL
        .keys()
        .filter_map(|&ch| dir(ch).map(|x| (x, ch)))
        .flat_map(|(fs, channel)| fs.map(move |f| (f, channel)))
}

pub fn load(f: &Path) -> Schematic {
    let dat = std::fs::read(f).unwrap();
    let mut dat = DataRead::new(&dat);
    Schematic::deserialize(&mut dat).unwrap()
}

pub fn schems() -> impl Iterator<Item = (Schematic, Data)> {
    files().map(|(f, channel)| {
        let ts = load(&f);
        let x = f.file_name().unwrap().to_string_lossy();
        (
            ts,
            Data {
                channel,
                message: flake(&x),
            },
        )
    })
}

pub fn flake(x: &str) -> u64 {
    u64::from_str_radix(&x[..x.len() - 5], 16).unwrap()
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
