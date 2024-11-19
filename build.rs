#![feature(let_chains)]
// use serenity::prelude::*;
use std::fs;
use std::io::prelude::*;
use std::path::Path;

pub fn process(input: impl AsRef<Path>) -> std::io::Result<()> {
    let mut f = fs::File::create(dbg!(Path::new("html").join(input.as_ref()))).unwrap();
    if !matches!(
        input.as_ref().extension().unwrap().to_str().unwrap(),
        "html" | "css"
    ) {
        return f.write_all(&std::fs::read(Path::new("html-src").join(input.as_ref()))?);
    }
    let mut c = std::process::Command::new("minify")
        .arg(Path::new("html-src").join(input.as_ref()))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut o = c.stdout.take().unwrap();
    let mut buf = [0; 1024];
    while let Ok(x) = o.read(&mut buf)
        && x != 0
    {
        f.write_all(&buf[..x])?;
    }
    c.wait()?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    if !Path::new("html").exists() {
        fs::create_dir("html")?;
    }

    emojib::load();

    for path in fs::read_dir("html-src")? {
        process(path.unwrap().path().file_name().unwrap())?;
    }
    println!("cargo:rerun-if-changed=html-src/");
    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
