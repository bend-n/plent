use anyhow::Result;
use atools::prelude::*;
use block::SORTER;
use exoquant::{
    ditherer::{self, Ditherer},
    Color, Remapper, SimpleColorSpace,
};
use fimg::Image;
use mindus::*;
use poise::{serenity_prelude::*, ChoiceParameter};

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
enum Dithering {
    #[name = "floyd steinberg"]
    FloydSteinberg,
    #[name = "ordered"]
    /// A 2x2 ordered dithering.
    Ordered,
}

fn sort(
    mut x: Image<Box<[u8]>, 4>,
    height: Option<u8>,
    width: Option<u8>,
    algorithm: Option<Scaling>,
    dithered: Option<Dithering>,
) -> (Image<Box<[u8]>, 4>, Schematic) {
    const PAL: [Color; 23] = car::map!(
        [0, 0, 0, 0].join(car::map!(
            car::map!(mindus::item::Type::ALL, |i| i.color()),
            |(r, g, b)| [r, g, b, 255]
        )),
        |[r, g, b, a]| Color { r, g, b, a }
    );

    if width.is_some() || height.is_some() {
        let width = width.map(|x| x as u32).unwrap_or(x.width());
        let height = height.map(|x| x as u32).unwrap_or(x.height());
        x = match algorithm.unwrap_or(Scaling::Nearest) {
            Scaling::Nearest => x.scale::<fimg::scale::Nearest>(width, height),
            Scaling::Lanczos3 => x.scale::<fimg::scale::Lanczos3>(width, height),
            Scaling::Box => x.scale::<fimg::scale::Box>(width, height),
            Scaling::Bilinear => x.scale::<fimg::scale::Bilinear>(width, height),
            Scaling::Hamming => x.scale::<fimg::scale::Hamming>(width, height),
            Scaling::CatmullRom => x.scale::<fimg::scale::CatmullRom>(width, height),
            Scaling::Mitchell => x.scale::<fimg::scale::Mitchell>(width, height),
        };
    };

    fn quant(x: Image<&[u8], 4>, d: impl Ditherer) -> Vec<u8> {
        Remapper::new(&PAL, &SimpleColorSpace::default(), &d).remap(
            &x.chunked()
                .map(|&[r, g, b, a]| Color::new(r, g, b, a))
                .collect::<Vec<_>>(),
            x.width() as usize,
        )
    }

    let quant = match dithered {
        Some(Dithering::FloydSteinberg) => quant(x.as_ref(), ditherer::FloydSteinberg::vanilla()),
        Some(Dithering::Ordered) => quant(x.as_ref(), ditherer::Ordered),
        None => quant(x.as_ref(), ditherer::None),
    };

    let (width, height) = (x.width() as usize, x.height() as usize);
    let mut s = Schematic::new(width, height);
    let pixels = (0..width)
        .flat_map(|x_| (0..height).map(move |y| (x_, y)))
        .filter_map(
            move |(x_, y_)| match quant[(height - y_ - 1) * width + x_] {
                0 => None,
                x => Some(((x_, y_), x - 1)),
            },
        );
    for ((x, y), i) in pixels.clone() {
        s.set(
            x,
            y,
            &SORTER,
            data::dynamic::DynData::Content(mindus::content::Type::Item, i as _),
            block::Rotation::Up,
        )
        .unwrap();
    }
    let mut preview = Image::build(x.width(), x.height()).alloc();
    for ((x, y), i) in pixels {
        unsafe {
            preview.set_pixel(
                x as _,
                (height - y - 1) as _,
                mindus::item::Type::ALL[i as usize]
                    .color()
                    .array()
                    .join(255),
            )
        };
    }
    (
        preview.scale::<fimg::scale::Nearest>(preview.width() * 4, preview.height() * 4),
        s,
    )
}

#[poise::command(slash_command)]
/// Create sorter representations of images.
pub async fn sorter(
    c: super::Context<'_>,
    #[description = "image: png, webp, jpg"] i: Attachment,
    #[description = "height in blocks"] height: Option<u8>,
    #[description = "height in blocks"] width: Option<u8>,
    #[description = "scaling algorithm, defaults to nearest"] algorithm: Option<Scaling>,
    #[description = "dithering algorithm, defaults to none"] dithered: Option<Dithering>,
) -> Result<()> {
    c.defer().await?;
    let image = i.download().await?;
    match image::load_from_memory(&image) {
        Ok(x) => {
            let x = x.to_rgba8();
            let (preview, mut schem) = sort(
                Image::<_, 4>::build(x.width(), x.height())
                    .buf(x.into_vec())
                    .boxed(),
                width,
                height,
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
