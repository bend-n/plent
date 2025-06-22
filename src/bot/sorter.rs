use anyhow::Result;
use atools::prelude::*;
use block::SORTER;
use fimg::{indexed::IndexedImage, Image};
use mindus::*;
use poise::{serenity_prelude::*, ChoiceParameter};
use remapper::pal;

#[derive(ChoiceParameter)]
enum Scaling {
    /// dumbest, jaggedest, scaling algorithm
    #[name = "nearest"]
    Nearest,
    /// prettiest scaling algorithm
    #[name = "lanczos 3"]
    Lanczos3,
    #[name = "box"]
    Box,
    #[name = "bilinear"]
    Bilinear,
    #[name = "hamming"]
    Hamming,
    #[name = "catmull rom"]
    CatmullRom,
    #[name = "mitchell"]
    Mitchell,
}

#[derive(ChoiceParameter)]
/// seeks to reduce banding
enum Dithering {
    #[name = "atkinsons"]
    /// error diffusion based dithering.
    Atkinsons,
    #[name = "bayer4x4"]
    /// bayer matrix.
    Bayer4x4,
    #[name = "bayer8x8"]
    /// bayer matrix.
    Bayer8x8,
    #[name = "bayer16x16"]
    /// bayer matrix.
    Bayer16x16,
}

fn d<'a>(
    x: Image<&[u8], 4>,
    d: Option<Dithering>,
    p: pal<'a, 4>,
) -> IndexedImage<Box<[u32]>, pal<'a, 4>> {
    let x = Image::<Box<[f32]>, 4>::from(x);
    match d {
        None => remapper::ordered::remap(x.as_ref(), p),
        Some(Dithering::Atkinsons) => remapper::diffusion::atkinson(x, p),
        Some(Dithering::Bayer4x4) => remapper::ordered::bayer4x4(x.as_ref(), p),
        Some(Dithering::Bayer8x8) => remapper::ordered::bayer8x8(x.as_ref(), p),
        Some(Dithering::Bayer16x16) => remapper::ordered::bayer16x16(x.as_ref(), p),
    }
}

fn s(mut x: Image<Box<[u8]>, 4>, f: f32, a: Option<Scaling>) -> Image<Box<[u8]>, 4> {
    let f = f.min(1.0);
    let width = (x.width() as f32 * f).round() as u32;
    let height = (x.height() as f32 * f).round() as u32;
    match a.unwrap_or(Scaling::Nearest) {
        Scaling::Nearest => x.scale::<fimg::scale::Nearest>(width, height),
        Scaling::Lanczos3 => x.scale::<fimg::scale::Lanczos3>(width, height),
        Scaling::Box => x.scale::<fimg::scale::Box>(width, height),
        Scaling::Bilinear => x.scale::<fimg::scale::Bilinear>(width, height),
        Scaling::Hamming => x.scale::<fimg::scale::Hamming>(width, height),
        Scaling::CatmullRom => x.scale::<fimg::scale::CatmullRom>(width, height),
        Scaling::Mitchell => x.scale::<fimg::scale::Mitchell>(width, height),
    }
}

fn sort(
    mut x: Image<Box<[u8]>, 4>,
    scale_factor: Option<f32>,
    algorithm: Option<Scaling>,
    dithered: Option<Dithering>,
) -> (Image<Box<[u8]>, 4>, Schematic) {
    const PAL: [[f32; 4]; 23] = [0.; 4].join(car::map!(
        car::map!(mindus::item::Type::ALL, |i| i.color()),
        |(r, g, b)| [r as f32 / 256.0, g as f32 / 256.0, b as f32 / 256.0, 1.0]
    ));

    if let Some(f) = scale_factor {
        x = s(x, f, algorithm);
    };

    let mut quant = d(x.as_ref(), dithered, pal::new(&PAL));

    let (width, height) = (x.width() as usize, x.height() as usize);
    let mut s = Schematic::new(width, height);
    let pixels = (0..width)
        .flat_map(|x_| (0..height).map(move |y| (x_, y)))
        .filter_map(|(x_, y_)| {
            match unsafe { quant.raw().buffer() }[(height - y_ - 1) * width + x_] {
                0 => None,
                x => Some(((x_, y_), x - 1)),
            }
        });
    for ((x, y), i) in pixels {
        s.set(
            x,
            y,
            &SORTER,
            data::dynamic::DynData::Content(mindus::content::Type::Item, i as _),
            block::Rotation::Up,
        )
        .unwrap();
    }
    let mut preview = quant.to().to_u8();
    (
        preview.scale::<fimg::scale::Nearest>(preview.width() * 4, preview.height() * 4),
        s,
    )
}

fn map(
    mut x: Image<Box<[u8]>, 4>,
    scale_factor: Option<f32>,
    algorithm: Option<Scaling>,
    dithered: Option<Dithering>,
) -> Image<Box<[u8]>, 4> {
    const PAL: &[[f32; 3]] = unsafe { include!("colors").as_chunks_unchecked::<3>() };

    if let Some(f) = scale_factor {
        x = s(x, f, algorithm);
    };

    let pal = PAL.iter().map(|&x| x.join(1.0)).collect::<Vec<_>>();
    d(x.as_ref(), dithered, pal::new(&pal)).to().to_u8()
}

#[poise::command(slash_command)]
/// Create sorter representations of images.
pub async fn sorter(
    c: super::Context<'_>,
    #[description = "image: png, webp, jpg"] i: Attachment,
    #[description = "scaling factor"] factor: Option<f32>,
    #[description = "scaling algorithm, defaults to nearest"] algorithm: Option<Scaling>,
    #[description = "dithering algorithm, defaults to none"] dithered: Option<Dithering>,
) -> Result<()> {
    super::log(&c);
    c.defer().await?;
    let image = i.download().await?;
    match image::load_from_memory(&image) {
        Ok(x) => {
            let x = x.to_rgba8();
            let (preview, mut schem) = sort(
                Image::<_, 4>::build(x.width(), x.height())
                    .buf(x.into_vec())
                    .boxed(),
                factor,
                algorithm,
                dithered,
            );
            use crate::emoji::to_mindustry::named::*;
            schem
                .tags
                .insert("labels".to_string(), format!(r#"["{SORTER}"]"#));
            let mut h = std::hash::DefaultHasher::default();
            std::hash::Hasher::write(&mut h, preview.bytes());
            let h = std::hash::Hasher::finish(&h) as u32;
            schem
                .tags
                .insert("name".to_string(), format!("{SORTER} #{h:x}"));
            let mut buff = data::DataWrite::default();
            schem.serialize(&mut buff)?;
            let buff = buff.consume();
            let mut preview_png = Vec::with_capacity(1 << 11);
            fimg::WritePng::write(&preview, &mut preview_png).unwrap();
            poise::send_reply(
                c,
                poise::CreateReply::default()
                    .attachment(CreateAttachment::bytes(preview_png, "preview.png"))
                    .attachment(CreateAttachment::bytes(buff, format!("sorter{h:x}.msch"))),
            )
            .await?;
        }
        Err(e) => {
            c.reply(e.to_string()).await?;
        }
    }
    Ok(())
}

#[poise::command(slash_command)]
/// Create map representations of images.
pub async fn mapper(
    c: super::Context<'_>,
    #[description = "image: png, webp, jpg"] i: Attachment,
    #[description = "scaling factor"] factor: Option<f32>,
    #[description = "scaling algorithm, defaults to nearest"] algorithm: Option<Scaling>,
    #[description = "dithering algorithm, defaults to none (if you want the map to be playable, go with ordered)"]
    dithered: Option<Dithering>,
) -> Result<()> {
    super::log(&c);
    c.defer().await?;
    let image = i.download().await?;
    match image::load_from_memory(&image) {
        Ok(x) => {
            let x = x.to_rgba8();
            let preview = map(
                Image::<_, 4>::build(x.width(), x.height())
                    .buf(x.into_vec())
                    .boxed(),
                factor,
                algorithm,
                dithered,
            );
            let mut preview_png = Vec::with_capacity(1 << 11);
            fimg::WritePng::write(&preview, &mut preview_png).unwrap();
            poise::send_reply(
                c,
                poise::CreateReply::default()
                    .attachment(CreateAttachment::bytes(preview_png, "preview.png")),
            )
            .await?;
        }
        Err(e) => {
            c.reply(e.to_string()).await?;
        }
    }
    Ok(())
}
