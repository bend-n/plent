use super::{Context, Result};
use lemu::Executor;
use poise::{serenity_prelude::AttachmentType, CodeBlock, KeyValueArgs};
use std::borrow::Cow;

#[poise::command(prefix_command, track_edits, rename = "eval")]
pub async fn run(
    ctx: Context<'_>,
    #[description = "number of iterations"] kv: KeyValueArgs,
    #[description = "Script"] block: CodeBlock,
) -> Result<()> {
    let _ = ctx.channel_id().start_typing(&ctx.serenity_context().http);
    let lemu::Output {
        output: Some(output),
        displays,
        ..
    } = (match tokio::task::spawn_blocking(move || {
        Executor::with_output(vec![])
            .large_display()
            .limit_iterations(
                kv.get("iters")
                    .map_or(1, |v| v.parse::<usize>().unwrap_or(1).clamp(1, 50)),
            )
            .limit_instructions(52789849)
            .program(&block.code)
            .map(|mut v| {
                v.run();
                v.output()
            })
            .map_err(|e| format!("{}", e.diagnose(&block.code)).replace('`', "\u{200b}`"))
    })
    .await?
    {
        Ok(o) => o,
        Err(e) => {
            ctx.send(|c| {
                c.allowed_mentions(|a| a.empty_parse())
                    .content(format!("```ansi\n{e}\n```"))
            })
            .await?;
            return Ok(());
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

    ctx.send(|c| {
        if output.is_empty() && display.is_none() {
            c.content("no output");
        }
        if !output.is_empty() {
            c.content(format!(
                "```\n{}\n```",
                String::from_utf8_lossy(&output).replace('`', "\u{200b}`")
            ));
        }
        if let Some(display) = display {
            c.attachment(AttachmentType::Bytes {
                data: Cow::from(display),
                filename: "display1.png".to_string(),
            })
            .embed(|e| e.attachment("display1.png"));
        }
        c.allowed_mentions(|a| a.empty_parse())
    })
    .await?;

    Ok(())
}
