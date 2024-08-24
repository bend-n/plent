use axum::{
    extract::Path,
    http::{header::*, StatusCode},
    response::{AppendHeaders, Html},
    routing::get,
    Router, Server as AxumServer,
};

use std::{net::SocketAddr, sync::LazyLock, time::SystemTime};
const COMPILED_AT: LazyLock<SystemTime> =
    LazyLock::new(|| edg::r! { || -> std::time::SystemTime { std::time::SystemTime::now() }});
static COMPILED: LazyLock<String> = LazyLock::new(|| httpdate::fmt_http_date(*COMPILED_AT));

fn no_bytes(map: HeaderMap) -> (StatusCode, Option<&'static [u8]>) {
    if let Some(x) = map.get("if-modified-since")
        && let Ok(x) = x.to_str()
        && let Ok(x) = httpdate::parse_http_date(x)
        && x < *COMPILED_AT
    {
        (StatusCode::NOT_MODIFIED, Some(&[]))
    } else {
        (StatusCode::OK, None)
    }
}

macro_rules! html {
    ($file:expr) => {
        get(|_map: HeaderMap| async {
            println!("a wild visitor approaches");
            #[cfg(debug_assertions)]
            return Html(std::fs::read(concat!("html-src/", stringify!($file), ".html")).unwrap());
            #[cfg(not(debug_assertions))]
            {
                let (code, bytes) = no_bytes(_map);
                (
                    code,
                    Html(bytes.unwrap_or(include_bytes!(concat!(
                        "../html/",
                        stringify!($file),
                        ".html"
                    )))),
                )
            }
        })
    };
}

macro_rules! png {
    ($file:expr) => {
        get(|map: HeaderMap| async {
            let (code, bytes) = no_bytes(map);
            let bytes = bytes.unwrap_or(include_bytes!(concat!(
                "../html/",
                stringify!($file),
                ".png"
            )));

            (
                code,
                (
                    AppendHeaders([(CONTENT_TYPE, "image/png"), (LAST_MODIFIED, &*COMPILED)]),
                    bytes,
                ),
            )
        })
    };
}

pub struct Server;
impl Server {
    pub async fn spawn(addr: SocketAddr) {
        let router = Router::new()
            .route("/", html!(index))
            .route("/fail.png", png!(fail))
            .route("/bg.png", png!(bg))
            .route("/border.png", png!(border))
            .route("/border-active.png", png!(border_active))
            .route("/border-hover.png", png!(border_hover))
            .route("/favicon.ico", png!(favicon))
            .route(
                "/default.woff",
                get(|| async {
                    (
                        [(CONTENT_TYPE, "font/woff")],
                        include_bytes!("../html-src/default.woff"),
                    )
                }),
            )
            .route(
                "/index.js",
                get(|| async {
                    (
                        [(CONTENT_TYPE, "application/javascript")],
                        if cfg!(debug_assertions) {
                            std::fs::read_to_string("html-src/index.js").unwrap().leak()
                        } else {
                            include_str!("../html-src/index.js")
                        },
                    )
                }),
            )
            .route(
                "/files",
                get(|| async {
                    serde_json::to_string(
                        &crate::bot::search::files()
                            .map(|(x, _)| {
                                x.with_extension("")
                                    .file_name()
                                    .unwrap()
                                    .to_string_lossy()
                                    .into_owned()
                            })
                            .collect::<Vec<_>>(),
                    )
                    .unwrap()
                }),
            )
            .route(
                "/blame/:file",
                get(|Path(file): Path<String>| async move {
                    match crate::bot::search::files().map(|(x, _)| x).find(|x| {
                        x.with_extension("").file_name().unwrap().to_string_lossy() == file
                    }) {
                        Some(x) => (
                            StatusCode::OK,
                            crate::bot::git::whos(
                                &x.components().nth(1).unwrap().as_os_str().to_str().unwrap(),
                                u64::from_str_radix(
                                    &x.with_extension("")
                                        .file_name()
                                        .unwrap()
                                        .to_os_string()
                                        .to_str()
                                        .unwrap(),
                                    16,
                                )
                                .unwrap()
                                .into(),
                            ),
                        ),
                        None => (StatusCode::NOT_FOUND, String::default()),
                    }
                }),
            )
            .route(
                "/files/:file",
                get(|Path(file): Path<String>| async move {
                    match crate::bot::search::files().map(|(x, _)| x).find(|x| {
                        x.with_extension("").file_name().unwrap().to_string_lossy() == file
                    }) {
                        Some(x) => (StatusCode::OK, std::fs::read(x).unwrap()),
                        None => (StatusCode::NOT_FOUND, vec![]),
                    }
                }),
            );
        tokio::spawn(async move {
            AxumServer::bind(&addr)
                .serve(router.into_make_service())
                .await
                .unwrap();
        });
    }
}
