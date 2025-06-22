use super::{Context, Result};
use lemu::Executor;
use poise::{serenity_prelude::*, CodeBlock, KeyValueArgs};

#[poise::command(slash_command, rename = "eval_file", install_context = "Guild|User")]
/// Execute MLOG from a file.
///
/// Your file can run up to 52789849 instructions, and up to 50 iterations.
/// You get one large display to use.
pub async fn run_file(
    ctx: Context<'_>,
    #[description = "logic, txt"] mlog: Attachment,
    #[description = "number of iterations (0–50)"] iterations: Option<u8>,
) -> Result<()> {
    super::log(&ctx);
    ctx.defer().await?;
    let bytes = mlog.download().await?;
    let Ok(code) = String::from_utf8(bytes) else {
        ctx.say("this is not a mlog file!").await?;
        return Ok(());
    };
    match exec(code, iterations.map_or(1, |x| x.clamp(0, 50)) as _).await {
        Err(Err::Other(x)) => return Err(x),
        Err(Err::Lemu(x)) => {
            ctx.send(
                poise::CreateReply::default()
                    .allowed_mentions(CreateAllowedMentions::default().empty_users().empty_roles())
                    .content(format!("```ansi\n{x}\n```")),
            )
            .await?;
        }
        Ok(x) => drop(ctx.send(x).await?),
    }
    ctx.say(format!("executed [{}]({})", mlog.filename, mlog.url))
        .await?;
    Ok(())
}

#[poise::command(prefix_command, track_edits, rename = "eval")]
pub async fn run(
    ctx: Context<'_>,
    #[description = "number of iterations"] kv: KeyValueArgs,
    #[description = "Script"] block: CodeBlock,
) -> Result<()> {
    super::log(&ctx);
    match exec(
        block.code,
        kv.get("iters")
            .map_or(1, |v| v.parse::<usize>().unwrap_or(1).clamp(1, 50)),
    )
    .await
    {
        Err(Err::Other(x)) => return Err(x),
        Err(Err::Lemu(x)) => {
            ctx.send(
                poise::CreateReply::default()
                    .allowed_mentions(CreateAllowedMentions::default().empty_users().empty_roles())
                    .content(format!("```ansi\n{x}\n```")),
            )
            .await?;
        }
        Ok(x) => drop(ctx.send(x).await?),
    }
    Ok(())
}

enum Err {
    Lemu(String),
    Other(anyhow::Error),
}
impl<T: Into<anyhow::Error>> From<T> for Err {
    fn from(value: T) -> Self {
        Self::Other(value.into())
    }
}

async fn exec(code: String, iters: usize) -> Result<poise::CreateReply, Err> {
    let lemu::Output {
        output: Some(output),
        displays,
        ..
    } = (match tokio::task::spawn_blocking(move || {
        Executor::with_output(vec![])
            .large_display()
            .limit_iterations(iters)
            .limit_instructions(52789849)
            .program(&code)
            .map(|mut v| {
                v.run();
                v.output()
            })
            .map_err(|e| format!("{}", e.diagnose(&code)).replace('`', "\u{200b}`"))
    })
    .await?
    {
        Ok(o) => o,
        Err(e) => {
            return Err(Err::Lemu(e));
        }
    })
    else {
        unreachable!()
    };
    let displays: Box<[_; 1]> = displays.try_into().unwrap();
    let [(display, _)] = *displays;
    let display = if display.buffer().iter().any(|&n| n != 0) {
        Some(
            tokio::task::spawn_blocking(move || {
                let p = oxipng::RawImage::new(
                    display.width(),
                    display.height(),
                    oxipng::ColorType::RGBA,
                    oxipng::BitDepth::Eight,
                    display.take_buffer(),
                )
                .unwrap();
                p.create_optimized_png(&oxipng::Options::default()).unwrap()
            })
            .await?,
        )
    } else {
        None
    };

    let mut c = poise::CreateReply::default();
    if output.is_empty() && display.is_none() {
        c = c.content("no output");
    }
    if !output.is_empty() {
        c = c.content(format!(
            "```\n{}\n```",
            String::from_utf8_lossy(&output).replace('`', "\u{200b}`")
        ));
    }
    if let Some(display) = display {
        c = c
            .attachment(CreateAttachment::bytes(display, "display1.png"))
            .embed(CreateEmbed::default().attachment("display1.png"));
    }
    Ok(c)
}
