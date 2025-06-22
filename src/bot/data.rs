use serde_json::json;
use serenity::json::Value;

use crate::bot::Context;

#[implicit_fn::implicit_fn]
pub fn log(c: &Context<'_>) {
    let v = json! {{
    "locale": c.author().locale.as_deref().unwrap_or("unknown".into()),
    "name": c.author().name.clone(),
    "id": c.author().id,
    "cname": &*c.command().name,
    "guild": c.guild().map_or(0, |x|x.id.get()),
    "channel": c.channel_id()
    }};
    push_j(v);
}

pub fn push_j(j: Value) {
    let mut f = std::fs::File::options().append(true).open("data").unwrap();
    use std::io::Write;
    writeln!(f, "{}", serde_json::to_string(&j).unwrap()).unwrap();
}
