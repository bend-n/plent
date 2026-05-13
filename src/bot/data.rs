use std::time::SystemTime;

use serde_json::json;
use serenity::json::Value;

use crate::bot::Context;

#[implicit_fn::implicit_fn]
pub fn log(c: &Context<'_>) {
    let v = json! {{
    // "locale": c.author().locale.as_deref().unwrap_or("unknown".into()),
    "name": c.author().name.clone(),
    "id": c.author().id,
    "cname": &*c.command().name,
    "at": SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("292 billion years have passed. goodbye cruel world."),
    "guild": c.guild().map_or(0, |x|x.id.get()),
    "channel": c.channel_id(),
    "message_id": match c { poise::Context::Application(_) => 0, poise::Context::Prefix(x) => x.msg.id.get() },
    }};
    push_j(v);
}

pub fn push_j(j: Value) {
    let mut f = std::fs::File::options().append(true).open("data").unwrap();
    use std::io::Write;
    writeln!(f, "{}", serde_json::to_string(&j).unwrap()).unwrap();
}
